# CPN Docker data layout and persistence

This page answers how CPN stores data, when **pull and recreate** is safe, and why Hub installer containers need broad host access.

Prefer **native RPM/DEB** on production hosts ([INSTALL.md](INSTALL.md)). Hub images are for lab and maintainer installs.

## Automatic outer-image updates

Hub deployments can use the root-owned host updater. This is different from
updating the RPM/DEB inside the guest: it pulls the configured OS stream tag
and recreates the running Compose service when the image digest changes.

The panel container never receives `/var/run/docker.sock`. Version Management
writes a request under the existing `/var/lib/cpn` bind. A host systemd timer
checks that request and the Owner automatic-update setting, validates the
container label, image allowlist, Compose definition, and persistent mounts,
then performs the pull and recreate. Automatic updates remain enabled by
default and use the same 6 hour Owner interval as native installs. The host
timer wakes every 15 minutes so manual requests are picked up promptly.

### One-time host setup

The updater requires root-owned Compose input. Copy the project Compose files
to a protected host directory first:

```bash
sudo install -d -m 700 /etc/cpn/compose-source
sudo install -m 600 docker-compose.hub.yml /etc/cpn/compose-source/
sudo install -m 600 docker-compose.hub.lsws.yml /etc/cpn/compose-source/

# Existing OLS/LiteSpeed guest: migrate its tree before first recreate.
sudo ./scripts/prepare-docker-update-mounts.sh --lsws

# Recreate once with the persistent web server overlay.
sudo CPN_HUB_IMAGE=master3395/cpn-installer:almalinux9 \
  docker compose \
  -f /etc/cpn/compose-source/docker-compose.hub.yml \
  -f /etc/cpn/compose-source/docker-compose.hub.lsws.yml up -d

sudo ./scripts/install-docker-image-updater.sh install \
  --compose-file /etc/cpn/compose-source/docker-compose.hub.yml \
  --compose-file /etc/cpn/compose-source/docker-compose.hub.lsws.yml \
  --image master3395/cpn-installer:almalinux9
```

Use `almalinux10` or `ubuntu26.04` consistently for those streams. The setup
renders a root-owned Compose copy, installs
`cpn-docker-image-update.service` and `.timer`, and writes non-secret status
under `/var/lib/cpn/docker-host-update/`. The host configuration is mode 600.

For a guest with Postfix or Dovecot enabled, preserve its config and queues
before enabling updates:

```bash
sudo install -m 600 docker-compose.hub.mail.yml /etc/cpn/compose-source/
sudo ./scripts/prepare-docker-update-mounts.sh --mail
sudo docker compose \
  -f /etc/cpn/compose-source/docker-compose.hub.yml \
  -f /etc/cpn/compose-source/docker-compose.hub.lsws.yml \
  -f /etc/cpn/compose-source/docker-compose.hub.mail.yml up -d
sudo ./scripts/install-docker-image-updater.sh install \
  --compose-file /etc/cpn/compose-source/docker-compose.hub.yml \
  --compose-file /etc/cpn/compose-source/docker-compose.hub.lsws.yml \
  --compose-file /etc/cpn/compose-source/docker-compose.hub.mail.yml \
  --image master3395/cpn-installer:almalinux9
```

The helper refuses to update when OLS/LiteSpeed or mail is present without
the corresponding persistent mounts. It also rolls back to the prior local
image when the recreated panel service does not become active.

Manual verification:

```bash
sudo systemctl start cpn-docker-image-update.service
sudo systemctl status cpn-docker-image-update.timer
sudo journalctl -u cpn-docker-image-update.service -n 100 --no-pager
docker inspect cpn-installer --format '{{.Config.Image}} {{.Image}}'
```

Version Management supports manual **Upgrade to latest release** for this
deployment. Selected older releases and commit-level upgrades are intentionally
blocked because OS stream tags are published per release, while semver tags
currently cover only the primary AlmaLinux image.

## Two Docker stories

