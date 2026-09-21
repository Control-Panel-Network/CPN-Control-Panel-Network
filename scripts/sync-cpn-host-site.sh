#!/usr/bin/env bash
# Sync CPN host site (landing + bootstrap scripts) to a staging directory or live docroot.
# Usage:
#   ./scripts/sync-cpn-host-site.sh                 # write to ./site-dist
#   ./scripts/sync-cpn-host-site.sh /path/to/docroot
# Deploy example (from a machine with SSH to the VPS):
#   ./scripts/sync-cpn-host-site.sh /tmp/cpn-host-dist
#   rsync -az --delete /tmp/cpn-host-dist/ root@host:/home/newstargeted.com/public_html/cpn.newstargeted.com/
#   # then chown newst3922:newst3922 and restart LSWS if .htaccess changed
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${1:-$ROOT/site-dist}"

mkdir -p "$DEST"

rsync -a --delete \
  --exclude '.git' \
  --exclude 'README.md' \
  "$ROOT/site/" "$DEST/"

install -m 0644 "$ROOT/scripts/install.sh" "$DEST/install.sh"
install -m 0644 "$ROOT/scripts/upgrade.sh" "$DEST/upgrade.sh"
install -m 0644 "$ROOT/scripts/cpn-bootstrap-lib.sh" "$DEST/cpn-bootstrap-lib.sh"

# Optional alias used by some older docs; keep if present in scripts/
if [[ -f "$ROOT/scripts/preUpgrade.sh" ]]; then
  install -m 0644 "$ROOT/scripts/preUpgrade.sh" "$DEST/preUpgrade.sh"
fi

echo "Synced CPN host site -> $DEST"
ls -la "$DEST" | head -30
