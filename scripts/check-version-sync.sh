#!/usr/bin/env bash
# Fail if packaging / tag version drifts from Cargo.toml (issue #17).
set -euo pipefail
project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$project_dir/Cargo.toml" | head -1)"
if [[ -z "$cargo_version" ]]; then
  echo "Could not read version from Cargo.toml" >&2
  exit 1
fi
spec_version="$(awk '/^Version:/ { print $2; exit }' "$project_dir/packaging/cpn-installer.spec")"
if [[ "$spec_version" != "$cargo_version" ]]; then
  echo "Version drift: Cargo.toml=$cargo_version packaging/cpn-installer.spec=$spec_version" >&2
  echo "Run: ./scripts/sync-version.sh" >&2
  exit 1
fi
if [[ "${GITHUB_REF_TYPE:-}" == "tag" && "${GITHUB_REF_NAME:-}" == v* ]]; then
  tag="${GITHUB_REF_NAME#v}"
  if [[ "$tag" != "$cargo_version" ]]; then
    echo "Tag $GITHUB_REF_NAME does not match Cargo.toml version $cargo_version" >&2
    exit 1
  fi
fi
echo "Version sync OK: $cargo_version"
