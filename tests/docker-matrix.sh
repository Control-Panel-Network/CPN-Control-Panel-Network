#!/usr/bin/env bash
# Functional install matrix for web servers and mail clients.
# Default: AlmaLinux 9 RPM. Override with CPN_TEST_IMAGE or CPN_TEST_IMAGES.
# Examples:
#   CPN_TEST_IMAGES="almalinux:9 rockylinux:9" CPN_TEST_SCOPE=server CPN_TEST_SERVERS=nginx ./tests/docker-matrix.sh
#   CPN_TEST_IMAGE=ubuntu:22.04 ./tests/docker-matrix.sh /path/to/cpn-installer_*.deb
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pkg_path="${1:-}"
# Space-separated images. CPN_TEST_IMAGES wins over single CPN_TEST_IMAGE.
images="${CPN_TEST_IMAGES:-${CPN_TEST_IMAGE:-almalinux:9.8}}"
container_engine="${CPN_CONTAINER_ENGINE:-}"

detect_engine() {
  if [[ -n "$container_engine" ]]; then
    echo "$container_engine"
    return
  fi
  if command -v docker >/dev/null 2>&1; then
    echo docker
    return
  fi
  if command -v podman >/dev/null 2>&1; then
    echo podman
    return
  fi
  echo "Neither docker nor podman was found in PATH." >&2
  exit 1
}

engine="$(detect_engine)"

is_apt_image() {
  local image="$1"
  [[ "$image" == *ubuntu* || "$image" == *debian* ]]
}

