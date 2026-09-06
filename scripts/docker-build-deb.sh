#!/usr/bin/env bash
# Build the cpn-installer .deb inside an Ubuntu container (any host with Docker/Podman).
# Native path on Ubuntu/Debian remains: ./scripts/build-deb.sh
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ubuntu_version="${CPN_UBUNTU_VERSION:-22.04}"
image="${CPN_BUILD_IMAGE:-ubuntu:${ubuntu_version}}"
container_engine="${CPN_CONTAINER_ENGINE:-}"

detect_engine() {
  if [[ -n "$container_engine" ]]; then
    echo "$container_engine"
    return
  fi
  if command -v docker >/dev/null 2>&1; then
    echo docker
    return
  fi
  if command -v podman >/dev/null 2>&1; then
    echo podman
    return
  fi
  echo "Neither docker nor podman was found in PATH." >&2
  exit 1
}

engine="$(detect_engine)"
echo "[cpn] Building .deb inside ${image} using ${engine}..."

mount_opts="rw"
if [[ "$engine" == "podman" ]]; then
  mount_opts="rw,Z"
fi

"$engine" pull "$image"
"$engine" run --rm \
  -e DEBIAN_FRONTEND=noninteractive \
  -v "${project_dir}:/src:${mount_opts}" \
  -w /src \
  "$image" \
  bash -lc '
    set -euo pipefail
    apt-get update -y
    apt-get install -y ca-certificates curl build-essential pkg-config libssl-dev \
      git dpkg-dev
    if ! command -v rustc >/dev/null 2>&1; then
      curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    fi
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
    # Install Node 22 directly. Installing Jammy nodejs/npm first pulls libnode-dev,
    # whose headers conflict with the NodeSource package.
    curl -fsSL https://deb.nodesource.com/setup_22.x | bash -
    apt-get install -y nodejs
    ./scripts/build-deb.sh
  '

echo "[cpn] .deb build finished. Artifacts under target/deb/"
find "$project_dir/target/deb" -type f -name '*.deb' -print 2>/dev/null || true
