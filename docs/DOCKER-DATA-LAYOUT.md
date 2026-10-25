# CPN Docker data layout and persistence

This page answers how CPN stores data, when **pull and recreate** is safe, and why Hub installer containers need broad host access.

Prefer **native RPM/DEB** on production hosts ([INSTALL.md](INSTALL.md)). Hub images are for lab and maintainer installs.

## Two Docker stories

| Story | What it is | Pull and recreate |
| --- | --- | --- |
| **A. Hub / `cpn-installer` container** | Whole panel as a systemd guest (`master3395/cpn-installer`) | Safe **only** when host bind mounts below are used. Without them, recreate wipes panel settings, sites, databases, certificates, MFA, and webmail inside the container. |
| **B. In-panel Compose stacks** | Host package manages apps under `/var/lib/cpn/docker*` | Safe. Stacks bind host data under `/var/lib/cpn/docker-data/<stack>/data`. UI **Pull & Recreate** and upgrade `--bypass` refresh images and keep those binds. |

Create Container (one-off) keeps volumes **optional**. Without a host bind, recreate loses container filesystem data. Prefer Compose Stacks for lasting data.

## Path map (native host or bind targets)

Defaults from panel paths (`CPN_DATA_DIR` overrides the data root):

| Concern | Default path |
| --- | --- |
| Panel settings, site registry, MFA, CPN SSL, plugins, Docker metadata | `/var/lib/cpn` |
| MFA / passkeys | `/var/lib/cpn/mfa/` |
| CPN SSL material | `/var/lib/cpn/ssl/<domain>/` |
| Website docroots | `/home/<domain>/public_html` |
| Host MariaDB (when installed in the guest) | `/var/lib/mysql` |
| Let's Encrypt | `/etc/letsencrypt` |
| Webmail data | `/var/lib/cpn-webmail` |
| Managed compose projects | `/var/lib/cpn/docker/<stack>/` |
| Managed stack app data | `/var/lib/cpn/docker-data/<stack>/data` |

Native package upgrade already preserves those trees on the host. A Hub panel container only preserves them when they are **bind-mounted from the Docker host**.

OLS / LiteSpeed trees under `/usr/local/lsws` are not in the default bind set. Add an extra bind if you need that layer to survive recreate.

## Hub panel container: persistent mounts

Host data root used by `scripts/docker-run.sh` and `docker-compose.yml` (override with `CPN_DOCKER_DATA_ROOT`):

```text
/var/lib/cpn-docker-host/
  cpn-data/        -> /var/lib/cpn
  cpn-home/        -> /home
  cpn-mysql/       -> /var/lib/mysql
  cpn-letsencrypt/ -> /etc/letsencrypt
  cpn-webmail/     -> /var/lib/cpn-webmail
```

Example (same binds as the helper):

```bash
DATA_ROOT=/var/lib/cpn-docker-host
mkdir -p "$DATA_ROOT"/{cpn-data,cpn-home,cpn-mysql,cpn-letsencrypt,cpn-webmail}
# Writable for rootless engines; :Z on -v lines is required on SELinux Enforcing hosts:
chmod 777 "$DATA_ROOT"/cpn-{data,home,mysql,letsencrypt,webmail}

docker pull master3395/cpn-installer:almalinux9
docker rm -f cpn-installer 2>/dev/null || true

docker run -d --name cpn-installer \
  --cgroupns=host \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  --tmpfs /run --tmpfs /run/lock \
  -v "$DATA_ROOT/cpn-data:/var/lib/cpn:Z" \
  -v "$DATA_ROOT/cpn-home:/home:Z" \
  -v "$DATA_ROOT/cpn-mysql:/var/lib/mysql:Z" \
  -v "$DATA_ROOT/cpn-letsencrypt:/etc/letsencrypt:Z" \
  -v "$DATA_ROOT/cpn-webmail:/var/lib/cpn-webmail:Z" \
  -p 2087:2087 \
  --stop-signal SIGRTMIN+3 \
  # plus security flags from the section below \
  master3395/cpn-installer:almalinux9
```

Without those `-v` binds, do **not** assume pull and recreate keeps settings, websites, databases, or certificates.

### Marker check (persistence)

With the binds above:

1. Write a file on the host under `$DATA_ROOT/cpn-data/cpn-persist-marker`.
2. Recreate the container with the **same** binds.
3. Confirm the marker is still present on the host (and visible at `/var/lib/cpn/cpn-persist-marker` inside the new container).

## Security: systemd guest access

Hub installer images run **systemd as PID 1** and perform package installs / `systemctl` for lab guests. That is broader access than an ordinary single-process app container.

### Runtime flags

- Keep `--cgroupns=host` and a writable `/sys/fs/cgroup` mount (required for systemd in this design).
- Keep `/run` and `/run/lock` tmpfs and `STOPSIGNAL SIGRTMIN+3`.
- **Privileged vs reduced capabilities:** default in [`scripts/docker-run.sh`](../scripts/docker-run.sh) and [`docker-compose.yml`](../docker-compose.yml) is a **reduced `--cap-add` set** (verified on AlmaLinux 10 with `master3395/cpn-installer:almalinux10`: systemd `degraded`, `cpn-installer` active). Full `--privileged` grants substantially broader host access (effectively root-equivalent on the Docker/Podman host). Use `CPN_DOCKER_PRIVILEGED=1` or `--privileged` only if the reduced set fails on your engine. `docker-run.sh` auto-falls back to privileged when systemd does not become ready.
- Use dedicated lab or test hosts only. Do not expose port 2087 on untrusted networks.
- Production panels: install native RPM/DEB (no privileged container required for the panel itself).

Host cgroup read-write remains a deliberate trade-off for systemd PID 1. Hub containers are never claimed to match ordinary app-container isolation.

## In-panel Docker

- **Compose Stacks** (`/docker/stacks`): always bind under `/var/lib/cpn/docker-data/<stack>/data`. Pull & Recreate preserves data.
- **Create Container** (`/docker/create`): volumes optional; suggested bind `/var/lib/cpn/docker-data/<name>/data:/data`. Without a bind, recreate loses data.

## Related

- [INSTALL.md](INSTALL.md) (Hub pull/run, native install, `--bypass`)
- [docker/hub-overview.md](../docker/hub-overview.md) (Docker Hub description source)
- `scripts/docker-run.sh` (local build and run helper)
