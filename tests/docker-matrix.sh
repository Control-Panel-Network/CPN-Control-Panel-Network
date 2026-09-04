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
  "$engine" run -d --privileged --cgroupns=host --name "$name" --hostname "$name" \
    --tmpfs /run --tmpfs /run/lock -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
    -e container=docker "$image" /usr/lib/systemd/systemd >/dev/null
  for _ in {1..45}; do
    if "$engine" exec "$name" systemctl is-system-running --quiet 2>/dev/null; then return; fi
    # Some images report "degraded" while still usable.
    if "$engine" exec "$name" systemctl is-system-running 2>/dev/null | grep -Eq 'running|degraded'; then
      return
    fi
    sleep 1
  done
  echo "systemd no inició en $name ($image)" >&2
  return 1
}

installer_token() {
  local name="$1"
  "$engine" exec "$name" journalctl -u cpn-installer-test --no-pager -n 40 \
    | sed -n 's/.*token=\([[:alnum:]]*\).*/\1/p' | tail -1
}

wait_for_result() {
  local name="$1" token="$2"
  for _ in {1..240}; do
    local status
    status="$("$engine" exec "$name" curl -fsS "http://127.0.0.1:2087/api/status?token=$token")"
    if [[ "$status" == *'"phase":"completed"'* ]]; then return; fi
    if [[ "$status" == *'"phase":"failed"'* ]]; then
      echo "$status" >&2
      return 1
    fi
    sleep 1
  done
  echo "La instalación agotó el tiempo en $name" >&2
  return 1
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
  "$engine" exec "$name" systemd-run --unit=cpn-installer-test /usr/bin/cpn-installer >/dev/null
  sleep 2
  local token
  token="$(installer_token "$name")"
  test -n "$token"
  if [[ "$kind" == "mail" ]]; then
    "$engine" exec "$name" curl -fsS -X POST -H 'Content-Type: application/json' \
      -d '{"server":"nginx"}' \
      "http://127.0.0.1:2087/api/install/server?token=$token" >/dev/null
    wait_for_result "$name" "$token"
  fi
  "$engine" exec "$name" curl -fsS -X POST -H 'Content-Type: application/json' \
    -d "{\"$kind\":\"$component\"}" \
    "http://127.0.0.1:2087/api/install/$kind?token=$token" >/dev/null
  wait_for_result "$name" "$token"

  case "$component" in
    nginx)
      "$engine" exec "$name" systemctl is-active --quiet nginx
      "$engine" exec "$name" nginx -t
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1/ 2>/dev/null | grep -qi 'nginx\\|AlmaLinux\\|Welcome\\|Ubuntu\\|Debian'"
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
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1:8787/api/status?token=$token | grep -q '\"mail_backend_ready\":true'"
      ;;
    roundcube)
      "$engine" exec "$name" sh -lc 'systemctl is-active --quiet php-fpm || systemctl is-active --quiet php*-fpm'
      "$engine" exec "$name" test -s /opt/cpn-webmail/roundcube/db.sqlite
      "$engine" exec "$name" php -r '$db=new PDO("sqlite:/opt/cpn-webmail/roundcube/db.sqlite"); $n=$db->query("SELECT name FROM sqlite_master WHERE type=\"table\" AND name=\"users\"")->fetchColumn(); if(!$n){exit(1);}'
      "$engine" exec "$name" php -r '$m=fileperms("/opt/cpn-webmail/roundcube/db.sqlite") & 0777; if ($m & 0002) {exit(1);}'
      "$engine" exec -i "$name" bash -s /opt/cpn-webmail/roundcube/public_html <"$project_dir/tests/webmail-permissions.sh"
      "$engine" exec "$name" sh -lc "ss -ltn | grep -E ':143|:587|:25'"
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1:8080/ 2>/dev/null | grep -qi Roundcube"
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1:8787/api/status?token=$token | grep -q '\"mail_backend_ready\":true'"
      ;;
    thunderbird)
      if is_apt_image "$image"; then
        "$engine" exec "$name" dpkg -l thunderbird | grep -qi thunderbird
      else
        "$engine" exec "$name" rpm -q thunderbird
      fi
      "$engine" exec "$name" thunderbird --version | grep -qi Thunderbird
      "$engine" exec "$name" sh -lc "curl -fsS http://127.0.0.1:8787/api/status?token=$token | grep -q '\"mail_backend_ready\":false'"
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
