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

echo "[OK] webmail permissions and php-fpm runtime"
