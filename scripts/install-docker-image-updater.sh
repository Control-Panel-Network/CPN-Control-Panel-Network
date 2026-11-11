#!/usr/bin/env bash
# Install or run the CPN outer-container image updater.
# The installed timer runs on the Docker host. The panel container never gets
# /var/run/docker.sock.
set -euo pipefail

readonly install_path="/usr/local/sbin/cpn-docker-image-updater"
readonly config_dir="/etc/cpn"
readonly config_path="${config_dir}/docker-image-updater.conf"
readonly compose_copy="${config_dir}/docker-image-updater.compose.yml"
readonly service_path="/etc/systemd/system/cpn-docker-image-update.service"
readonly timer_path="/etc/systemd/system/cpn-docker-image-update.timer"

die() { echo "cpn-docker-image-updater: $*" >&2; exit 1; }
log() { echo "cpn-docker-image-updater: $*"; }

require_root() {
  [[ "${EUID}" -eq 0 ]] || die "run as root"
}

valid_image() {
  [[ "$1" =~ ^master3395/cpn-installer:(almalinux9|almalinux10|ubuntu26\.04|latest)$ ]]
}

valid_data_root() {
  [[ "$1" =~ ^/[A-Za-z0-9._/-]+$ && "$1" != *".."* && "$1" != *"//"* ]]
}

json_value() {
  local file="$1" key="$2" fallback="$3"
  python3 - "$file" "$key" "$fallback" <<'PY'
import json, pathlib, sys
path, key, fallback = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3]
try:
    value = json.loads(path.read_text(encoding="utf-8")).get(key, fallback)
except Exception:
    value = fallback
print(value if not isinstance(value, bool) else ("true" if value else "false"))
PY
}

write_json_status() {
  local ok="$1" result="$2" image="$3" image_id="${4:-}"
  STATUS_OK="$ok" STATUS_RESULT="$result" STATUS_IMAGE="$image" STATUS_IMAGE_ID="$image_id" \
    STATUS_PATH="$status_path" python3 <<'PY'
import json, os, pathlib, time
path = pathlib.Path(os.environ["STATUS_PATH"])
path.parent.mkdir(parents=True, exist_ok=True)
data = {
    "container_deployment": True,
    "host_updater_configured": True,
    "image": os.environ["STATUS_IMAGE"],
    "compose_service": "cpn-installer",
    "last_check_at": int(time.time()),
    "last_check_ok": os.environ["STATUS_OK"] == "1",
    "last_result": os.environ["STATUS_RESULT"],
    "current_image_id": os.environ["STATUS_IMAGE_ID"],
    "pending_request": (path.parent / "request.json").exists(),
    "privilege_note": "Root-owned host helper; no Docker socket in the panel container.",
}
tmp = path.with_suffix(".json.tmp")
tmp.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
os.chmod(tmp, 0o600)
tmp.replace(path)
PY
}

load_config() {
  [[ -f "$config_path" ]] || die "missing ${config_path}; run install first"
  [[ "$(stat -c '%a' "$config_path")" == "600" ]] || die "${config_path} must be mode 600"
  image="$(awk -F= '$1=="IMAGE"{print substr($0,index($0,"=")+1)}' "$config_path")"
  data_root="$(awk -F= '$1=="DATA_ROOT"{print substr($0,index($0,"=")+1)}' "$config_path")"
  valid_image "$image" || die "configured image is not an allowed CPN Hub stream"
  valid_data_root "$data_root" || die "invalid DATA_ROOT"
  update_dir="${data_root}/cpn-data/docker-host-update"
  request_path="${update_dir}/request.json"
  status_path="${update_dir}/status.json"
  auto_config="${data_root}/cpn-data/auto-upgrade.json"
}

mount_destinations() {
  docker inspect cpn-installer --format '{{range .Mounts}}{{println .Destination}}{{end}}'
}

require_mounts() {
  local mounts required
  mounts="$(mount_destinations)"
  for required in /var/lib/cpn /home /var/lib/mysql /etc/letsencrypt /var/lib/cpn-webmail; do
    grep -Fxq "$required" <<<"$mounts" || die "refusing recreate: required persistent mount ${required} is missing"
  done

  if docker exec cpn-installer sh -c \
      'systemctl is-enabled postfix >/dev/null 2>&1 || systemctl is-enabled dovecot >/dev/null 2>&1'; then
    for required in /etc/postfix /etc/dovecot /var/spool/postfix /var/mail; do
      grep -Fxq "$required" <<<"$mounts" || die "refusing recreate: mail is enabled but persistent mount ${required} is missing"
    done
  fi

  if docker exec cpn-installer test -d /usr/local/lsws; then
    grep -Fxq /usr/local/lsws <<<"$mounts" \
      || die "refusing recreate: /usr/local/lsws exists but its persistent mount is missing"
  fi
}

