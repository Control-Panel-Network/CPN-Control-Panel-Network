#!/usr/bin/env bash
# Build and run the CPN installer in a privileged AlmaLinux + systemd container.
# Pattern matches tests/docker-matrix.sh (privileged, cgroup host, systemd PID 1).
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
alma_version="${CPN_ALMA_VERSION:-9}"
port="${CPN_PORT:-8787}"
name="${CPN_CONTAINER_NAME:-cpn-installer}"
image_tag="cpn-installer:el${alma_version}"
container_engine="${CPN_CONTAINER_ENGINE:-}"
rpm_path="${CPN_RPM_PATH:-}"
skip_build_rpm="${CPN_SKIP_BUILD_RPM:-0}"

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

resolve_rpm() {
  if [[ -n "$rpm_path" && -f "$rpm_path" ]]; then
    echo "$rpm_path"
    return
  fi
  shopt -s nullglob
  local candidates=(
    "$project_dir"/target/rpmbuild/RPMS/x86_64/cpn-installer-*.rpm
    "$project_dir"/target/rpmbuild/RPMS/aarch64/cpn-installer-*.rpm
    "$project_dir"/cpn-installer.rpm
  )
  if ((${#candidates[@]} == 0)); then
    return 1
  fi
  # Prefer newest by mtime.
  local newest=""
  local newest_mtime=0
  local candidate mtime
  for candidate in "${candidates[@]}"; do
    mtime="$(stat -c %Y "$candidate" 2>/dev/null || stat -f %m "$candidate")"
    if ((mtime >= newest_mtime)); then
      newest_mtime="$mtime"
      newest="$candidate"
    fi
  done
  echo "$newest"
}

engine="$(detect_engine)"
cd "$project_dir"

if ! rpm_path="$(resolve_rpm)"; then
  if [[ "$skip_build_rpm" == "1" ]]; then
    echo "No cpn-installer RPM found and CPN_SKIP_BUILD_RPM=1." >&2
    exit 1
  fi
  echo "[cpn] No RPM found; building inside AlmaLinux ${alma_version}..."
  CPN_ALMA_VERSION="$alma_version" CPN_CONTAINER_ENGINE="$engine" \
    bash "$project_dir/scripts/docker-build-rpm.sh"
  rpm_path="$(resolve_rpm)" || {
    echo "RPM still missing after docker-build-rpm.sh." >&2
    exit 1
  }
fi

echo "[cpn] Using RPM: $rpm_path"
cp -f "$rpm_path" "$project_dir/cpn-installer.rpm"

echo "[cpn] Building image ${image_tag} (AlmaLinux ${alma_version})..."
"$engine" build \
  --build-arg "ALMA_VERSION=${alma_version}" \
  -t "$image_tag" \
  -f "$project_dir/Dockerfile" \
  "$project_dir"

"$engine" rm -f "$name" >/dev/null 2>&1 || true

echo "[cpn] Starting privileged systemd container ${name} on port ${port}..."
# Warning: privileged is required for systemd, dnf installs, and service management.
"$engine" run -d \
  --privileged \
  --cgroupns=host \
  --name "$name" \
  --hostname "$name" \
  -p "${port}:8787" \
  --tmpfs /run \
  --tmpfs /run/lock \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  -e container=docker \
  --stop-signal SIGRTMIN+3 \
  "$image_tag" \
  /usr/lib/systemd/systemd >/dev/null

echo "[cpn] Waiting for systemd..."
for _ in {1..45}; do
  if "$engine" exec "$name" systemctl is-system-running --quiet 2>/dev/null \
    || "$engine" exec "$name" systemctl is-system-running 2>/dev/null | grep -Eq 'running|degraded'; then
    break
  fi
  sleep 1
done

"$engine" exec "$name" systemctl restart cpn-installer.service >/dev/null 2>&1 || true
sleep 2

token=""
for _ in {1..30}; do
  token="$("$engine" exec "$name" journalctl -u cpn-installer --no-pager -n 40 2>/dev/null \
    | sed -n 's/.*token=\([[:alnum:]]*\).*/\1/p' | tail -1 || true)"
  if [[ -n "$token" ]]; then
    break
  fi
  sleep 1
done

echo
echo "[cpn] Container ${name} is running (${image_tag})."
echo "[cpn] Installer UI: http://127.0.0.1:${port}/"
if [[ -n "$token" ]]; then
  echo "[cpn] Open with token: http://127.0.0.1:${port}/?token=${token}"
  echo "[cpn] Login page:     http://127.0.0.1:${port}/login?token=${token}"
else
  echo "[cpn] Token not found yet. Check logs:"
  echo "       ${engine} logs -f ${name}"
  echo "       ${engine} exec ${name} journalctl -u cpn-installer -f"
fi
echo
echo "[cpn] Security: this container runs privileged and can change the guest OS."
echo "[cpn] Do not publish port ${port} on untrusted networks."
echo "[cpn] Stop: ${engine} rm -f ${name}"
