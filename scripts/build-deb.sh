#!/usr/bin/env bash
# Experimental .deb packaging for Ubuntu/Debian guests.
# Native RHEL-family path remains: ./scripts/build-rpm.sh
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ui_dir="$project_dir/installer-ui"
out_dir="$project_dir/target/deb"

if [[ ! -f /etc/os-release ]]; then
  echo "Missing /etc/os-release." >&2
  exit 1
fi

# shellcheck disable=SC1091
source /etc/os-release
major="${VERSION_ID%%.*}"
if [[ "${ID:-}" != "ubuntu" && "${ID:-}" != "debian" ]]; then
  echo "build-deb.sh expects Ubuntu or Debian (detected: ID=${ID:-unknown} VERSION_ID=${VERSION_ID:-unknown})." >&2
  echo "For RHEL-family RPMs use ./scripts/build-rpm.sh or ./scripts/docker-build-rpm.sh" >&2
  exit 1
fi

if [[ "${ID}" == "ubuntu" ]] && [[ "$major" != "20" && "$major" != "22" && "$major" != "24" ]]; then
  echo "Unsupported Ubuntu major ${major}. Supported for packaging experiments: 20, 22, 24." >&2
  exit 1
fi

cd "$ui_dir"
npm ci
npm run bundle

cd "$project_dir"
bash "$project_dir/scripts/sync-version.sh"
cargo build --release

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
arch="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
pkg_root="$out_dir/cpn-installer_${version}_$arch"
rm -rf "$pkg_root"
mkdir -p "$pkg_root/DEBIAN" "$pkg_root/usr/bin"

install -m 0755 target/release/cpn-installer "$pkg_root/usr/bin/cpn-installer"
cat >"$pkg_root/DEBIAN/control" <<EOF
Package: cpn-installer
Version: ${version}
Section: admin
Priority: optional
Architecture: ${arch}
Maintainer: CPN <dev@cpn.invalid>
Depends: systemd, curl, ca-certificates
Description: CPN Server Panel web installer (Ubuntu/Debian experimental package)
 Experimental .deb of the Linux installer. See to-do/OS-SUPPORT-MATRIX.md.
EOF

dpkg-deb --build "$pkg_root" "$out_dir/cpn-installer_${version}_${arch}.deb"
find "$out_dir" -type f -name '*.deb' -print
