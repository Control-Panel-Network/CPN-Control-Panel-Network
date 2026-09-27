# OS support matrix (maintainer)

Keep this table aligned with [SUPPORT.md](SUPPORT.md) and the root [README](../README.md). CPN product line: **0.2.x-alpha** (not stable 1.x).

| Guest | Tier | Native package | Docker Hub `master3395/cpn-installer` tag | Smoke notes |
|---|---|---|---|---|
| AlmaLinux 9 | Supported | RPM el9 | `almalinux9`, `latest` | Primary EL; OS matrix + Hub publish |
| AlmaLinux 10 | Supported | RPM el10 | `almalinux10` | OS matrix + Hub publish |
| Rocky Linux 9 | Supported | RPM el9 | (use `almalinux9` runtime for smoke only) | OS matrix RPM path |
| Ubuntu 22.04 | Supported | DEB | `ubuntu22.04` | OS matrix apt + Hub publish |
| Ubuntu 24.04 | Supported | DEB | `ubuntu24.04` | OS matrix apt + Hub publish |
| Ubuntu 26.04 / 26.04.1 | Supported | DEB | `ubuntu26.04` | apt codename `resolute`; Hub publish |
| AlmaLinux 8 | Partial | dnf recipes | n/a | No release RPM (WebAuthn toolchain) |
| Rocky 8 / 10 | Partial | RPM/recipes | n/a | Less smoke evidence |
| RHEL / CloudLinux / CentOS Stream | Partial | RPM/recipes | n/a | Subscription/repos required |
| Debian 12 / 13 | Partial | DEB | n/a | apt path; expanding matrix |
| Windows Server 2016+ | Partial | ZIP | n/a | Phase A only |

Hub images are optional **alpha/prerelease** installer runtimes (systemd, privileged). End users should still prefer [INSTALL.md](INSTALL.md) native packages from GitHub Releases.
