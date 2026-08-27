#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ui_dir="$project_dir/installer-ui"
panel_dir="$project_dir/Panel"
rpm_root="$project_dir/target/rpmbuild"
node_version="22.23.2"
node_archive="node-v${node_version}-linux-x64.tar.xz"
node_sha256="d60acfe00a2932254bb0ad20e01b0d74397a0875595de719654b214f4b03f307"
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$project_dir/Cargo.toml" | head -1)"
test -n "$version"
if [[ ! -f "$project_dir/Cargo.lock" ]]; then
  echo "Falta Cargo.lock; el RPM solo puede construirse con dependencias bloqueadas." >&2
  exit 1
fi

if [[ ! -f /etc/almalinux-release ]]; then
  echo "Este empaquetado debe ejecutarse dentro de AlmaLinux 9." >&2
  exit 1
fi

cd "$ui_dir"
npm ci
npm run bundle

cd "$panel_dir"
npm ci
CLOUDFLARE_OAUTH_CLIENT_ID= \
CLOUDFLARE_OAUTH_CLIENT_SECRET= \
CLOUDFLARE_OAUTH_SCOPES= \
npm run build

panel_stage="$(mktemp -d)"
node_stage="$(mktemp -d)"
cleanup() { rm -rf -- "$panel_stage" "$node_stage"; }
trap cleanup EXIT
cp -a .next/standalone/. "$panel_stage/"
mkdir -p "$panel_stage/.next"
cp -a .next/static "$panel_stage/.next/static"
if [[ -d public ]]; then cp -a public "$panel_stage/public"; fi
mkdir -p "$panel_stage/.next/cache"

cd "$project_dir"
cargo build --release --locked

mkdir -p "$rpm_root"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
install -m 0755 target/release/cpn-installer "$rpm_root/SOURCES/cpn-installer"
tar -czf "$rpm_root/SOURCES/cpn-panel.tar.gz" -C "$panel_stage" .
if [[ ! -f "$rpm_root/SOURCES/$node_archive" ]]; then
  curl --fail --location --proto '=https' --proto-redir '=https' \
    --output "$rpm_root/SOURCES/$node_archive" \
    "https://nodejs.org/dist/v${node_version}/${node_archive}"
fi
echo "$node_sha256  $rpm_root/SOURCES/$node_archive" | sha256sum --check --status
tar -xJf "$rpm_root/SOURCES/$node_archive" -C "$node_stage" --strip-components=1 \
  "node-v${node_version}-linux-x64/bin/node" \
  "node-v${node_version}-linux-x64/LICENSE"
tar -cJf "$rpm_root/SOURCES/node-runtime.tar.xz" -C "$node_stage" bin/node LICENSE
install -m 0644 packaging/cpn-panel.service "$rpm_root/SOURCES/cpn-panel.service"
install -m 0644 packaging/cpn-installer.spec "$rpm_root/SPECS/cpn-installer.spec"
rpmbuild --define "_topdir $rpm_root" --define "cpn_version $version" -bb "$rpm_root/SPECS/cpn-installer.spec"

find "$rpm_root/RPMS" -type f -name '*.rpm' -print