| Story | What it is | Pull and recreate |
| --- | --- | --- |
| **A. Hub / `cpn-installer` container** | Whole panel as a systemd guest (`master3395/cpn-installer`) | Safe **only** when host bind mounts below are used. Without them, recreate wipes panel settings, sites, databases, certificates, MFA, and webmail inside the container. |
| **B. In-panel Compose stacks** | Host package manages apps under `/var/lib/cpn/docker*` | Safe. Stacks bind host data under `/var/lib/cpn/docker-data/<stack>/data`. UI **Pull & Recreate** and upgrade `--bypass` refresh images and keep those binds. |

Create Container (one-off) keeps volumes **optional**. Without a host bind, recreate loses container filesystem data. Prefer Compose Stacks for lasting data.

## Path map (native host or bind targets)

Defaults from panel paths (`CPN_DATA_DIR` overrides the data root):

| Concern | Default path | In default Hub binds? |
| --- | --- | --- |
| Panel settings, site registry, MFA, CPN SSL, plugins, Docker metadata | `/var/lib/cpn` | Yes (`cpn-data`) |
| MFA / passkeys | `/var/lib/cpn/mfa/` | Yes (under `cpn-data`) |
| CPN SSL material | `/var/lib/cpn/ssl/<domain>/` | Yes (under `cpn-data`) |
| Website docroots and mailbox homes under `/home` | `/home/<domain>/...` (site home stays `root:root` `0755` for OpenSSH SFTP chroot; `public_html` is chowned to the web runtime user such as `nobody` so PHP can write; never `chmod 777`) | Yes (`cpn-home`) |
| Host MariaDB (when installed in the guest) | `/var/lib/mysql` | Yes (`cpn-mysql`) |
| Let's Encrypt | `/etc/letsencrypt` | Yes (`cpn-letsencrypt`) |
| Webmail app data | `/var/lib/cpn-webmail` | Yes (`cpn-webmail`) |
| Managed compose projects | `/var/lib/cpn/docker/<stack>/` | Yes (under `cpn-data`) |
| Managed stack app data | `/var/lib/cpn/docker-data/<stack>/data` | Yes (under `cpn-data`) |
| OLS / LiteSpeed tree | `/usr/local/lsws` | **No** (optional; see below) |
| Mail MTA/MDA config | `/etc/postfix`, `/etc/dovecot`, … | **No** |
| Mail queues / spool | `/var/spool/postfix`, `/var/mail`, … | **No** |
| System account databases | `/etc/passwd`, `/etc/shadow`, `/etc/group`, `/etc/gshadow` | **No** |

Native package upgrade already preserves those trees on the host. A Hub panel container only preserves paths that are **bind-mounted from the Docker host**.

### Optional: LiteSpeed / OLS

`/usr/local/lsws` is outside the default mounts. Vhost configs, serial, and logs there are lost on recreate unless you add a bind.

Helper: `CPN_DOCKER_BIND_LSWS=1 ./scripts/docker-run.sh` mounts `$DATA_ROOT/cpn-lsws` → `/usr/local/lsws`.

Compose Hub overlay: [`docker-compose.hub.lsws.yml`](../docker-compose.hub.lsws.yml) with `-f docker-compose.hub.yml -f docker-compose.hub.lsws.yml`.

### Mail and system users inside a Hub container

If Postfix/Dovecot (or similar) run **inside** the Hub guest:

- Mailbox files under `/home` are covered by `cpn-home`.
- **Config**, **queues**, and **system users** created for mailboxes are **not** in the default bind set. Recreate can drop MTA config, spool, and OS accounts even when `/home` and `/var/lib/cpn` survive.
- Binding `/etc/passwd` (and friends) from the host is awkward and easy to get wrong; do **not** treat Hub as a production mail host.
- For lasting mail, prefer **native RPM/DEB** on the host, or add explicit extra binds only after you understand the trade-offs (config + spool at minimum).

## Hub panel container: persistent mounts

Host data root used by `scripts/docker-run.sh`, `docker-compose.yml`, and `docker-compose.hub.yml` (override with `CPN_DOCKER_DATA_ROOT`):

