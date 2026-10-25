#!/usr/bin/env bash
# Build and run the CPN installer in an AlmaLinux + systemd container.
# Maintainer / lab path (not the normal end-user native installer).
# Persistence and security: docs/DOCKER-DATA-LAYOUT.md
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
alma_version="${CPN_ALMA_VERSION:-9}"
port="${CPN_PORT:-2087}"
name="${CPN_CONTAINER_NAME:-cpn-installer}"
image_tag="cpn-installer:el${alma_version}"
container_engine="${CPN_CONTAINER_ENGINE:-}"
rpm_path="${CPN_RPM_PATH:-}"
skip_build_rpm="${CPN_SKIP_BUILD_RPM:-0}"
data_root="${CPN_DOCKER_DATA_ROOT:-/var/lib/cpn-docker-host}"
# CPN_DOCKER_PRIVILEGED=1 forces full --privileged (fallback). Default: reduced caps.
force_privileged="${CPN_DOCKER_PRIVILEGED:-0}"

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

resolve_rpm() {
  if [[ -n "$rpm_path" ]]; then
    if [[ ! -f "$rpm_path" ]]; then
      echo "CPN_RPM_PATH does not exist: $rpm_path" >&2
      return 1
    fi
    echo "$rpm_path"
    return
  fi

  shopt -s nullglob
  local candidates=(
    "$project_dir"/target/rpmbuild/RPMS/x86_64/cpn-installer-*.el"$alma_version".*.rpm
    "$project_dir"/target/rpmbuild/RPMS/aarch64/cpn-installer-*.el"$alma_version".*.rpm
    "$project_dir"/target/rpmbuild/RPMS/*/cpn-installer-*.el"$alma_version".*.rpm
  )
  if ((${#candidates[@]} == 0)); then
    return 1
  fi

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

ensure_data_dirs() {
  mkdir -p \
    "$data_root/cpn-data" \
    "$data_root/cpn-home" \
    "$data_root/cpn-mysql" \
    "$data_root/cpn-letsencrypt" \
    "$data_root/cpn-webmail"
  # Writable for rootful Docker and rootless Podman UID mapping into the guest.
  chmod 777 \
    "$data_root/cpn-data" \
    "$data_root/cpn-home" \
    "$data_root/cpn-mysql" \
    "$data_root/cpn-letsencrypt" \
    "$data_root/cpn-webmail" 2>/dev/null || true
}

# Reduced capability set (narrower than --privileged). Still needs host cgroup access.
cap_args=(
  --cap-add=SYS_ADMIN
  --cap-add=NET_ADMIN
  --cap-add=SYS_RESOURCE
  --cap-add=SYS_NICE
  --cap-add=SYS_PTRACE
  --cap-add=AUDIT_WRITE
  --cap-add=AUDIT_CONTROL
  --cap-add=SETUID
  --cap-add=SETGID
  --cap-add=CHOWN
  --cap-add=DAC_OVERRIDE
  --cap-add=FOWNER
  --cap-add=KILL
  --cap-add=MKNOD
  --cap-add=NET_BIND_SERVICE
  --cap-add=NET_RAW
  --security-opt=apparmor:unconfined
  --security-opt=seccomp:unconfined
)

engine="$(detect_engine)"
cd "$project_dir"

if ! rpm_path="$(resolve_rpm)"; then
  if [[ "$skip_build_rpm" == "1" ]]; then
    echo "No EL${alma_version} cpn-installer RPM found and CPN_SKIP_BUILD_RPM=1." >&2
    exit 1
  fi
  echo "[cpn] No EL${alma_version} RPM found; building inside AlmaLinux ${alma_version}..."
  CPN_ALMA_VERSION="$alma_version" CPN_CONTAINER_ENGINE="$engine" \
    bash "$project_dir/scripts/docker-build-rpm.sh"
  rpm_path="$(resolve_rpm)" || {
    echo "EL${alma_version} RPM still missing after docker-build-rpm.sh." >&2
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

ensure_data_dirs
"$engine" rm -f "$name" >/dev/null 2>&1 || true

security_mode="reduced-capabilities"
run_security=( "${cap_args[@]}" )
if [[ "$force_privileged" == "1" ]]; then
  security_mode="privileged"
  run_security=( --privileged )
fi

echo "[cpn] Starting systemd container ${name} on port ${port} (${security_mode})..."
echo "[cpn] Data root: ${data_root} (survives pull/recreate when binds are kept)"
"$engine" run -d \
  "${run_security[@]}" \
  --cgroupns=host \
  --name "$name" \
  --hostname "$name" \
  -p "${port}:2087" \
  --tmpfs /run \
  --tmpfs /run/lock \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  -v "${data_root}/cpn-data:/var/lib/cpn:Z" \
  -v "${data_root}/cpn-home:/home:Z" \
  -v "${data_root}/cpn-mysql:/var/lib/mysql:Z" \
  -v "${data_root}/cpn-letsencrypt:/etc/letsencrypt:Z" \
  -v "${data_root}/cpn-webmail:/var/lib/cpn-webmail:Z" \
  -e container=docker \
  --stop-signal SIGRTMIN+3 \
  "$image_tag" \
  /usr/lib/systemd/systemd >/dev/null

echo "[cpn] Waiting for systemd..."
systemd_ok=0
for _ in {1..45}; do
  if "$engine" exec "$name" systemctl is-system-running --quiet 2>/dev/null \
    || "$engine" exec "$name" systemctl is-system-running 2>/dev/null | grep -Eq 'running|degraded'; then
    systemd_ok=1
    break
  fi
  sleep 1
done

if [[ "$systemd_ok" != "1" && "$force_privileged" != "1" ]]; then
  echo "[cpn] Reduced capabilities did not bring systemd up; retrying with --privileged..." >&2
  "$engine" rm -f "$name" >/dev/null 2>&1 || true
  security_mode="privileged (auto-fallback)"
  "$engine" run -d \
    --privileged \
    --cgroupns=host \
    --name "$name" \
    --hostname "$name" \
    -p "${port}:2087" \
    --tmpfs /run \
    --tmpfs /run/lock \
    -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
    -v "${data_root}/cpn-data:/var/lib/cpn:Z" \
    -v "${data_root}/cpn-home:/home:Z" \
    -v "${data_root}/cpn-mysql:/var/lib/mysql:Z" \
    -v "${data_root}/cpn-letsencrypt:/etc/letsencrypt:Z" \
    -v "${data_root}/cpn-webmail:/var/lib/cpn-webmail:Z" \
    -e container=docker \
    --stop-signal SIGRTMIN+3 \
    "$image_tag" \
    /usr/lib/systemd/systemd >/dev/null
  for _ in {1..45}; do
    if "$engine" exec "$name" systemctl is-system-running --quiet 2>/dev/null \
      || "$engine" exec "$name" systemctl is-system-running 2>/dev/null | grep -Eq 'running|degraded'; then
      break
    fi
    sleep 1
  done
fi

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
echo "[cpn] Container ${name} is running (${image_tag}, ${security_mode})."
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
echo "[cpn] Security: ${security_mode}; host cgroup mount required for systemd."
echo "[cpn] Broader host access than an ordinary app container. Lab/test hosts only."
echo "[cpn] Do not publish port ${port} on untrusted networks."
echo "[cpn] Data binds under ${data_root} survive recreate (see docs/DOCKER-DATA-LAYOUT.md)."
echo "[cpn] Stop: ${engine} rm -f ${name}"
