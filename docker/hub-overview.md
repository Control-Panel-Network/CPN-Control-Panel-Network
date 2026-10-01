# CPN Control Panel Network installer

AlmaLinux (9, 10) and Ubuntu 26.04 runtime images with **systemd** and the **CPN** (`cpn-installer`) package preinstalled. Built for maintainer smoke tests and lab installs of [CPN Control Panel Network](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network).

## Status: stable (v1.0.0)

CPN **v1.0.0** is the first stable release, following the 0.2.x alpha line. Keep backups and test upgrades on a staging host before production use.

## Pull

```bash
docker pull master3395/cpn-installer:almalinux9
docker pull master3395/cpn-installer:almalinux10
docker pull master3395/cpn-installer:ubuntu26.04
docker pull master3395/cpn-installer:latest   # same baseline as almalinux9
```

| Tag | Base |
|-----|------|
| `almalinux9`, `latest` | AlmaLinux 9 |
| `almalinux10` | AlmaLinux 10 |
| `ubuntu26.04` | Ubuntu 26.04 |

**Ubuntu container tag:** the Docker Ubuntu image is **26.04 only**. The `ubuntu22.04` and `ubuntu24.04` tags were retired from Docker Hub because Docker Scout reports Medium/Low findings in base-distro packages that Canonical has not fixed yet. On Ubuntu 22.04 or 24.04 hosts, use the native **DEB/apt** install from GitHub Releases (`docs/INSTALL.md`), not these container tags. Tracking: issue #353.

On each `v*` Git release tag, the semver (without leading `v`) is also pushed on the `almalinux9` image.

**Rocky Linux** and other EL guests: use native **RPM** installs from GitHub Releases (`docs/INSTALL.md`). There is no separate `rocky9` Hub tag; EL smoke may use the AlmaLinux runtime images.

## Run (privileged)

See `scripts/docker-run.sh` in the repository. Typical pattern:

```bash
docker run -d --privileged \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  --cgroupns=host \
  -p 2087:2087 \
  master3395/cpn-installer:almalinux9
```

Prefer native RPM/DEB installs for production hosts (`docs/INSTALL.md`).
