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
  if su -s /bin/bash cpn-webmail -c "touch '$DOCROOT/.__cpn_perm_probe.php'" 2>/dev/null; then
    rm -f "$DOCROOT/.__cpn_perm_probe.php"
    echo "cpn-webmail can write PHP into docroot" >&2
    exit 1
  fi
fi

if systemctl list-unit-files | grep -q '^cpn-webmail.service'; then
  if systemctl cat cpn-webmail 2>/dev/null | grep -q 'php -S'; then
    echo "legacy php -S unit still active" >&2
    exit 1
  fi
fi

if ! systemctl is-active --quiet php-fpm; then
  echo "php-fpm is not active" >&2
  exit 1
fi

echo "[OK] webmail permissions and php-fpm runtime"
