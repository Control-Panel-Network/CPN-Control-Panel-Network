#!/usr/bin/env bash
# Single source of version truth: Cargo.toml package version.
# RPM Version cannot contain '-' (forbidden by rpm). Cargo prereleases like
# 0.2.1-rc1 become Version 0.2.1 and Release 0.1.rc1%{?dist}.
set -euo pipefail
project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$project_dir/Cargo.toml" | head -1)"
if [[ -z "$cargo_version" ]]; then
  echo "Could not read version from Cargo.toml" >&2
  exit 1
fi

rpm_version="$cargo_version"
rpm_release="1%{?dist}"
if [[ "$cargo_version" == *-* ]]; then
  rpm_version="${cargo_version%%-*}"
  pre="${cargo_version#*-}"
  # Keep prereleases sortable before the final "1" release of the same Version.
  rpm_release="0.1.${pre}%{?dist}"
fi

spec="$project_dir/packaging/cpn-installer.spec"
tmp="$(mktemp)"
awk -v ver="$rpm_version" -v rel="$rpm_release" '
  BEGIN { v=0; r=0 }
  /^Version:/ { print "Version:        " ver; v=1; next }
  /^Release:/ { print "Release:        " rel; r=1; next }
  { print }
  END { if (!v || !r) exit 2 }
' "$spec" > "$tmp"
mv "$tmp" "$spec"
echo "Synced packaging Version=$rpm_version Release=$rpm_release (from Cargo.toml $cargo_version)"
