#!/usr/bin/env bash
# Fail if packaging / tag version drifts from Cargo.toml (issue #17).
# Cargo may use prerelease suffixes (0.2.1-rc1); RPM Version is the numeric
# prefix and Release carries the prerelease token (see sync-version.sh).
set -euo pipefail
project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$project_dir/Cargo.toml" | head -1)"
if [[ -z "$cargo_version" ]]; then
  echo "Could not read version from Cargo.toml" >&2
  exit 1
fi

expected_rpm_version="$cargo_version"
expected_rpm_release="1%{?dist}"
if [[ "$cargo_version" == *-* ]]; then
  expected_rpm_version="${cargo_version%%-*}"
  pre="${cargo_version#*-}"
  expected_rpm_release="0.1.${pre}%{?dist}"
fi

spec_version="$(awk '/^Version:/ { print $2; exit }' "$project_dir/packaging/cpn-installer.spec")"
spec_release="$(awk '/^Release:/ { print $2; exit }' "$project_dir/packaging/cpn-installer.spec")"
if [[ "$spec_version" != "$expected_rpm_version" || "$spec_release" != "$expected_rpm_release" ]]; then
  echo "Version drift: Cargo.toml=$cargo_version" >&2
  echo "  expected RPM Version=$expected_rpm_version Release=$expected_rpm_release" >&2
  echo "  actual   RPM Version=$spec_version Release=$spec_release" >&2
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
echo "Version sync OK: cargo=$cargo_version rpm=$spec_version-$spec_release"
