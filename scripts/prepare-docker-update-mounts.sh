#!/usr/bin/env bash
# Copy state that was previously inside a running CPN Hub container into host
# bind directories before enabling the matching Compose overlays.
set -euo pipefail

[[ "${EUID}" -eq 0 ]] || { echo "Run as root." >&2; exit 1; }
command -v docker >/dev/null 2>&1 || { echo "Docker is required." >&2; exit 1; }

data_root="${CPN_DOCKER_DATA_ROOT:-/var/lib/cpn-docker-host}"
container="${CPN_CONTAINER_NAME:-cpn-installer}"
want_mail=0
want_lsws=0

while (($#)); do
  case "$1" in
    --mail) want_mail=1 ;;
    --lsws) want_lsws=1 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
  shift
done
(( want_mail == 1 || want_lsws == 1 )) || {
  echo "Usage: $0 --mail [--lsws]" >&2
  exit 1
}
[[ "$(docker inspect "$container" --format '{{.State.Running}}' 2>/dev/null)" == "true" ]] \
  || { echo "Container $container is not running." >&2; exit 1; }

copy_tree() {
  local source="$1" destination="$2"
  if [[ -n "$(ls -A "$destination" 2>/dev/null || true)" ]]; then
    echo "Refusing to overwrite non-empty $destination" >&2
    exit 1
  fi
  install -d -m 0755 "$destination"
  if docker exec "$container" test -d "$source"; then
    docker exec "$container" tar -C "$source" -cf - . | tar -C "$destination" -xf -
  fi
}

if (( want_mail == 1 )); then
  copy_tree /etc/postfix "$data_root/cpn-postfix"
  copy_tree /etc/dovecot "$data_root/cpn-dovecot"
  copy_tree /var/spool/postfix "$data_root/cpn-mail-spool"
  copy_tree /var/mail "$data_root/cpn-var-mail"
  echo "Prepared mail binds. Add docker-compose.hub.mail.yml to every Compose command."
fi

if (( want_lsws == 1 )); then
  copy_tree /usr/local/lsws "$data_root/cpn-lsws"
  echo "Prepared web server bind. Add docker-compose.hub.lsws.yml to every Compose command."
fi
