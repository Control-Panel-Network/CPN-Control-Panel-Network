#!/usr/bin/env bash
# Single source of version truth: Cargo.toml package version.
set -euo pipefail
project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$project_dir/Cargo.toml" | head -1)"
if [[ -z "$cargo_version" ]]; then
  echo "Could not read version from Cargo.toml" >&2
  exit 1
fi
spec="$project_dir/packaging/cpn-installer.spec"
tmp="$(mktemp)"
awk -v ver="$cargo_version" '
  BEGIN { updated=0 }
  /^Version:/ { print "Version:        " ver; updated=1; next }
  { print }
  END { if (!updated) exit 2 }
' "$spec" > "$tmp"
mv "$tmp" "$spec"
echo "Synced packaging Version to $cargo_version (from Cargo.toml)"