resolve_pkg() {
  local image="$1"
  if [[ -n "$pkg_path" ]]; then
    echo "$pkg_path"
    return
  fi
  shopt -s nullglob
  if is_apt_image "$image"; then
    local debs=(
      "$project_dir"/target/deb/cpn-installer_*.deb
    )
    if ((${#debs[@]} == 0)); then
      echo "No .deb found under target/deb/. Build with scripts/build-deb.sh or scripts/docker-build-deb.sh" >&2
      exit 1
    fi
    echo "${debs[0]}"
  else
    local rpms=(
      "$project_dir"/target/rpmbuild/RPMS/x86_64/cpn-installer-*.rpm
      "$project_dir"/target/rpmbuild/RPMS/aarch64/cpn-installer-*.rpm
      "$project_dir"/target/rpmbuild/RPMS/*/cpn-installer-*.rpm
    )
    if ((${#rpms[@]} == 0)); then
      echo "No RPM found under target/rpmbuild/RPMS/{x86_64,aarch64,...}/" >&2
      exit 1
    fi
    echo "${rpms[0]}"
  fi
}

start_container() {
  local name="$1" image="$2"
  local entry=()
  local status=""
  if "$engine" run --rm "$image" test -x /usr/lib/systemd/systemd >/dev/null 2>&1; then
    entry=(/usr/lib/systemd/systemd)
  else
    # Minimal Rocky (and some apt) images omit systemd; install then exec as PID 1.
    entry=(
      bash -lc
      'if command -v dnf >/dev/null 2>&1; then dnf install -y systemd systemd-udev; elif command -v apt-get >/dev/null 2>&1; then export DEBIAN_FRONTEND=noninteractive; apt-get update -y; apt-get install -y systemd systemd-sysv; else echo missing systemd and no package manager >&2; exit 1; fi; exec /usr/lib/systemd/systemd'
    )
  fi

  "$engine" run -d --privileged --cgroupns=host --name "$name" --hostname "$name" \
    --tmpfs /run --tmpfs /run/lock -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
    -e container=docker "$image" "${entry[@]}" >/dev/null
  for _ in {1..180}; do
    # Capture status text explicitly (avoid set -e / pipefail traps on degraded).
    status="$("$engine" exec "$name" systemctl is-system-running 2>/dev/null || true)"
    if [[ "$status" == "running" || "$status" == "degraded" ]]; then
      return 0
    fi
    sleep 1
  done
  echo "systemd no inició en $name ($image) (last status='${status:-empty}')" >&2
  "$engine" logs "$name" 2>&1 | tail -n 80 >&2 || true
  return 1
}

# Log path inside the guest. Prefer this over systemd-run: nesting dnf/systemctl
# inside a transient unit deadlocks systemd on GHA and freezes :2087 (HTTP hang).
installer_log_path() {
  printf '%s' /tmp/cpn-installer.log
}

installer_token() {
  local name="$1"
  local log
  log="$(installer_log_path)"
  "$engine" exec "$name" bash -lc "test -f '$log' && sed -n 's/.*token=\\([[:alnum:]]*\\).*/\\1/p' '$log' | tail -1" \
    2>/dev/null || true
}

dump_installer_diag() {
  local name="$1"
  local log host_log
  log="$(installer_log_path)"
  host_log="/tmp/cpn-matrix-${name}.log"
  echo "[DIAG] installer log ($log):" >&2
  "$engine" exec "$name" bash -lc "tail -n 120 '$log' 2>/dev/null || echo '(missing)'" >&2 || true
  "$engine" cp "$name:$log" "$host_log" >/dev/null 2>&1 || true
  echo "[DIAG] installer processes:" >&2
  "$engine" exec "$name" bash -lc '
    pid=$(cat /tmp/cpn-installer.pid 2>/dev/null || true)
    echo "pid_file=${pid:-missing}"
    if [[ -n "$pid" && -d "/proc/$pid" ]]; then
      echo "proc_state=$(cut -d" " -f1-3 /proc/$pid/stat 2>/dev/null || true)"
      tr "\0" " " < /proc/$pid/cmdline; echo
      ls /proc/$pid/task 2>/dev/null | wc -l | awk "{print \"tasks=\" \$1}"
    else
      echo "installer pid not alive"
    fi
    command -v ps >/dev/null && ps -ef | grep -E "[c]pn-installer|[d]nf|[y]um" || true
  ' >&2 || true
  echo "[DIAG] listeners (ss/netstat//proc/net/tcp):" >&2
  "$engine" exec "$name" bash -lc '
    ss -ltn 2>/dev/null || netstat -ltn 2>/dev/null || true
    grep -E " 0517 | 0050 " /proc/net/tcp 2>/dev/null || true
  ' >&2 || true
  echo "[DIAG] last /api/status (best effort):" >&2
  "$engine" exec "$name" bash -lc 'curl -sS --max-time 3 "http://127.0.0.1:2087/api/status" || true' >&2 || true
}

# Start installer in the background (not as a systemd unit). systemd-run --no-block
# still left the process as a unit; package scriptlets calling systemctl then wedged
# the nested systemd and stopped answering HTTP on :2087.
start_installer() {
  local name="$1"
  local token=""
  local log
  log="$(installer_log_path)"
  "$engine" exec "$name" bash -lc \
    "rm -f '$log' /tmp/cpn-installer.pid; nohup /usr/bin/cpn-installer >'$log' 2>&1 </dev/null & echo \$! >/tmp/cpn-installer.pid"
  for _ in {1..90}; do
    token="$(installer_token "$name")"
    if [[ -n "$token" ]]; then
      if "$engine" exec "$name" curl -fsS --max-time 3 \
        "http://127.0.0.1:2087/api/status?token=$token" >/dev/null 2>&1; then
        printf '%s' "$token"
        return 0
      fi
    fi
    sleep 1
  done
  echo "Installer did not become ready in $name (no token/HTTP on :2087)" >&2
  dump_installer_diag "$name"
  return 1
}

wait_for_result() {
  local name="$1" token="$2"
  local empty=0
  for _ in {1..240}; do
    local status
    status="$("$engine" exec "$name" curl -fsS --max-time 5 "http://127.0.0.1:2087/api/status?token=$token" 2>/dev/null || true)"
    if [[ -z "$status" ]]; then
      empty=$((empty + 1))
      # HTTP frozen (often nested-systemd deadlock). Fail before the job timeout.
      if ((empty >= 18)); then
        echo "Installer HTTP stopped responding in $name (empty status x${empty})" >&2
        dump_installer_diag "$name"
        return 1
      fi
      sleep 1
      continue
    fi
    empty=0
    if [[ "$status" == *'"phase":"completed"'* ]]; then return; fi
    if [[ "$status" == *'"phase":"failed"'* ]]; then
      echo "$status" >&2
      dump_installer_diag "$name"
      return 1
    fi
    sleep 1
  done
  echo "La instalación agotó el tiempo en $name" >&2
  dump_installer_diag "$name"
  return 1
}

post_json() {
  local name="$1" url="$2" body="$3"
  local response code
  response="$("$engine" exec "$name" curl -sS --max-time 30 -w '\n%{http_code}' \
    -X POST -H 'Content-Type: application/json' -d "$body" "$url" || true)"
  code="$(printf '%s' "$response" | tail -n1)"
  response="$(printf '%s' "$response" | sed '$d')"
  if [[ "$code" != "202" && "$code" != "200" ]]; then
    echo "POST $url failed http=$code body=$response" >&2
    dump_installer_diag "$name"
    return 1
  fi
}

install_pkg() {
  local name="$1" pkg="$2" image="$3"
  local base
  base="$(basename "$pkg")"
  "$engine" cp "$pkg" "$name:/tmp/$base"
  if is_apt_image "$image"; then
    "$engine" exec "$name" bash -lc "export DEBIAN_FRONTEND=noninteractive; apt-get update -y >/dev/null && apt-get install -y /tmp/$base >/dev/null"
  else
    "$engine" exec "$name" dnf install -y "/tmp/$base" >/dev/null
  fi
}

run_case() {
  local image="$1" kind="$2" component="$3" pkg="$4"
  local safe_image
  safe_image="$(echo "$image" | tr '/:' '--')"
  local name="cpn-test-${safe_image}-${kind}-${component}"
  echo "[TEST] image=$image $kind/$component (engine=$engine)"
  "$engine" rm -f "$name" >/dev/null 2>&1 || true
  start_container "$name" "$image"
  install_pkg "$name" "$pkg" "$image"
  local token
  token="$(start_installer "$name")"
  test -n "$token"
  if [[ "$kind" == "mail" ]]; then
    post_json "$name" "http://127.0.0.1:2087/api/install/server?token=$token" \
      '{"server":"nginx","database":"none","install_phpmyadmin":false}'
    wait_for_result "$name" "$token"
  fi
  if [[ "$kind" == "server" ]]; then
    post_json "$name" "http://127.0.0.1:2087/api/install/server?token=$token" \
      "{\"server\":\"$component\",\"database\":\"none\",\"install_phpmyadmin\":false}"
  else
    post_json "$name" "http://127.0.0.1:2087/api/install/$kind?token=$token" \
      "{\"$kind\":\"$component\"}"
  fi
  wait_for_result "$name" "$token"

  case "$component" in
    nginx)
      "$engine" exec "$name" systemctl is-active --quiet nginx
      "$engine" exec "$name" nginx -t
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1/ 2>/dev/null | grep -qi 'nginx\\|AlmaLinux\\|Welcome\\|Ubuntu\\|Debian'"
      # External-ish check: query from the container's eth0 address, not loopback (issue #21).
      "$engine" exec "$name" sh -lc '
        set -e
        ip=$(hostname -I 2>/dev/null | awk "{print \$1}")
        if [[ -n "$ip" && "$ip" != "127.0.0.1" ]]; then
          curl -fsS --max-time 5 "http://$ip/" >/dev/null
        else
          echo "skip non-loopback probe (no eth IP)"
        fi
      '
      ;;
    caddy)
      "$engine" exec "$name" systemctl is-active --quiet caddy
      "$engine" exec "$name" caddy validate --config /etc/caddy/Caddyfile
      "$engine" exec "$name" sh -lc "curl -fsSI http://127.0.0.1/ 2>/dev/null | grep -qi '^Server: Caddy'"
      ;;
    openlitespeed)
      "$engine" exec "$name" sh -lc 'systemctl is-active --quiet lsws || systemctl is-active --quiet lshttpd'
      "$engine" exec "$name" test ! -e /etc/systemd/system/openlitespeed.service
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1/ 2>/dev/null | grep -qi 'CPN OpenLiteSpeed'"
      ;;
    snappymail)
      "$engine" exec "$name" sh -lc 'systemctl is-active --quiet php-fpm || systemctl is-active --quiet php*-fpm'
      "$engine" exec "$name" test ! -e /etc/systemd/system/cpn-webmail.service || \
        ! "$engine" exec "$name" systemctl cat cpn-webmail 2>/dev/null | grep -q 'php -S'
      "$engine" exec "$name" php -m | grep -qi mbstring
      "$engine" exec "$name" bash /usr/share/cpn-installer/webmail-permissions.sh /opt/cpn-webmail/snappymail 2>/dev/null || \
        "$engine" exec -i "$name" bash -s /opt/cpn-webmail/snappymail <"$project_dir/tests/webmail-permissions.sh"
      "$engine" exec "$name" sh -lc "ss -ltn | grep -E ':143|:587|:25'"
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1:8080/ 2>/dev/null | grep -qi SnappyMail"
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1:2087/api/status?token=$token | grep -q '\"mail_backend_ready\":true'"
      ;;
    roundcube)
      "$engine" exec "$name" sh -lc 'systemctl is-active --quiet php-fpm || systemctl is-active --quiet php*-fpm'
      "$engine" exec "$name" test -s /opt/cpn-webmail/roundcube/db.sqlite
      "$engine" exec "$name" php -r '$db=new PDO("sqlite:/opt/cpn-webmail/roundcube/db.sqlite"); $n=$db->query("SELECT name FROM sqlite_master WHERE type=\"table\" AND name=\"users\"")->fetchColumn(); if(!$n){exit(1);}'
      "$engine" exec "$name" php -r '$m=fileperms("/opt/cpn-webmail/roundcube/db.sqlite") & 0777; if ($m & 0002) {exit(1);}'
      "$engine" exec -i "$name" bash -s /opt/cpn-webmail/roundcube/public_html <"$project_dir/tests/webmail-permissions.sh"
      "$engine" exec "$name" sh -lc "ss -ltn | grep -E ':143|:587|:25'"
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1:8080/ 2>/dev/null | grep -qi Roundcube"
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1:2087/api/status?token=$token | grep -q '\"mail_backend_ready\":true'"
      ;;
    thunderbird)
      if is_apt_image "$image"; then
        "$engine" exec "$name" dpkg -l thunderbird | grep -qi thunderbird
      else
        "$engine" exec "$name" rpm -q thunderbird
      fi
      "$engine" exec "$name" thunderbird --version | grep -qi Thunderbird
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1:2087/api/status?token=$token | grep -q '\"mail_backend_ready\":false'"
      ;;
  esac
  "$engine" rm -f "$name" >/dev/null
  echo "[OK] image=$image $kind/$component"
}

run_image_matrix() {
  local image="$1"
  local pkg
  pkg="$(resolve_pkg "$image")"
  if [[ ! -f "$pkg" ]]; then
    echo "Package not found: $pkg" >&2
    exit 1
  fi
  echo "[MATRIX] Pulling $image ..."
  "$engine" pull "$image" >/dev/null
  if [[ "${CPN_TEST_SCOPE:-all}" != "mail" ]]; then
    for server in ${CPN_TEST_SERVERS:-nginx caddy openlitespeed}; do
      run_case "$image" server "$server" "$pkg"
    done
  fi
  if [[ "${CPN_TEST_SCOPE:-all}" != "server" ]]; then
    for mail in ${CPN_TEST_MAILS:-snappymail roundcube thunderbird}; do
      run_case "$image" mail "$mail" "$pkg"
    done
  fi
}

for image in $images; do
  run_image_matrix "$image"
done

echo "[DONE] docker-matrix images: $images"