```text
/var/lib/cpn-docker-host/
  cpn-data/        -> /var/lib/cpn
  cpn-home/        -> /home
  cpn-mysql/       -> /var/lib/mysql
  cpn-letsencrypt/ -> /etc/letsencrypt
  cpn-webmail/     -> /var/lib/cpn-webmail
  cpn-lsws/        -> /usr/local/lsws   (optional)
```

**Permissions:** create dirs with mode **0755**. Never use `chmod 777` (or world-writable modes) on these paths.

### Pre-built Hub image (Compose, no local build)

```bash
# AlmaLinux 9 (default image in the file)
docker compose -f docker-compose.hub.yml up -d

# AlmaLinux 10 Hub tag
CPN_HUB_IMAGE=master3395/cpn-installer:almalinux10 \
  docker compose -f docker-compose.hub.yml up -d

# Ubuntu 26.04 Hub tag (same binds, 0755, reduced caps; no weaker defaults)
CPN_HUB_IMAGE=master3395/cpn-installer:ubuntu26.04 \
  docker compose -f docker-compose.hub.yml up -d

# Optional OLS/LiteSpeed bind
docker compose -f docker-compose.hub.yml -f docker-compose.hub.lsws.yml up -d
```

Local RPM build path (AlmaLinux 8/9/10): [`docker-compose.yml`](../docker-compose.yml) + [`scripts/docker-run.sh`](../scripts/docker-run.sh) (`CPN_ALMA_VERSION=8|9|10`). There is no Hub `almalinux8` tag.

### Manual `docker run` example

```bash
DATA_ROOT=/var/lib/cpn-docker-host
mkdir -p "$DATA_ROOT"/{cpn-data,cpn-home,cpn-mysql,cpn-letsencrypt,cpn-webmail}
chmod 0755 "$DATA_ROOT"/cpn-{data,home,mysql,letsencrypt,webmail}

# Swap tag for almalinux10 or ubuntu26.04; security flags stay the same.
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

### What remains (intentional trade-offs)

These are still required for this systemd-in-container design and are **not** equivalent to a locked-down app container:

- `SYS_ADMIN` (and related) capabilities
- AppArmor and seccomp set to `unconfined`
- Host cgroup namespace + writable `/sys/fs/cgroup`

Treat Hub guests as **lab/test only**. Prefer native packages for production.

### Runtime flags

- Keep `--cgroupns=host` and a writable `/sys/fs/cgroup` mount (required for systemd in this design).
- Keep `/run` and `/run/lock` tmpfs and `STOPSIGNAL SIGRTMIN+3`.
- **Privileged vs reduced capabilities:** default in [`scripts/docker-run.sh`](../scripts/docker-run.sh), [`docker-compose.yml`](../docker-compose.yml), and [`docker-compose.hub.yml`](../docker-compose.hub.yml) is a **reduced `--cap-add` set** (verified on AlmaLinux 10 with `master3395/cpn-installer:almalinux10`: systemd `degraded`, `cpn-installer` active). Full `--privileged` grants substantially broader host access (effectively root-equivalent on the Docker/Podman host).
- **No automatic privileged retry.** Set `CPN_DOCKER_PRIVILEGED=1` (or `privileged: true` in Compose) only if the reduced set fails and you accept that risk on a dedicated lab host.
- Host data directories: mode **0755** only (never `777`).
- Do not expose port 2087 on untrusted networks.
- Production panels: install native RPM/DEB (no privileged container required for the panel itself).

## In-panel Docker

- **Compose Stacks** (`/docker/stacks`): always bind under `/var/lib/cpn/docker-data/<stack>/data`. Pull & Recreate preserves data.
- **Create Container** (`/docker/create`): volumes optional; suggested bind `/var/lib/cpn/docker-data/<name>/data:/data`. Without a bind, recreate loses data.

## Related

- [INSTALL.md](INSTALL.md) (Hub pull/run, native install, `--bypass`)
- [docker/hub-overview.md](../docker/hub-overview.md) (Docker Hub description source)
- `scripts/docker-run.sh` (local build and run helper)
- `docker-compose.hub.yml` (pre-built Hub image, including Ubuntu tags)
