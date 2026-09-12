#!/usr/bin/env bash
# Single source of version truth: Cargo.toml package version.
# RPM Version cannot contain '-' (forbidden by rpm). Cargo prereleases like
# 0.2.1-rc1 become Version 0.2.1 and Release 0.1.rc1%{?dist}.
set -euo pipefail
project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$project_dir/Cargo.toml" | head -1 | tr -d '\r')"
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
  # Compact prerelease so Name-Version-Release stays under RPM's 31-char NVR limit.
  # Example: 0.2.2-alpha.1 -> Version 0.2.2, Release 0.alpha1%{?dist}
  pre_compact="${pre//./}"
  rpm_release="0.${pre_compact}%{?dist}"
fi

spec="$project_dir/packaging/cpn-installer.spec"
tmp_in="$(mktemp)"
tmp_out="$(mktemp)"
# Drop UTF-8 BOM if present. Windows editors (and OneDrive) reintroduce it;
# rpmbuild then fails with: Unknown tag: Name
if [[ -s "$spec" ]] && cmp -s <(head -c 3 "$spec") <(printf '\xef\xbb\xbf'); then
  tail -c +4 "$spec" > "$tmp_in"
else
  cat "$spec" > "$tmp_in"
fi
awk -v ver="$rpm_version" -v rel="$rpm_release" '
  BEGIN { v=0; r=0 }
  /^Version:/ { print "Version:        " ver; v=1; next }
  /^Release:/ { print "Release:        " rel; r=1; next }
  { print }
  END { if (!v || !r) exit 2 }
' "$tmp_in" > "$tmp_out"
rm -f "$tmp_in"
mv "$tmp_out" "$spec"
echo "Synced packaging Version=$rpm_version Release=$rpm_release (from Cargo.toml $cargo_version)"
