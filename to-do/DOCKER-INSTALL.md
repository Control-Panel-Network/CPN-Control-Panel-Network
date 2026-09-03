# Docker install for CPN

Date: 03/09/2026

## Why Docker

Native install still targets AlmaLinux 9 and AlmaLinux 10 (RPM via `scripts/build-rpm.sh`). Docker (or Podman) lets you run the same AlmaLinux-based installer on other hosts, including lab VMs and developer machines that are not AlmaLinux.

The installer still requires an AlmaLinux 9 or 10 userland. Docker does not remove that requirement; it provides the AlmaLinux environment in a container.

## Requirements

- Docker or Podman
- Privileged containers with systemd as PID 1
- Host cgroup mount (`/sys/fs/cgroup`)
- Port `8787` free on the host (override with `CPN_PORT`)

## Security warnings

- The container must run **privileged**. That grants broad host access. Use only on dedicated lab or test machines.
- Do not expose `8787` to the public internet. The installer prints a temporary token; treat it as a secret.
- Package installs (`dnf`) and service starts run inside the container guest, not on a locked-down unprivileged sandbox.

## Quick start from Docker Hub

Published images (built from the same `Dockerfile` after `scripts/build-rpm.sh` / `scripts/docker-build-rpm.sh`):

- https://hub.docker.com/r/master3395/cpn-installer
- Tags: `almalinux9`, `almalinux10`, and `latest` (tracks `almalinux10`)

```bash
# Pull AlmaLinux 9 or 10
docker pull master3395/cpn-installer:almalinux9
docker pull master3395/cpn-installer:almalinux10

# Run (privileged + systemd; same flags as scripts/docker-run.sh)
docker run -d --privileged --cgroupns=host --name cpn-installer \
  -p 8787:8787 \
  --tmpfs /run --tmpfs /run/lock \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  -e container=docker \
  --stop-signal SIGRTMIN+3 \
  master3395/cpn-installer:almalinux9 \
  /usr/lib/systemd/systemd

docker exec cpn-installer systemctl restart cpn-installer
docker exec cpn-installer journalctl -u cpn-installer -n 50 --no-pager
```

With Podman, replace `docker` with `podman`. Open `http://127.0.0.1:8787/` and check the journal for the `?token=` URL.

## Quick start (build locally from this repo)

From the repository root on a machine with Docker or Podman:

```bash
# AlmaLinux 9 image (default)
./scripts/docker-run.sh

# AlmaLinux 10 image
CPN_ALMA_VERSION=10 ./scripts/docker-run.sh

# Custom host port
CPN_PORT=8788 ./scripts/docker-run.sh
```

If no RPM exists under `target/rpmbuild/RPMS/`, `docker-run.sh` calls `scripts/docker-build-rpm.sh` first (builds the RPM inside AlmaLinux).

Open the printed URL (includes `?token=...`) in a browser.

## Build RPM only (any host)

```bash
./scripts/docker-build-rpm.sh
CPN_ALMA_VERSION=10 ./scripts/docker-build-rpm.sh
```

Native AlmaLinux hosts can still use `./scripts/build-rpm.sh` without Docker.

## Manual docker build / run

```bash
# 1) Produce cpn-installer.rpm (native or via docker-build-rpm.sh)
cp target/rpmbuild/RPMS/x86_64/cpn-installer-*.rpm ./cpn-installer.rpm

# 2) Build runtime image
docker build --build-arg ALMA_VERSION=9 -t cpn-installer:el9 .

# 3) Run with systemd (same flags as tests/docker-matrix.sh)
docker run -d --privileged --cgroupns=host --name cpn-installer \
  -p 8787:8787 \
  --tmpfs /run --tmpfs /run/lock \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  -e container=docker \
  --stop-signal SIGRTMIN+3 \
  cpn-installer:el9 \
  /usr/lib/systemd/systemd

docker exec cpn-installer systemctl restart cpn-installer
docker exec cpn-installer journalctl -u cpn-installer -n 50 --no-pager
```

## Compose

Place `cpn-installer.rpm` in the repo root, then:

```bash
export CPN_ALMA_VERSION=9   # or 10
docker compose build
docker compose up -d
```

Compose also uses `privileged: true` and mounts `/sys/fs/cgroup`. Prefer `scripts/docker-run.sh` when possible (prints the token URL).

## Environment variables

| Variable | Default | Purpose |
|---|---|---|
| `CPN_ALMA_VERSION` | `9` | AlmaLinux major (`9` or `10`) |
| `CPN_PORT` | `8787` | Host port mapped to container `8787` |
| `CPN_CONTAINER_NAME` | `cpn-installer` | Container name |
| `CPN_CONTAINER_ENGINE` | auto (`docker` or `podman`) | Force engine |
| `CPN_RPM_PATH` | auto-discover | Path to an existing RPM |
| `CPN_SKIP_BUILD_RPM` | `0` | Set `1` to fail instead of building RPM |
| `CPN_BUILD_IMAGE` | `almalinux:$CPN_ALMA_VERSION` | Override build base image |
| `CPN_TEST_IMAGE` | `almalinux:9.8` | Used by `tests/docker-matrix.sh` |

## Functional matrix

Existing privileged systemd matrix (unchanged):

```bash
./tests/docker-matrix.sh
CPN_TEST_IMAGE=almalinux:10 ./tests/docker-matrix.sh
```

## Limitations

- Privileged + systemd is mandatory for install recipes that call `systemctl` and `dnf`.
- Building the RPM still needs AlmaLinux (native or via `docker-build-rpm.sh`).
- Windows Docker Desktop may not provide a full Linux cgroup/systemd experience; prefer Podman or Docker on a Linux AlmaLinux 9/10 lab VM.
- Nested container labs (Docker-in-Docker) can be fragile; verify with `systemctl is-system-running` inside the guest.
- Native RPM install on AlmaLinux 9/10 remains fully supported and is not removed.
