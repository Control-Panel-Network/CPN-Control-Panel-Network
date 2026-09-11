#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ui_dir="$project_dir/installer-ui"
rpm_root="$project_dir/target/rpmbuild"

if [[ ! -f /etc/os-release ]]; then
  echo "Could not identify the operating system (/etc/os-release)." >&2
  exit 1
fi

# shellcheck disable=SC1091
source /etc/os-release
major="${VERSION_ID%%.*}"
# RHEL-family RPM build hosts.
allowed_ids=(almalinux rocky rhel centos cloudlinux)
id_ok=0
for candidate in "${allowed_ids[@]}"; do
  if [[ "${ID:-}" == "$candidate" ]]; then
    id_ok=1
    break
  fi
done
if [[ "$id_ok" -ne 1 ]] || [[ "$major" != "8" && "$major" != "9" && "$major" != "10" ]]; then
  echo "This RPM packaging must run on AlmaLinux/Rocky/RHEL/CentOS/CloudLinux 8-10 (detected: ID=${ID:-unknown} VERSION_ID=${VERSION_ID:-unknown})." >&2
  echo "On other development hosts, use ./scripts/docker-build-rpm.sh." >&2
  echo "End users should install packages published on GitHub Releases." >&2
  exit 1
fi

cd "$ui_dir"
npm ci
npm run bundle

cd "$project_dir"

if [[ ! -f "$project_dir/Cargo.lock" ]]; then
  echo "Cargo.lock missing; refuse unlockable release build (issue #12)." >&2
  exit 1
fi
bash "$project_dir/scripts/sync-version.sh"
cargo build --release --locked

mkdir -p "$rpm_root"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
install -m 0755 target/release/cpn-installer "$rpm_root/SOURCES/cpn-installer"
install -m 0755 target/release/cpn "$rpm_root/SOURCES/cpn"
install -m 0644 packaging/cpn-installer.service "$rpm_root/SOURCES/cpn-installer.service"
install -m 0644 packaging/cpn-installer.spec "$rpm_root/SPECS/cpn-installer.spec"
rpmbuild --define "_topdir $rpm_root" -bb "$rpm_root/SPECS/cpn-installer.spec"

find "$rpm_root/RPMS" -type f -name '*.rpm' -print
