#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ui_dir="$project_dir/installer-ui"
rpm_root="$project_dir/target/rpmbuild"

if [[ ! -f /etc/os-release ]]; then
  echo "No se pudo identificar el sistema operativo (/etc/os-release)." >&2
  exit 1
fi

# shellcheck disable=SC1091
source /etc/os-release
major="${VERSION_ID%%.*}"
# RHEL-family RPM build hosts (CyberPanel-aligned EL guests).
allowed_ids=(almalinux rocky rhel centos cloudlinux)
id_ok=0
for candidate in "${allowed_ids[@]}"; do
  if [[ "${ID:-}" == "$candidate" ]]; then id_ok=1; break; fi
done
if [[ "$id_ok" -ne 1 ]] || [[ "$major" != "8" && "$major" != "9" && "$major" != "10" ]]; then
  echo "Este empaquetado RPM debe ejecutarse en AlmaLinux/Rocky/RHEL/CentOS/CloudLinux 8-10 (detectado: ID=${ID:-unknown} VERSION_ID=${VERSION_ID:-unknown})." >&2
  echo "En otros hosts, construye el RPM con: ./scripts/docker-build-rpm.sh" >&2
  echo "En Ubuntu, prueba el helper experimental: ./scripts/build-deb.sh" >&2
  echo "Matriz: to-do/OS-SUPPORT-MATRIX.md" >&2
  exit 1
fi

cd "$ui_dir"
npm ci
npm run bundle

cd "$project_dir"
bash "$project_dir/scripts/sync-version.sh"
cargo build --release

mkdir -p "$rpm_root"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
install -m 0755 target/release/cpn-installer "$rpm_root/SOURCES/cpn-installer"
install -m 0755 target/release/cpn "$rpm_root/SOURCES/cpn"
install -m 0644 packaging/cpn-installer.spec "$rpm_root/SPECS/cpn-installer.spec"
rpmbuild --define "_topdir $rpm_root" -bb "$rpm_root/SPECS/cpn-installer.spec"

find "$rpm_root/RPMS" -type f -name '*.rpm' -print
