# OS support matrix (maintainer)

Keep this table aligned with [SUPPORT.md](SUPPORT.md) and the root [README](../README.md). The current stable line is **1.1.x**.

| Guest | Tier | Native package | Docker Hub `master3395/cpn-installer` tag | Smoke notes |
|---|---|---|---|---|
| AlmaLinux 9 | Supported | RPM el9 | `almalinux9`, `latest` | Primary EL; OS matrix + Hub publish |
| AlmaLinux 10 | Supported | RPM el10 | `almalinux10` | OS matrix + Hub publish |
| Rocky Linux 9 | Supported | RPM el9 | (use `almalinux9` runtime for smoke only) | OS matrix RPM path |
| Ubuntu 22.04 | Supported | DEB | n/a (retired, use native DEB) | OS matrix apt; Docker tag retired, issue #353 |
| Ubuntu 24.04 | Supported | DEB | n/a (retired, use native DEB) | OS matrix apt; Docker tag retired, issue #353 |
| Ubuntu 26.04 / 26.04.1 | Supported | DEB | `ubuntu26.04` | Native `resolute` packages, verified scoped LiteSpeed keyring, OLS 1.9.3+, Hub publish |
| AlmaLinux 8 | Partial | dnf recipes | n/a | No release RPM (WebAuthn toolchain) |
| Rocky 8 / 10 | Partial | RPM/recipes | n/a | Less smoke evidence |
| RHEL / CloudLinux / CentOS Stream | Partial | RPM/recipes | n/a | Subscription/repos required |
| Debian 12 / 13 | Partial | DEB | n/a | apt path; expanding matrix |
| Windows Server 2016+ | Partial | ZIP | n/a | Phase A only |

Hub images are optional installer runtimes (systemd, privileged). End users should still prefer [INSTALL.md](INSTALL.md) native packages from GitHub Releases.

## Ubuntu 26.04 OpenLiteSpeed

LiteSpeed publishes a native `resolute` suite for amd64 and arm64. CPN uses the official repository directly and scopes its verified keys to `/usr/share/keyrings/litespeed-archive-keyring.gpg` with apt `signed-by`. CPN temporarily disables an existing LiteSpeed source while bootstrapping or repairing the keyring, so the first `apt-get update` cannot fail on a not-yet-installed key. The installer then enables and starts the vendor `lsws` or `lshttpd` service and applies the normal CPN vhost/listener configuration.

System Repair offers the `openlitespeed` heal when an installed OLS service is inactive or its apt source/keyring is outdated. It does not silently install or switch to OLS on a host where OLS is absent. Use the installer web-server selection for a stack switch so port conflicts are handled safely.
