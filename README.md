# CPN — Control Panel Network

> [!WARNING]
> **CPN is under active development and is not ready for production servers.** Use a test VPS or VM and keep backups of any machine you modify.

[![CI](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml/badge.svg)](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)

CPN is a Rust-based web installer and server-control project. The installer embeds its React UI, reports real progress over WebSockets, and prepares web, database, mail, and panel components on supported Linux guests. Windows Server currently has a limited Phase A path.

## Installation

Install CPN from packages published in [GitHub Releases](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases). End users do **not** need Rust, Node.js, Docker, `rpmbuild`, or a clone of this repository.

Current release package paths are:

- Enterprise Linux family: `cpn-installer-...el8...rpm`, `...el9...rpm`, or `...el10...rpm`.
- Ubuntu/Debian: `cpn-installer_...amd64.deb`.
- Windows Server 2016+ Phase A: `cpn-windows-x86_64.zip`.

See [Platform Support](docs/SUPPORT.md) before installing on a new operating system.

### AlmaLinux, Rocky Linux, RHEL and compatible EL systems

Download the RPM matching the server's Enterprise Linux major version and install it with `dnf`:

```bash
sudo dnf install ./cpn-installer-*.rpm
```

Do not mix EL package generations; for example, do not install an EL9 RPM on EL10.

### Ubuntu and Debian

Download the `.deb` release package and install it with `apt`:

```bash
sudo apt install ./cpn-installer_*.deb
```

### Start CPN

After installing the native package:

```bash
sudo cpn-installer
```

The installer listens on `127.0.0.1:2087` by default and prints a temporary URL containing its access token.

For a remote server, use SSH port forwarding:

```bash
ssh -L 2087:127.0.0.1:2087 root@your-server
```

Then open the URL printed by `cpn-installer` in a browser on your local machine.

> [!IMPORTANT]
> Do not publish or share the temporary installer token printed in the console URL.

Direct remote HTTP exposure is available but is not the recommended default:

```bash
sudo cpn-installer --allow-remote
```

`--allow-remote` binds to `0.0.0.0`. Use it only on a trusted network because the temporary installer UI currently uses HTTP rather than TLS.

### Windows Server Phase A

1. Download `cpn-windows-x86_64.zip` from GitHub Releases.
2. Extract it.
3. Open PowerShell as Administrator.
4. Run:

```powershell
.\Install-Cpn.ps1
```

The default Windows mode binds only to loopback and does not create an inbound firewall rule. Remote HTTP exposure is explicit:

```powershell
.\Install-Cpn.ps1 -AllowRemote
```

Windows support is currently Phase A and does not yet provide feature parity with the Linux web/mail installation path.

> [!NOTE]
> If the newest release does not include an artifact for your OS and architecture, treat that target as unavailable for that release. The files under `scripts/` are maintainer/development tools, not an installation requirement.

## After installation

The `cpn` command is the operator CLI for accounts, websites, network settings, plugins, host apps, and hosting packages.

```bash
cpn --help
cpn list
```

See the full **[CPN CLI and installer argument reference](docs/CLI.md)** for commands and flags such as `cpn site`, `cpn app`, `cpn network`, `cpn package`, `cpn-installer --port`, and `--panel-hostname`.

## Documentation

- **[CLI Reference](docs/CLI.md)** — `cpn` commands, subcommands, arguments, and `cpn-installer` runtime flags.
- **[Platform Support](docs/SUPPORT.md)** — supported/partial/refused systems, repeat installs, and behavior on hosts with existing software.
- **[Releases and Verification](docs/RELEASES.md)** — release assets, package selection, checksums, GPG signatures, and provenance.
- **[Contributing Guide](CONTRIBUTING.md)** — source builds, development prerequisites, tests, package build helpers, and pull-request workflow.
- **[Security Policy](SECURITY.md)** — vulnerability reporting and operator security guidance.
- **[Code of Conduct](CODE_OF_CONDUCT.md)** — community participation guidelines.

## License

Copyright (C) 2026 CPN contributors.

CPN is distributed under the [GNU General Public License version 3](LICENSE) (`GPL-3.0-only`).
