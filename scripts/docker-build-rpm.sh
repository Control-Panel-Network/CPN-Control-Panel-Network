#!/usr/bin/env bash
# Build the cpn-installer RPM inside an AlmaLinux 9 or 10 container.
# Use this when the host is not AlmaLinux (for example Windows or Ubuntu).
# Native path on AlmaLinux remains: ./scripts/build-rpm.sh
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
alma_version="${CPN_ALMA_VERSION:-9}"
image="${CPN_BUILD_IMAGE:-almalinux:${alma_version}}"
container_engine="${CPN_CONTAINER_ENGINE:-}"

if [[ "$alma_version" != "9" && "$alma_version" != "10" ]]; then
  echo "CPN_ALMA_VERSION must be 9 or 10 (got: ${alma_version})." >&2
  exit 1
fi

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
echo "[cpn] Building RPM inside ${image} using ${engine}..."

# SELinux-friendly bind mount when available (:Z is ignored by Docker Desktop).
mount_opts="rw"
if [[ "$engine" == "podman" ]]; then
  mount_opts="rw,Z"
fi

"$engine" pull "$image"
"$engine" run --rm \
  -e CPN_ALMA_VERSION="$alma_version" \
  -v "${project_dir}:/src:${mount_opts}" \
  -w /src \
  "$image" \
  bash -lc '
    set -euo pipefail
    dnf -y install \
      curl ca-certificates gcc gcc-c++ make openssl-devel \
      rpm-build rpmdevtools git which hostname \
      && dnf clean all

    # Node 22 (Vite 6 / React 19 in installer-ui).
    if ! command -v node >/dev/null 2>&1 || [[ "$(node -v 2>/dev/null | tr -d v | cut -d. -f1)" -lt 20 ]]; then
      curl -fsSL https://rpm.nodesource.com/setup_22.x | bash -
      dnf -y install nodejs
    fi

    if ! command -v rustc >/dev/null 2>&1; then
      curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    fi
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"

    ./scripts/build-rpm.sh
  '

echo "[cpn] RPM build finished. Artifacts under target/rpmbuild/RPMS/"
find "$project_dir/target/rpmbuild/RPMS" -type f -name "cpn-installer-*.rpm" -print 2>/dev/null || true