host_config_marker() {
  HOST_CONFIG_PATH="${update_dir}/host-config.json" HOST_IMAGE="$image" python3 <<'PY'
import json, os, pathlib
path = pathlib.Path(os.environ["HOST_CONFIG_PATH"])
path.parent.mkdir(parents=True, exist_ok=True)
tmp = path.with_suffix(".json.tmp")
tmp.write_text(json.dumps({
    "schema": 1,
    "image": os.environ["HOST_IMAGE"],
    "compose_service": "cpn-installer",
    "socket_mounted_into_panel": False,
}, indent=2) + "\n", encoding="utf-8")
os.chmod(tmp, 0o600)
tmp.replace(path)
PY
}

mark_auto_applied() {
  local version="$1"
  [[ -n "$version" && -f "$auto_config" ]] || return 0
  AUTO_CONFIG_PATH="$auto_config" AUTO_APPLIED_VERSION="$version" python3 <<'PY'
import json, os, pathlib, time
path = pathlib.Path(os.environ["AUTO_CONFIG_PATH"])
try:
    data = json.loads(path.read_text(encoding="utf-8"))
except Exception:
    raise SystemExit(0)
data["last_applied_version"] = os.environ["AUTO_APPLIED_VERSION"]
data["last_check_at"] = int(time.time())
data["last_check_ok"] = True
data["last_result"] = "Host image update completed and the recreated panel service is active."
tmp = path.with_suffix(".json.tmp")
tmp.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
os.chmod(tmp, 0o600)
tmp.replace(path)
PY
}

run_update() {
  require_root
  command -v docker >/dev/null 2>&1 || die "Docker is required"
  command -v python3 >/dev/null 2>&1 || die "python3 is required"
  docker compose version >/dev/null 2>&1 || die "Docker Compose plugin is required"
  load_config
  mkdir -p "$update_dir"
  chmod 700 "$update_dir"
  host_config_marker

  [[ "$(docker inspect cpn-installer --format '{{index .Config.Labels "com.cpn.outer-update"}}' 2>/dev/null)" == "1" ]] \
    || die "running cpn-installer lacks the managed outer-update label"
  running_image="$(docker inspect cpn-installer --format '{{.Config.Image}}')"
  [[ "$running_image" == "$image" ]] || die "running image ${running_image} does not match configured ${image}"
  require_mounts

  enabled="$(json_value "$auto_config" enabled true)"
  has_request=0
  [[ -f "$request_path" ]] && has_request=1
  if [[ "$has_request" != "1" ]]; then
    write_json_status 1 "Host updater ready; waiting for Version Management or the scheduled panel check." "$image"
    return 0
  fi
  requested_by="$(json_value "$request_path" requested_by "")"
  requested_version=""
  requested_version="$(json_value "$request_path" target_version "")"
  if [[ "$requested_by" == "automatic" && "$enabled" != "true" ]]; then
    rm -f "$request_path"
    write_json_status 1 "Automatic image updates are disabled by the Owner." "$image"
    return 0
  fi

  old_id="$(docker inspect cpn-installer --format '{{.Image}}')"
  if ! pull_output="$(docker pull "$image" 2>&1)"; then
    write_json_status 0 "Image pull failed; running container was not changed." "$image" "$old_id"
    printf '%s\n' "$pull_output" >&2
    return 1
  fi
  new_id="$(docker image inspect "$image" --format '{{.Id}}')"
  if [[ "$new_id" == "$old_id" ]]; then
    mark_auto_applied "$requested_version"
    rm -f "$request_path"
    write_json_status 1 "Already running the latest published Hub image." "$image" "$old_id"
    return 0
  fi

  rollback_tag="cpn-installer:rollback-$(date +%s)"
  docker tag "$old_id" "$rollback_tag"
  if ! CPN_HUB_IMAGE="$image" docker compose -f "$compose_copy" up -d --no-deps cpn-installer; then
    docker tag "$old_id" "$image"
    CPN_HUB_IMAGE="$image" docker compose -f "$compose_copy" up -d --no-deps cpn-installer || true
    write_json_status 0 "Compose recreate failed; rollback was attempted." "$image" "$old_id"
    return 1
  fi

  healthy=0
  for _ in $(seq 1 60); do
    if [[ "$(docker inspect cpn-installer --format '{{.State.Running}}' 2>/dev/null || true)" == "true" ]] \
      && docker exec cpn-installer systemctl is-active --quiet cpn-installer.service; then
      healthy=1
      break
    fi
    sleep 3
  done
  if [[ "$healthy" != "1" ]]; then
    docker tag "$old_id" "$image"
    CPN_HUB_IMAGE="$image" docker compose -f "$compose_copy" up -d --no-deps cpn-installer || true
    write_json_status 0 "New image failed health verification; rolled back to the previous image." "$image" "$old_id"
    return 1
  fi

  mark_auto_applied "$requested_version"
  rm -f "$request_path"
  docker image rm "$rollback_tag" >/dev/null 2>&1 || true
  write_json_status 1 "Updated the outer CPN container image and verified the panel service." "$image" "$new_id"
  log "updated ${old_id} to ${new_id}"
}

