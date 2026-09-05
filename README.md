# CPN — Control Panel Network

> [!WARNING]
> **CPN is under active development and is not ready for production servers.** Use a test VPS or VM and keep backups of any machine you modify.

[![CI](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml/badge.svg)](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)

CPN is a Rust-based web installer and server-control project. The installer embeds its React UI, reports real progress over WebSockets, and prepares web, database, mail, and panel components on supported Linux guests. Windows Server currently has a limited Phase A path.

## Install

**End users should download the package for their OS from [GitHub Releases](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases). You do not need Rust, Node.js, Docker, `rpmbuild`, or the source repository to install a release.**

Release assets are produced for these distribution paths:

- Enterprise Linux family: `cpn-installer-...el8...rpm`, `...el9...rpm`, or `...el10...rpm`.
- Ubuntu/Debian: `cpn-installer_...amd64.deb`.
- Windows Server 2016+ Phase A: `cpn-windows-x86_64.zip`.

Linux installation:

```bash
# AlmaLinux / Rocky / RHEL-family
sudo dnf install ./cpn-installer-*.rpm

# Ubuntu / Debian
sudo apt install ./cpn-installer_*.deb

# Start an interactive installer session
sudo cpn-installer
```

The Linux installer listens on `127.0.0.1:2087` by default and prints a temporary URL containing its access token. For a remote server, prefer SSH port forwarding rather than exposing the installer directly:

```bash
ssh -L 2087:127.0.0.1:2087 root@your-server
```

Then open the URL printed by `cpn-installer` locally. `--allow-remote` binds to all interfaces and should only be used on a trusted network because the installer UI currently uses HTTP rather than TLS.

Windows Phase A installation: extract `cpn-windows-x86_64.zip`, open an elevated PowerShell prompt, and run `Install-Cpn.ps1`. The default Windows mode also binds only to loopback and does not create an inbound firewall rule. `-AllowRemote` is an explicit HTTP/network exposure opt-in.

> [!NOTE]
> If the newest release does not contain an asset for your target OS, treat that target as unavailable for that release. The build scripts in `scripts/` are maintainer/development tools, not an installation requirement.

## Current guest support

CPN distinguishes between targets with recurring smoke evidence and targets that share an implemented package-family path but still need broader validation.

| Guest | Status | Package path | Notes |
|---|---|---|---|
| AlmaLinux 9 / 10 | Supported | RPM / dnf | Primary EL targets |
| Rocky Linux 9 | Supported | RPM / dnf | Automated Rocky smoke path |
| Ubuntu 22.04 / 24.04 | Supported | DEB / apt | Primary apt targets |
| AlmaLinux 8 | Partial | RPM / dnf | Maintenance-era EL8; PHP uses Remi 8.2 path |
| Rocky Linux 8 / 10 | Partial | RPM / dnf | Shared EL recipes; less CPN smoke evidence |
| RHEL 8 / 9 / 10 | Partial | RPM / dnf | Requires working RHEL subscriptions/repos |
| CloudLinux 8 / 9 / 10 | Partial | RPM / dnf | Shared EL recipes; no public CPN lab matrix |
| CentOS Stream 9 / 10 | Partial | RPM / dnf | Shared EL recipes |
| Debian 12 / 13 | Partial | DEB / apt | Implemented apt path; matrix coverage is being expanded |
| Windows Server 2016+ | Partial | Windows ZIP | Phase A: installer UI/account bootstrap; no Linux web/mail package parity |

CPN recognizes but refuses new installs on Ubuntu 20.04 and Debian 11 because their normal security-support windows have ended, and on openEuler because CPN's third-party web/mail repository stack has not been validated there. Windows Server 2012/2012 R2 and unknown distributions are also refused.

“Supported” does **not** mean every possible web-server/mail-client combination has production-grade coverage. CPN is still alpha software.

## Existing software and repeat installs

CPN is designed to be increasingly idempotent instead of assuming every machine is empty:

- Nginx, Caddy, and OpenLiteSpeed recipes detect an existing selected server and reuse it instead of deliberately installing a second copy, then continue with activation/configuration.
- Existing Caddy/LiteSpeed repository files that CPN must change are backed up through the install journal; rollback restores operator-owned content instead of deleting it.
- OpenLiteSpeed configuration changes are journaled, and CPN no longer deletes administrator-owned systemd units while adopting an existing installation.
- MariaDB/MySQL defaults detect an existing database service and avoid replacing it with the conflicting engine.
- PHP setup keeps a sufficiently new existing PHP installation rather than blindly switching module streams.
- Firewall cleanup removes only rules CPN recorded as its own; pre-existing firewalld/UFW rules remain operator-owned.

Do not use CPN as an automatic migration tool for a complex production host. Existing custom virtual hosts, nonstandard package layouts, or another web server already bound to the same ports can still require manual review.

## Release verification

Official tagged releases publish `SHA256SUMS`, a signed checksum manifest, the public release key, provenance information, and signatures for signed artifacts.

Release signing fingerprint:

```text
FE70B9718F63B10BB70A6F70BECBB7488AE5C3E5
```

Basic verification after downloading the release files:

```bash
sha256sum -c SHA256SUMS
gpg --import RPM-GPG-KEY-CPN
gpg --verify SHA256SUMS.asc SHA256SUMS

# RPM only
sudo rpm --import RPM-GPG-KEY-CPN
rpm --checksig ./cpn-installer-*.rpm
```

Never publish the temporary installer token printed in the console URL. See [SECURITY.md](SECURITY.md) for vulnerability reporting and operator guidance.

## Docker / Podman

The privileged systemd containers in this repository are **development and smoke-test environments**, not the recommended end-user installation method. They intentionally require elevated container privileges so package managers, systemd, and service tests behave like a guest VM.

Maintainers can use:

```bash
./scripts/docker-run.sh
./tests/docker-matrix.sh
```

## Development

Source builds are documented for contributors, not end users. See [CONTRIBUTING.md](CONTRIBUTING.md) for Rust/Node prerequisites, validation commands, package build helpers, and the pull-request workflow.

Main areas:

- `src/` — Rust installer, detection, install/configuration logic, CLI and panel backend.
- `installer-ui/` — React/Vite installer UI embedded into the Rust binary.
- `Panel/` — Next.js panel frontend.
- `packaging/` — native package/service definitions.
- `scripts/` — maintainer build, signing and container helpers.
- `tests/` — functional installation smoke tests.
- `.github/workflows/` — CI, OS matrix, CodeQL and release automation.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Copyright (C) 2026 CPN contributors.

CPN is distributed under the [GNU General Public License version 3](LICENSE) (`GPL-3.0-only`).
