# CPN Control Panel Network installer

AlmaLinux and Ubuntu runtime images with **systemd** and the **CPN** (`cpn-installer`) package preinstalled. Built for maintainer smoke tests and lab installs of [CPN Control Panel Network](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network).

## Status: alpha / prerelease

CPN is **alpha-only** (0.2.x-alpha line). These images are **not** a production-stable 1.x release. Use for evaluation and lab testing.

## Pull

```bash
docker pull master3395/cpn-installer:almalinux9
docker pull master3395/cpn-installer:almalinux10
docker pull master3395/cpn-installer:ubuntu22.04
docker pull master3395/cpn-installer:ubuntu24.04
docker pull master3395/cpn-installer:ubuntu26.04
docker pull master3395/cpn-installer:latest   # same baseline as almalinux9
```

| Tag | Base |
|-----|------|
| `almalinux9`, `latest` | AlmaLinux 9 |
| `almalinux10` | AlmaLinux 10 |
| `ubuntu22.04` | Ubuntu 22.04 LTS |
| `ubuntu24.04` | Ubuntu 24.04 LTS |
| `ubuntu26.04` | Ubuntu 26.04 |

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
