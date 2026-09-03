#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
rpm_path="${1:-}"
# Default CI/lab image is AlmaLinux 9. For AlmaLinux 10: CPN_TEST_IMAGE=almalinux:10
image="${CPN_TEST_IMAGE:-almalinux:9.8}"

if [[ -z "$rpm_path" ]]; then
  shopt -s nullglob
  candidates=(
    "$project_dir"/target/rpmbuild/RPMS/x86_64/cpn-installer-*.rpm
  )
  if ((${#candidates[@]} == 0)); then
    echo "No se encontró el RPM en target/rpmbuild/RPMS/x86_64/" >&2
    exit 1
  fi
  rpm_path="${candidates[0]}"
fi

if [[ ! -f "$rpm_path" ]]; then
  echo "No se encontró el RPM: $rpm_path" >&2
  exit 1
fi

start_container() {
  local name="$1"
  docker run -d --privileged --cgroupns=host --name "$name" --hostname "$name" \
    --tmpfs /run --tmpfs /run/lock -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
    -e container=docker "$image" /usr/lib/systemd/systemd >/dev/null
  for _ in {1..30}; do
    if docker exec "$name" systemctl is-system-running --quiet 2>/dev/null; then return; fi
    sleep 1
  done
  echo "systemd no inició en $name" >&2
  return 1
}

installer_token() {
  local name="$1"
  docker exec "$name" journalctl -u cpn-installer-test --no-pager -n 30 \
    | sed -n 's/.*token=\([[:alnum:]]*\).*/\1/p' | tail -1
}

wait_for_result() {
  local name="$1" token="$2"
  for _ in {1..240}; do
    local status
    status="$(docker exec "$name" curl -fsS "http://127.0.0.1:8787/api/status?token=$token")"
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

run_case() {
  local kind="$1" component="$2"
  local name="cpn-test-${kind}-${component}"
  echo "[TEST] $kind/$component"
  docker rm -f "$name" >/dev/null 2>&1 || true
  start_container "$name"
  docker cp "$rpm_path" "$name:/tmp/cpn-installer.rpm"
  docker exec "$name" dnf install -y /tmp/cpn-installer.rpm >/dev/null
  docker exec "$name" systemd-run --unit=cpn-installer-test /usr/bin/cpn-installer >/dev/null
  sleep 2
  local token
  token="$(installer_token "$name")"
  test -n "$token"
  docker exec "$name" curl -fsS -X POST -H 'Content-Type: application/json' \
    -d "{\"$kind\":\"$component\"}" \
    "http://127.0.0.1:8787/api/install/$kind?token=$token" >/dev/null
  wait_for_result "$name" "$token"

  case "$component" in
    nginx)
      docker exec "$name" systemctl is-active --quiet nginx
      docker exec "$name" nginx -t
      docker exec "$name" sh -lc "curl -fsS http://127.0.0.1/ 2>/dev/null | grep -qi 'nginx\\|AlmaLinux'"
      ;;
    caddy)
      docker exec "$name" systemctl is-active --quiet caddy
      docker exec "$name" caddy validate --config /etc/caddy/Caddyfile
      docker exec "$name" sh -lc "curl -fsSI http://127.0.0.1/ 2>/dev/null | grep -qi '^Server: Caddy'"
      ;;
    snappymail)
      docker exec "$name" systemctl is-active --quiet cpn-webmail
      docker exec "$name" php -m | grep -qi mbstring
      docker exec "$name" sh -lc "curl -fsS http://127.0.0.1:8888/ 2>/dev/null | grep -qi SnappyMail"
      ;;
    rainloop)
      docker exec "$name" systemctl is-active --quiet cpn-webmail
      docker exec "$name" php -m | grep -qi mbstring
      docker exec "$name" sh -lc "curl -fsS http://127.0.0.1:8888/ 2>/dev/null | grep -qi RainLoop"
      ;;
    roundcube)
      docker exec "$name" systemctl is-active --quiet cpn-webmail
      docker exec "$name" test -s /opt/cpn-webmail/roundcube/db.sqlite
      docker exec "$name" sh -lc "curl -fsS http://127.0.0.1:8888/ 2>/dev/null | grep -qi Roundcube"
      ;;
    thunderbird)
      docker exec "$name" rpm -q thunderbird
      docker exec "$name" thunderbird --version | grep -qi Thunderbird
      ;;
  esac
  docker rm -f "$name" >/dev/null
  echo "[OK] $kind/$component"
}

docker pull "$image" >/dev/null
if [[ "${CPN_TEST_SCOPE:-all}" != "mail" ]]; then
  for server in ${CPN_TEST_SERVERS:-nginx caddy}; do run_case server "$server"; done
fi
if [[ "${CPN_TEST_SCOPE:-all}" != "server" ]]; then
  for mail in ${CPN_TEST_MAILS:-snappymail rainloop roundcube thunderbird}; do run_case mail "$mail"; done
fi
