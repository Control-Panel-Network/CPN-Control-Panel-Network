#!/usr/bin/env bash
# Build the cpn-installer RPM inside an AlmaLinux 8, 9, or 10 container.
# End users should install release assets; this helper is for maintainers/development.
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
alma_version="${CPN_ALMA_VERSION:-9}"
image="${CPN_BUILD_IMAGE:-almalinux:${alma_version}}"
container_engine="${CPN_CONTAINER_ENGINE:-}"

if [[ "$alma_version" != "8" && "$alma_version" != "9" && "$alma_version" != "10" ]]; then
  echo "CPN_ALMA_VERSION must be 8, 9, or 10 (got: ${alma_version})." >&2
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
    # AlmaLinux images may ship curl-minimal; --allowerasing avoids a curl conflict.
    dnf -y install --allowerasing \
      ca-certificates gcc gcc-c++ make openssl-devel \
      rpm-build rpmdevtools git which hostname
    dnf clean all

    node_major() {
      node -v 2>/dev/null | tr -d v | cut -d. -f1
    }

    # Prefer the distribution Node.js first. EL10 currently ships Node 22 in AppStream,
    # which avoids relying on a third-party repository that may lag a new EL major.
    if ! command -v node >/dev/null 2>&1 || [[ "$(node_major)" -lt 20 ]]; then
      dnf -y install nodejs npm || true
    fi

    # EL8/9 may still expose an older AppStream Node. Use NodeSource only as a fallback.
    if ! command -v node >/dev/null 2>&1 || [[ "$(node_major)" -lt 20 ]]; then
      if [[ "${CPN_ALMA_VERSION}" == "10" ]]; then
        echo "EL10 did not provide Node.js >=20 from configured distro repositories." >&2
        exit 1
      fi
      dnf -y remove nodejs npm >/dev/null 2>&1 || true
      curl -fsSL https://rpm.nodesource.com/setup_22.x | bash -
      dnf -y install nodejs
    fi

    if [[ "$(node_major)" -lt 20 ]]; then
      echo "Node.js >=20 is required to build installer-ui; got $(node -v 2>/dev/null || echo missing)." >&2
      exit 1
    fi

    # Reassert the native compiler after repository/package transitions.
    dnf -y install gcc gcc-c++ make
    if ! command -v cc >/dev/null 2>&1 && ! command -v gcc >/dev/null 2>&1; then
      echo "gcc/cc missing after package install" >&2
      exit 1
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
