#!/usr/bin/env bash
# Verify webmail docroot is root-owned and not writable by cpn-webmail (issue #6).
set -euo pipefail

DOCROOT="${1:-/opt/cpn-webmail/current}"
if [[ ! -e "$DOCROOT" ]]; then
  echo "docroot missing: $DOCROOT" >&2
  exit 1
fi

if [[ "$(stat -c '%U' /opt/cpn-webmail)" == "cpn-webmail" ]]; then
  echo "/opt/cpn-webmail must not be owned by cpn-webmail" >&2
  exit 1
fi

bad="$(find "$DOCROOT" -type f -name '*.php' ! -user root -print -quit || true)"
if [[ -n "$bad" ]]; then
  echo "PHP file not root-owned: $bad" >&2
  exit 1
fi

if id cpn-webmail >/dev/null 2>&1; then
  # Prefer runuser/sudo so non-interactive lab runs never hang on su password prompts.
  if command -v runuser >/dev/null 2>&1; then
    if runuser -u cpn-webmail -- touch "$DOCROOT/.__cpn_perm_probe.php" 2>/dev/null; then
      rm -f "$DOCROOT/.__cpn_perm_probe.php"
      echo "cpn-webmail can write PHP into docroot" >&2
      exit 1
    fi
  elif sudo -n -u cpn-webmail touch "$DOCROOT/.__cpn_perm_probe.php" 2>/dev/null; then
    rm -f "$DOCROOT/.__cpn_perm_probe.php"
    echo "cpn-webmail can write PHP into docroot" >&2
    exit 1
  fi
fi

if [[ -e /etc/systemd/system/cpn-webmail.service ]] \
  && grep -q 'php -S' /etc/systemd/system/cpn-webmail.service 2>/dev/null; then
  echo "legacy php -S unit still present" >&2
  exit 1
fi

if ! systemctl is-active --quiet php-fpm; then
  echo "php-fpm is not active" >&2
  exit 1
fi

# SnappyMail: data must live outside the HTTP docroot when include.php is present.
if [[ -f "$DOCROOT/include.php" ]] && grep -q "APP_DATA_FOLDER_PATH" "$DOCROOT/include.php"; then
  if [[ -e "$DOCROOT/data" ]]; then
    echo "SnappyMail docroot must not contain a data/ tree when APP_DATA_FOLDER_PATH is set" >&2
    exit 1
  fi
  if [[ ! -d /var/lib/cpn-webmail/snappymail ]]; then
    echo "missing /var/lib/cpn-webmail/snappymail for SnappyMail data" >&2
    exit 1
  fi
fi

# Optional live HTTP denial checks when the loopback proxy is up.
if curl -sS --max-time 2 -o /dev/null http://127.0.0.1:8080/ 2>/dev/null; then
  for path in data temp logs; do
    code="$(curl -sS -o /dev/null -w '%{http_code}' --max-time 5 "http://127.0.0.1:8080/${path}" || true)"
    case "$code" in
      200|301|302)
        echo "sensitive path /${path} returned ${code}" >&2
        exit 1
        ;;
    esac
  done
fi

echo "[OK] webmail permissions and php-fpm runtime"
