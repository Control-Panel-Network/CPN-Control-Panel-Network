# CPN - Control Panel Network

> [!WARNING]
> **Work in progress (not finished).** This version is experimental and is not ready for production servers.

[![Status: in development](https://img.shields.io/badge/status-in%20development-f59e0b)](#project-status)
[![CI](https://github.com/KraoESPfan1n/CPN-Control-Panel-Network/actions/workflows/ci.yml/badge.svg)](https://github.com/KraoESPfan1n/CPN-Control-Panel-Network/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)

CPN is a Linux web installer for preparing server panel components on CyberPanel-aligned guest operating systems. A single Rust process serves the HTTP interface, streams real progress over WebSockets, and embeds the React app in the final binary.

Install on a supported Linux guest (RPM on RHEL-family, experimental `.deb` on Ubuntu), or run an AlmaLinux-based installer inside Docker/Podman (privileged + systemd). See [to-do/OS-SUPPORT-MATRIX.md](to-do/OS-SUPPORT-MATRIX.md) (authoritative matrix) and [to-do/DOCKER-INSTALL.md](to-do/DOCKER-INSTALL.md).

## Supported operating systems

CPN installs on **Linux guests** only (CyberPanel-aligned targets). Status matches [`to-do/OS-SUPPORT-MATRIX.md`](to-do/OS-SUPPORT-MATRIX.md) and `src/os_support.rs`:

| Status | Meaning |
|---|---|
| **Supported** | Detection and install recipes implemented for that family |
| **Partial** | Allowlisted; recipes run via the family path; less lab evidence |
| **Not yet** | Known CyberPanel/community target; installer refuses with a helpful message |
| **Host only** | Hypervisor or Windows host for Linux guests; not a CPN install target |

### Guest OS (where `cpn-installer` runs)

| Guest OS | Status | Package path | Notes |
|---|---|---|---|
| AlmaLinux 10 | Supported | dnf | Lab-verified earlier |
| AlmaLinux 9 | Supported | dnf | Lab-verified; default Docker matrix image |
| AlmaLinux 8 | Partial | dnf | Detected; PHP module `php:8.0`; needs more lab proof |
| Rocky Linux 9 | Supported | dnf | Same EL9 recipe family as Alma 9 |
| Rocky Linux 8 | Partial | dnf | Same EL8 path; needs lab proof |
| RHEL 9 | Partial | dnf | Allowlisted; subscription/repos are operator responsibility |
| RHEL 8 | Partial | dnf | Allowlisted; needs lab proof |
| CloudLinux 8 | Partial | dnf | Detected when `ID=cloudlinux` |
| CentOS Stream 9 | Partial | dnf | Detected when `ID=centos` major 9 |
| Ubuntu 24.04 | Supported | apt | Code path present; lab verification still needed |
| Ubuntu 22.04 | Supported | apt | Code path present; lab verification still needed |
| Ubuntu 20.04 | Partial | apt | Allowlisted; older PHP/repos; verify before production |
| Debian | Not yet | apt (planned) | Clear error; best-effort community |
| openEuler | Not yet | (planned) | Clear error; best-effort community |
| Other RHEL derivatives | Not yet | dnf (planned) | Clear error when not in allowlist |

Quick scan by status:

- **Supported:** AlmaLinux 10, AlmaLinux 9, Rocky Linux 9, Ubuntu 24.04, Ubuntu 22.04
- **Partial:** AlmaLinux 8, Rocky Linux 8, RHEL 9, RHEL 8, CloudLinux 8, CentOS Stream 9, Ubuntu 20.04
- **Not yet:** Debian, openEuler, other RHEL derivatives outside the allowlist

Do not treat "Supported" as lab-verified for every row. AlmaLinux 9/10 have lab verification; Ubuntu 22.04/24.04 and Rocky 9 are supported in code and still need full lab proof (see the matrix).

### Host / hypervisor (not install targets)

| Platform | Role |
|---|---|
| Windows Server | Host only (Hyper-V role for Linux guests); not a native panel install |
| VirtualBox | Host only (lab VMs for Linux guests) |
| Hyper-V | Host only (lab VMs for Linux guests) |

WSL2 is not a supported guest target for systemd and firewall recipes.

## Project status

The first phase implements the installer flow and is still in development. It currently includes:

- Selection and installation of OpenLiteSpeed, Caddy, or Nginx.
- Selection and installation of SnappyMail, Roundcube, or Thunderbird.
- Real download, install, and verification progress sent over WebSocket.
- VPS and container detection; opening port `8787` in `firewalld` or `ufw` when those are active.
- RPM packaging for RHEL-family guests (AlmaLinux/Rocky/RHEL 8-10); experimental Ubuntu `.deb` via `scripts/build-deb.sh`.
- Docker/Podman runtime images for AlmaLinux 9 and 10 (`Dockerfile`, `docker-compose.yml`, `scripts/docker-run.sh`).
- Service tests in clean AlmaLinux containers (`almalinux:9.8` by default; override with `CPN_TEST_IMAGE`).

Recipes, security, and compatibility still need review before CPN can be considered production-ready.

## Structure

- `installer-ui/`: React and Vite interface.
- `Panel/`: React and Next.js control panel based on Stitch screens.
- `src/`: Actix Web server, WebSocket, environment detection, and install recipes.
- `packaging/`: RPM specification and systemd unit used by the Docker image.
- `scripts/build-rpm.sh`: builds the binary and RPM on AlmaLinux/Rocky/RHEL/CentOS/CloudLinux 8-10.
- `scripts/build-deb.sh`: experimental `.deb` on Ubuntu 20.04/22.04/24.04.
- `to-do/OS-SUPPORT-MATRIX.md`: guest OS matrix and hypervisor host notes.
- `scripts/docker-build-rpm.sh`: builds the RPM inside an AlmaLinux container (any host with Docker/Podman).
- `scripts/docker-run.sh`: builds the runtime image and starts a privileged systemd container on port `8787`.
- `Dockerfile` / `docker-compose.yml`: AlmaLinux 9/10 installer container (parameterized with `CPN_ALMA_VERSION`).
- `tests/docker-matrix.sh`: functional matrix for web servers and mail clients.
- `to-do/DOCKER-INSTALL.md`: Docker install guide, ports, volumes, and security notes.

## Development and validation

```bash
# Rust
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings

# React, without a production build
cd installer-ui
npm ci
npm run lint

# Panel Next.js
cd ../Panel
npm ci
npm run lint
npm run typecheck
```

The continuous integration workflow runs these checks on every push and pull request.

## RPM packaging (RHEL-family)

On AlmaLinux, Rocky Linux, RHEL, CentOS Stream, or CloudLinux (majors 8-10):

```bash
./scripts/build-rpm.sh
sudo dnf install ./target/rpmbuild/RPMS/x86_64/cpn-installer-*.rpm
sudo cpn-installer
```

The RPM release suffix follows the host distro (`.el9` or `.el10`).

When you run `cpn-installer`, the console immediately prints the full installer web URL, including the reachable IP and a temporary token. The service listens on `0.0.0.0:8787`.

## Docker / Podman install

On any host with Docker or Podman (Linux preferred for systemd/cgroup support):

```bash
# AlmaLinux 9 (default): build RPM if needed, start privileged installer container
./scripts/docker-run.sh

# AlmaLinux 10
CPN_ALMA_VERSION=10 ./scripts/docker-run.sh
```

Open the printed `http://127.0.0.1:8787/?token=...` URL.

**Security:** the container runs privileged (systemd + `dnf` + `systemctl`). Use only on dedicated lab/test hosts. Do not expose port `8787` on untrusted networks.

Build the RPM alone from a non-AlmaLinux host:

```bash
./scripts/docker-build-rpm.sh
```

Compose (after `cpn-installer.rpm` exists in the repo root):

```bash
export CPN_ALMA_VERSION=9
docker compose build && docker compose up -d
```

Full details: [to-do/DOCKER-INSTALL.md](to-do/DOCKER-INSTALL.md).

## Functional tests in Docker

The matrix requires Docker with privileged containers and systemd support:

```bash
./tests/docker-matrix.sh
```

It checks that Nginx and Caddy respond over HTTP, and that SnappyMail, Roundcube, and Thunderbird are installed and pass their specific checks.

## Security

Do not publish the temporary token that appears in the installer URL. CPN makes system changes and should only be run on a dedicated test machine during this stage.

See [SECURITY.md](SECURITY.md) for how to report vulnerabilities.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Copyright (C) 2026 CPN contributors.

This project is distributed under the [GNU General Public License version 3](LICENSE) (`GPL-3.0-only`).
