# CPN - Control Panel Network

> [!WARNING]
> **Work in progress (not finished).** This version is experimental and is not ready for production servers.

[![Status: in development](https://img.shields.io/badge/status-in%20development-f59e0b)](#project-status)
[![CI](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml/badge.svg)](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)

CPN is a web installer for preparing server panel components. The primary path is Linux guests (AlmaLinux, Rocky Linux, RHEL, Ubuntu, Debian, and related targets listed below). Windows Server 2016+ has a Phase A path (installer UI + account bootstrap) without Linux package parity. A single Rust process serves the HTTP interface, streams real progress over WebSockets, and embeds the React app in the final binary.

Install on a supported Linux guest (RPM on RHEL-family, experimental `.deb` on Ubuntu), run an AlmaLinux-based installer inside Docker/Podman (privileged + systemd), or use the Windows zip + PowerShell installer on Server 2016+. See [to-do/OS-SUPPORT-MATRIX.md](to-do/OS-SUPPORT-MATRIX.md) (authoritative matrix), [to-do/WINDOWS-SERVER-INSTALL.md](to-do/WINDOWS-SERVER-INSTALL.md), [to-do/DOCKER-INSTALL.md](to-do/DOCKER-INSTALL.md), and [to-do/INSTALL-METHODS.md](to-do/INSTALL-METHODS.md) (GUI + CLI + SSH).

## Supported operating systems

This section mirrors [`to-do/OS-SUPPORT-MATRIX.md`](to-do/OS-SUPPORT-MATRIX.md) (authoritative guest OS matrix) and `src/os_support.rs`.

| Status | Meaning |
|---|---|
| **Supported** | Detection + install recipes implemented **and** smoke evidence (lab VM and/or `tests/docker-matrix.sh`) |
| **Partial** | Allowlisted; Linux family recipes or Windows Phase A; less smoke evidence, or an external blocker remains |
| **Not yet** | Known or planned target outside the installable allowlist; installer refuses with a helpful message |
| **Host only** | Hypervisor for Linux guests; not an install target by itself |

### Guest OS (where `cpn-installer` runs)

| Guest OS | Status | Package path | Evidence / notes |
|---|---|---|---|
| AlmaLinux 10 | Supported | dnf | Lab VM verified (SSH 2223 / UI 2088) |
| AlmaLinux 9 | Supported | dnf | Lab VM verified (SSH 2222 / UI 2087); default Docker matrix image |
| AlmaLinux 8 | Partial | dnf | Recipes + Remi PHP 8.2; promote after nginx matrix smoke |
| Rocky Linux 9 | Supported | dnf | Same EL9 recipe family; CI/os-matrix nginx smoke |
| Rocky Linux 8 | Partial | dnf | Same EL8 path; promote after matrix smoke |
| RHEL 9 | Partial | dnf | Allowlisted; subscription/repos are operator responsibility |
| RHEL 8 | Partial | dnf | Same subscription blocker as RHEL 9 |
| CloudLinux 8 | Partial | dnf | Detected when `ID=cloudlinux`; no public ISO/lab image here |
| CentOS Stream 9 | Partial | dnf | Detected when `ID=centos` major 9; promote after matrix smoke |
| Ubuntu 24.04 | Supported | apt | Apt recipes + OLS apt keyring bootstrap; lab/matrix verification in progress |
| Ubuntu 22.04 | Supported | apt | Apt recipes + OLS apt keyring bootstrap; lab/matrix verification in progress |
| Ubuntu 20.04 | Partial | apt | Allowlisted (focal OLS suite exists); older PHP/repos; thinner evidence |
| Debian 11/12/13 | Partial | apt | Detection + Ubuntu-like apt path (nginx/Caddy/OLS/PHP); not full matrix yet |
| openEuler 20-24 | Partial | dnf | Detection + dnf family path; package names may diverge; no lab ISO here |
| Windows Server 2016+ | Partial | Windows Phase A | Native installer UI + service + `C:\ProgramData\CPN`; no dnf/apt. See [WINDOWS-SERVER-INSTALL.md](to-do/WINDOWS-SERVER-INSTALL.md) |
| Windows Server 2012 / 2012 R2 | Not yet | n/a | Modern Rust/MSVC does not support these hosts |
| Other RHEL derivatives | Not yet | dnf (planned) | Clear error when not in allowlist |

Quick scan by status:

- **Supported:** AlmaLinux 10, AlmaLinux 9, Rocky Linux 9, Ubuntu 24.04, Ubuntu 22.04
- **Partial:** AlmaLinux 8, Rocky Linux 8, RHEL 9, RHEL 8, CloudLinux 8, CentOS Stream 9, Ubuntu 20.04, Debian 11/12/13, openEuler 20-24, Windows Server 2016+
- **Not yet:** Windows Server 2012 / 2012 R2; other RHEL derivatives outside the allowlist

Do not treat "Supported" as a full mail/server matrix for every row. Windows Partial is Phase A only (no Linux recipe parity). See the matrix for lab notes and blockers.

### Host / hypervisor

| Platform | Status | Role |
|---|---|---|
| VirtualBox | Host only | Lab VMs for Linux guests |
| Hyper-V | Host only | Lab VMs for Linux guests (Windows Server can also run Phase A natively) |

WSL2 is not a supported guest target for systemd and firewall recipes.

## Project status

The first phase implements the installer flow and is still in development. It currently includes:

- Selection and installation of OpenLiteSpeed, Caddy, or Nginx.
- Selection and installation of SnappyMail, Roundcube, or Thunderbird.
- Real download, install, and verification progress sent over WebSocket.
- VPS and container detection; opening port `2087` in `firewalld` or `ufw` when those are active.
- RPM packaging for RHEL-family guests (AlmaLinux/Rocky/RHEL 8-10); experimental Ubuntu `.deb` via `scripts/build-deb.sh`.
- Windows Server 2016+ Phase A: `packaging/windows/` zip + service install (`C:\ProgramData\CPN`, port `2087`).
- Docker/Podman runtime images for AlmaLinux 9 and 10 (`Dockerfile`, `docker-compose.yml`, `scripts/docker-run.sh`).
- Service tests in clean containers (`almalinux:9.8` by default; override with `CPN_TEST_IMAGE` or `CPN_TEST_IMAGES`).
- Privileged multi-OS nginx smoke: `.github/workflows/os-matrix.yml` (not on untrusted PRs; see issue #7).

Recipes, security, and compatibility still need review before CPN can be considered production-ready.

## Structure

- `installer-ui/`: React and Vite interface.
- `Panel/`: React and Next.js control panel based on Stitch screens.
- `src/`: Actix Web server, WebSocket, environment detection, and install recipes.
- `packaging/`: RPM specification, systemd unit, and `packaging/windows/` PowerShell installers.
- `scripts/build-rpm.sh`: builds the binary and RPM on AlmaLinux/Rocky/RHEL/CentOS/CloudLinux 8-10.
- `scripts/build-deb.sh`: experimental `.deb` on Ubuntu 20.04/22.04/24.04.
- `to-do/OS-SUPPORT-MATRIX.md`: guest OS matrix and hypervisor host notes.
- `to-do/WINDOWS-SERVER-INSTALL.md`: Windows Server Phase A guide.
- `scripts/docker-build-rpm.sh`: builds the RPM inside an AlmaLinux container (any host with Docker/Podman).
- `scripts/docker-run.sh`: builds the runtime image and starts a privileged systemd container on port `2087`.
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
# Optional: choose listen port (default 2087)
sudo cpn-installer --port 2087
# Lab example on a different port
sudo cpn-installer --port 8787
```

The RPM release suffix follows the host distro (`.el9` or `.el10`).

When you run `cpn-installer`, the console immediately prints the full installer web URL, including the reachable IP and a temporary token. By default the service listens on `127.0.0.1:2087` (override with `--port`, `CPN_LISTEN_PORT`, or the installer UI). Use `--allow-remote` to bind `0.0.0.0`.

## Docker / Podman install

On any host with Docker or Podman (Linux preferred for systemd/cgroup support):

```bash
# AlmaLinux 9 (default): build RPM if needed, start privileged installer container
./scripts/docker-run.sh

# AlmaLinux 10
CPN_ALMA_VERSION=10 ./scripts/docker-run.sh
```

Open the printed `http://127.0.0.1:2087/?token=...` URL.

**Security:** the container runs privileged (systemd + `dnf` + `systemctl`). Use only on dedicated lab/test hosts. Do not expose port `2087` on untrusted networks.

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

Official release packages are GPG-signed. Before installing a downloaded RPM or binary as root, verify checksums and signatures (`SHA256SUMS`, `SHA256SUMS.asc`, `rpm --checksig`). Fingerprint: `EA5B57B1230FFA37F4426A873EC7121204EB5515`. Public key: [`packaging/RPM-GPG-KEY-CPN`](packaging/RPM-GPG-KEY-CPN). Full steps: [`to-do/RELEASE-SIGNING.md`](to-do/RELEASE-SIGNING.md).

See [SECURITY.md](SECURITY.md) for how to report vulnerabilities.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Copyright (C) 2026 CPN contributors.

This project is distributed under the [GNU General Public License version 3](LICENSE) (`GPL-3.0-only`).
