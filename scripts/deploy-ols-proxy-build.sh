#!/bin/bash
# Build CPN with the OLS WebAdmin proxy fix in an isolated dir, after backing up live binaries.
# Does NOT install or restart anything. Run deploy-ols-proxy-install.sh afterwards.
set -euo pipefail
export PATH="/root/.cargo/bin:/usr/bin:/bin:$PATH"
TS="$(date +%Y%m%d-%H%M%S)"
BK="/home/backup/cpn-ols-proxy-fix-$TS"
BUILD="/var/tmp/cpn-ols-build-$TS"
log() { printf '[%s] %s\n' "$(date '+%F %T')" "$*"; }

mkdir -p "$BK"
cp -a /usr/bin/cpn "$BK/cpn"
cp -a /usr/bin/cpn-installer "$BK/cpn-installer"
cp -a /etc/systemd/system/cpn-installer.service.d "$BK/" 2>/dev/null || true
sha256sum "$BK/cpn" /usr/bin/cpn "$BK/cpn-installer" /usr/bin/cpn-installer | tee "$BK/SHA256SUMS" >/dev/null
[ "$(sha256sum < "$BK/cpn-installer" | cut -d' ' -f1)" = "$(sha256sum < /usr/bin/cpn-installer | cut -d' ' -f1)" ] \
  || { log "backup verification FAILED"; exit 1; }
[ "$(sha256sum < "$BK/cpn" | cut -d' ' -f1)" = "$(sha256sum < /usr/bin/cpn | cut -d' ' -f1)" ] \
  || { log "backup verification FAILED (cpn)"; exit 1; }
log "backup verified at $BK"
echo "$BK" > /var/tmp/cpn-ols-last-backup
echo "$BUILD" > /var/tmp/cpn-ols-last-build

mkdir -p "$BUILD"
tar -xzf /var/tmp/cpn-ols-src.tgz -C "$BUILD"
cp -a /root/cpn-build-live/installer-ui/dist "$BUILD/installer-ui/dist"
# warm dependency cache from the previous live build
cp -a /root/cpn-build-live/target "$BUILD/target"
cd "$BUILD"
export CPN_GIT_SHA="ols-proxy-fix-$TS"
log "cargo test (proxy module)"
cargo test --release --lib panel_ols_webadmin_proxy -j 6 2>&1 | tail -15
log "cargo build --release"
cargo build --release -j 6 2>&1 | tail -8
ls -la target/release/cpn target/release/cpn-installer
log "BUILD OK (not installed)"
