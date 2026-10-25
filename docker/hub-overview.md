# CPN Control Panel Network installer

**GitHub (source, `stable`):** https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/tree/stable

**Docker Hub:** https://hub.docker.com/r/master3395/cpn-installer

AlmaLinux (9, 10) and Ubuntu 26.04 runtime images with **systemd** and the **CPN** (`cpn-installer`) package preinstalled. AlmaLinux 8 has no Hub tag; local build uses `CPN_ALMA_VERSION=8 ./scripts/docker-run.sh`. Built for maintainer and lab installs of [CPN Control Panel Network](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network).

## Status: stable (v1.4.6)

CPN **v1.4.6** is the current stable release (first stable was **v1.0.0**). Keep backups and test upgrades on a staging host before production use.

## Pull

```bash
docker pull master3395/cpn-installer:almalinux9
docker pull master3395/cpn-installer:almalinux10
docker pull master3395/cpn-installer:ubuntu26.04
docker pull master3395/cpn-installer:latest   # same baseline as almalinux9
docker pull master3395/cpn-installer:1.4.6    # semver on the AlmaLinux 9 image
```

| Tag | Base |
|-----|------|
| `almalinux9`, `latest` | AlmaLinux 9 |
| `almalinux10` | AlmaLinux 10 |
| `ubuntu26.04` | Ubuntu 26.04 |

**Ubuntu container tag:** the Docker Ubuntu image is **26.04 only**. The `ubuntu22.04` and `ubuntu24.04` tags were retired from Docker Hub because Docker Scout reports Medium/Low findings in base-distro packages that Canonical has not fixed yet. On Ubuntu 22.04 or 24.04 hosts, use the native **DEB/apt** install from GitHub Releases (`docs/INSTALL.md`), not these container tags. Tracking: issue #353.

On each `v*` Git release tag, the semver (without leading `v`) is also pushed on the `almalinux9` image.

**Rocky Linux** and other EL guests: use native **RPM** installs from GitHub Releases (`docs/INSTALL.md`). There is no separate `rocky9` Hub tag; EL lab installs may use the AlmaLinux runtime images.

## Data persistence (required for pull and recreate)

Without host bind mounts, recreating this container **loses** panel settings, websites, databases, certificates, MFA, and webmail stored inside the container filesystem. Published examples that only mount cgroups are **not** persistence-safe.

Use binds (same layout as `scripts/docker-run.sh` / `docker-compose.hub.yml`). Host dirs: **0755 only** (never `chmod 777`).

```bash
DATA_ROOT=/var/lib/cpn-docker-host
mkdir -p "$DATA_ROOT"/{cpn-data,cpn-home,cpn-mysql,cpn-letsencrypt,cpn-webmail}
chmod 0755 "$DATA_ROOT"/cpn-{data,home,mysql,letsencrypt,webmail}

docker run -d --name cpn-installer \
  --cgroupns=host \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  --tmpfs /run --tmpfs /run/lock \
  -v "$DATA_ROOT/cpn-data:/var/lib/cpn:Z" \
  -v "$DATA_ROOT/cpn-home:/home:Z" \
  -v "$DATA_ROOT/cpn-mysql:/var/lib/mysql:Z" \
  -v "$DATA_ROOT/cpn-letsencrypt:/etc/letsencrypt:Z" \
  -v "$DATA_ROOT/cpn-webmail:/var/lib/cpn-webmail:Z" \
  --cap-add=SYS_ADMIN --cap-add=NET_ADMIN --cap-add=SYS_RESOURCE \
  --cap-add=SYS_NICE --cap-add=SYS_PTRACE --cap-add=AUDIT_WRITE \
  --cap-add=AUDIT_CONTROL --cap-add=SETUID --cap-add=SETGID \
  --cap-add=CHOWN --cap-add=DAC_OVERRIDE --cap-add=FOWNER \
  --cap-add=KILL --cap-add=MKNOD --cap-add=NET_BIND_SERVICE \
  --cap-add=NET_RAW \
  --security-opt apparmor:unconfined --security-opt seccomp:unconfined \
  -p 2087:2087 \
  --stop-signal SIGRTMIN+3 \
  master3395/cpn-installer:almalinux9
```

Compose without a local build (AlmaLinux or Ubuntu Hub tags):

```bash
docker compose -f docker-compose.hub.yml up -d
CPN_HUB_IMAGE=master3395/cpn-installer:ubuntu26.04 \
  docker compose -f docker-compose.hub.yml up -d
```

`/usr/local/lsws`, mail config/queues, and OS account files are **not** in the default binds; see repository `docs/DOCKER-DATA-LAYOUT.md`.

## Security (systemd guest)

This image runs **systemd as PID 1** and is not an ordinary single-process app container. It needs host cgroup namespace access (`--cgroupns=host` and a writable `/sys/fs/cgroup` mount). The default run path uses a **reduced capability set** instead of full `--privileged` (narrower than privileged; still includes `SYS_ADMIN`, unconfined AppArmor/seccomp, and rw cgroups). Set `CPN_DOCKER_PRIVILEGED=1` with `scripts/docker-run.sh`, or pass `--privileged`, only if the reduced set fails; there is **no automatic privileged retry**.

Use dedicated lab or test hosts only. Do not expose port 2087 on untrusted networks.

Prefer native RPM/DEB installs for production hosts (`docs/INSTALL.md`).

See `scripts/docker-run.sh` and `docker-compose.hub.yml` in the repository.
