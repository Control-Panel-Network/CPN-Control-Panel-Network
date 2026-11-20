#!/bin/bash
# Install the binaries built by deploy-ols-proxy-build.sh, restart the panel, auto-rollback on failure.
set -euo pipefail
BK="$(cat /var/tmp/cpn-ols-last-backup)"
BUILD="$(cat /var/tmp/cpn-ols-last-build)"
log() { printf '[%s] %s\n' "$(date '+%F %T')" "$*"; }
[ -x "$BUILD/target/release/cpn-installer" ] && [ -x "$BUILD/target/release/cpn" ] || { log "missing build output"; exit 1; }
[ -x "$BK/cpn-installer" ] || { log "missing backup"; exit 1; }

rollback() {
  log "ROLLBACK from $BK"
  systemctl stop cpn-installer || true
  install -m 0755 "$BK/cpn" /usr/bin/cpn
  install -m 0755 "$BK/cpn-installer" /usr/bin/cpn-installer
  systemctl start cpn-installer || true
}

systemctl stop cpn-installer
install -m 0755 "$BUILD/target/release/cpn" /usr/bin/cpn
install -m 0755 "$BUILD/target/release/cpn-installer" /usr/bin/cpn-installer
systemctl start cpn-installer
ok=0
for i in 1 2 3 4 5 6 7 8 9 10; do
  sleep 2
  code="$(curl -sk -o /dev/null -w '%{http_code}' https://panel.newstargeted.com/login || true)"
  if systemctl is-active --quiet cpn-installer && [ "$code" = "200" ]; then ok=1; break; fi
  log "waiting for panel (try $i, http=$code)"
done
if [ "$ok" != 1 ]; then rollback; log "deploy FAILED, rolled back"; exit 1; fi
log "deploy OK: panel /login http=$code"