install_updater() {
  require_root
  command -v docker >/dev/null 2>&1 || die "Docker is required"
  command -v python3 >/dev/null 2>&1 || die "python3 is required"
  local image="master3395/cpn-installer:almalinux9"
  local data_root="/var/lib/cpn-docker-host"
  local -a compose_files=()
  shift
  while (($#)); do
    case "$1" in
      --compose-file) [[ $# -ge 2 ]] || die "--compose-file needs a path"; compose_files+=("$2"); shift 2 ;;
      --image) [[ $# -ge 2 ]] || die "--image needs a value"; image="$2"; shift 2 ;;
      --data-root) [[ $# -ge 2 ]] || die "--data-root needs a path"; data_root="$2"; shift 2 ;;
      *) die "unknown install option: $1" ;;
    esac
  done
  ((${#compose_files[@]} > 0)) || die "provide --compose-file docker-compose.hub.yml"
  valid_image "$image" || die "image must be an allowed master3395/cpn-installer Hub stream"
  valid_data_root "$data_root" \
    || die "data root must be an absolute path using letters, numbers, dot, underscore, slash, or hyphen"

  local file owner mode
  local -a compose_args=()
  for file in "${compose_files[@]}"; do
    file="$(readlink -f "$file")"
    [[ -f "$file" ]] || die "compose file not found: $file"
    owner="$(stat -c '%u' "$file")"; mode="$(stat -c '%a' "$file")"
    [[ "$owner" == "0" ]] || die "compose file must be owned by root: $file"
    (( (8#$mode & 8#022) == 0 )) || die "compose file must not be group/world writable: $file"
    compose_args+=(-f "$file")
  done

  install -d -m 700 "$config_dir"
  CPN_HUB_IMAGE="$image" CPN_DOCKER_DATA_ROOT="$data_root" \
    docker compose "${compose_args[@]}" config > "${compose_copy}.tmp"
  install -m 600 "${compose_copy}.tmp" "$compose_copy"
  rm -f "${compose_copy}.tmp"
  printf 'IMAGE=%s\nDATA_ROOT=%s\n' "$image" "$data_root" > "${config_path}.tmp"
  install -m 600 "${config_path}.tmp" "$config_path"
  rm -f "${config_path}.tmp"
  if [[ "$(readlink -f "$0")" != "$install_path" ]]; then
    install -m 755 "$0" "$install_path"
  fi

  load_config
  [[ "$(docker inspect cpn-installer --format '{{index .Config.Labels "com.cpn.outer-update"}}' 2>/dev/null)" == "1" ]] \
    || die "running cpn-installer lacks the managed outer-update label; recreate it from the current Hub Compose file first"
  [[ "$(docker inspect cpn-installer --format '{{.Config.Image}}')" == "$image" ]] \
    || die "running cpn-installer does not use configured image ${image}"
  require_mounts

  cat >"$service_path" <<EOF
[Unit]
Description=CPN Docker image update
After=docker.service network-online.target
Wants=network-online.target
Requires=docker.service

[Service]
Type=oneshot
ExecStart=${install_path} run
PrivateTmp=true
ProtectHome=read-only
ProtectSystem=strict
ReadWritePaths=${data_root} ${config_dir} /run/docker.sock
NoNewPrivileges=true
CapabilityBoundingSet=
UMask=0077
EOF
  cat >"$timer_path" <<'EOF'
[Unit]
Description=Check CPN Docker image updates

[Timer]
OnBootSec=3min
OnUnitActiveSec=15min
RandomizedDelaySec=2min
Persistent=true
Unit=cpn-docker-image-update.service

[Install]
WantedBy=timers.target
EOF
  chmod 644 "$service_path" "$timer_path"
  systemctl daemon-reload
  systemctl enable --now cpn-docker-image-update.timer
  mkdir -p "$update_dir"
  chmod 700 "$update_dir"
  host_config_marker
  log "installed. Timer checks every 15 minutes and honors the Owner interval (default 6 hours)."
}

case "${1:-}" in
  install) install_updater "$@" ;;
  run) run_update ;;
  *) die "usage: $0 install --compose-file FILE [--compose-file OVERLAY] [--image IMAGE] [--data-root PATH] | run" ;;
esac
