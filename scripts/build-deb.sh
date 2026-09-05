#!/usr/bin/env bash
# Experimental .deb packaging for supported apt-family guests.
# End users should install release assets; this script is for maintainers/development.
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
  echo "For RHEL-family development builds use ./scripts/build-rpm.sh or ./scripts/docker-build-rpm.sh." >&2
  exit 1
fi

if [[ "${ID}" == "ubuntu" ]] && [[ "$major" != "22" && "$major" != "24" ]]; then
  echo "Unsupported Ubuntu major ${major}. Maintainer .deb builds target Ubuntu 22.04/24.04." >&2
  exit 1
fi
if [[ "${ID}" == "debian" ]] && [[ "$major" != "12" && "$major" != "13" ]]; then
  echo "Unsupported Debian major ${major}. Maintainer .deb builds target Debian 12/13." >&2
  exit 1
fi

cd "$ui_dir"
npm ci
npm run bundle

cd "$project_dir"
bash "$project_dir/scripts/sync-version.sh"
cargo build --release --locked

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
arch="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
pkg_root="$out_dir/cpn-installer_${version}_$arch"
rm -rf "$pkg_root"
mkdir -p "$pkg_root/DEBIAN" "$pkg_root/usr/bin" "$pkg_root/lib/systemd/system"

install -m 0755 target/release/cpn-installer "$pkg_root/usr/bin/cpn-installer"
install -m 0755 target/release/cpn "$pkg_root/usr/bin/cpn"
install -m 0644 packaging/cpn-installer.service "$pkg_root/lib/systemd/system/cpn-installer.service"

cat >"$pkg_root/DEBIAN/control" <<EOF
Package: cpn-installer
Version: ${version}
Section: admin
Priority: optional
Architecture: ${arch}
Maintainer: CPN contributors <noreply@github.com>
Depends: systemd, curl, ca-certificates
Homepage: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network
Description: CPN Control Panel Network web installer
 Experimental CPN installer package for supported Ubuntu and Debian guests.
 Includes the installer, operator CLI, and systemd unit.
EOF

cat >"$pkg_root/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v systemctl >/dev/null 2>&1; then
  systemctl daemon-reload >/dev/null 2>&1 || true
fi
exit 0
EOF
chmod 0755 "$pkg_root/DEBIAN/postinst"

cat >"$pkg_root/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
if command -v systemctl >/dev/null 2>&1; then
  systemctl daemon-reload >/dev/null 2>&1 || true
fi
exit 0
EOF
chmod 0755 "$pkg_root/DEBIAN/postrm"

dpkg-deb --build "$pkg_root" "$out_dir/cpn-installer_${version}_${arch}.deb"
find "$out_dir" -type f -name '*.deb' -print
