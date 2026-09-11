# CPN - Control Panel Network

> [!WARNING]
> **CPN is alpha-only for now** (newest published cut: **v0.2.4-alpha.19**). There is no stable 1.x line yet. Prefer a disposable test VPS or VM, keep backups, and review [Platform Support](docs/SUPPORT.md) before touching important hosts.

[![CI](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml/badge.svg?branch=stable)](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)

CPN is a Rust-based web installer and server-control project. The installer embeds its React UI, reports real progress over WebSockets, and prepares web, database, mail, and panel components on supported Linux guests. Windows Server currently has a limited Phase A path.

## Installation Instructions

Primary path for Linux guests: run the official CPN bootstrap script as **root**. It detects AlmaLinux / Rocky / RHEL (EL9/EL10) or Ubuntu / Debian, downloads the matching package from [GitHub Releases](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases), verifies `SHA256SUMS` and GPG when those assets exist, installs the package, then prints how to start `cpn-installer`.

```bash
sh <(curl https://cpn.newstargeted.com/install.sh || wget -O - https://cpn.newstargeted.com/install.sh)
```

Notes:

- Root (or `sudo`) is required.
- Supported package targets today: EL9/EL10 RPM and Ubuntu/Debian `.deb` (see [Platform Support](docs/SUPPORT.md)).
- Unknown or refused OS versions fail closed (no install).
- Prefer a disposable test machine for first installs.
- GitHub raw fallback (same script): `https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh`.
- `cpn.newstargeted.com` serves or mirrors that install script at `/install.sh` (HTTPS).
- Current published alpha: **v0.2.4-alpha.19** (see [Changelog](docs/CHANGELOG.md)). Former `v1.0.0` / `v1.0.1` tags were renamed to `v0.2.3-alpha.19` / `v0.2.4-alpha.19` and removed. Bootstrap one-liners pick the newest non-draft release (**including prereleases**) unless you pin `CPN_RELEASE_TAG` or set `CPN_STABLE_ONLY=1`.

After the package install:

```bash
sudo cpn-installer
```

On an interactive SSH session, the installer asks whether to use the **Web UI** or **SSH/CLI** wizard (English by default). You can skip the prompt:

```bash
# Web UI (browser; default listen 127.0.0.1:2087)
sudo cpn-installer --web

# SSH/CLI (answer install questions in the terminal)
sudo cpn-installer --cli
```

Default web listen address is `127.0.0.1:2087`. For a remote server, use SSH port forwarding:

```bash
ssh -L 2087:127.0.0.1:2087 root@your-server
```

Then open the URL printed by `cpn-installer --web` in a local browser.

After a successful install, CPN enables and starts `cpn-installer.service` so `/login` remains available after you disconnect SSH and after reboot. Manage it with `systemctl status cpn-installer` / `systemctl restart cpn-installer`.

> [!IMPORTANT]
> Do not publish or share the temporary installer token printed in the console URL.

Optional remote HTTP bind for the web UI (trusted networks only; installer UI is HTTP, not TLS). Needed for some VirtualBox NAT host port forwards when you are not using SSH `-L`:

```bash
sudo cpn-installer --web --allow-remote
```

Example lab: AlmaLinux guest listen `2087`, host NAT forward `2089` -> `2087`, open `http://127.0.0.1:2089/login` on the host (with `--allow-remote` or SSH tunnel).

Installer UI language defaults to **English** (not the guest OS locale). Use the language selector in the web UI for Spanish or Norwegian.
### Manual package install

If you prefer to download artifacts yourself, install from [GitHub Releases](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases). End users do **not** need Rust, Node.js, Docker, `rpmbuild`, or a clone of this repository.

Current release package paths are:

- Enterprise Linux family: `cpn-installer-...el9...rpm` or `...el10...rpm` (EL8 has no native release RPM today).
- Ubuntu/Debian: `cpn-installer_...amd64.deb`.
- Windows Server 2016+ Phase A: `cpn-windows-x86_64.zip`.

Verify checksums and signatures before installing as root: [Releases and Verification](docs/RELEASES.md).

#### AlmaLinux, Rocky Linux, RHEL and compatible EL systems

```bash
sudo dnf install ./cpn-installer-*.rpm
```

Do not mix EL package generations; for example, do not install an EL9 RPM on EL10.

#### Ubuntu and Debian

```bash
sudo apt install ./cpn-installer_*.deb
```

#### Windows Server Phase A

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
> If the newest release does not include an artifact for your OS and architecture, treat that target as unavailable for that release. Maintainer helpers under `scripts/` (build/sign/docker) are separate from the end-user bootstrap scripts (`install.sh`, `preUpgrade.sh`, `upgrade.sh`).

## Upgrading CPN

On a host that already has `cpn-installer` installed, upgrade the package from GitHub Releases (same OS detection and verification as install):

```bash
sh <(curl https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/preUpgrade.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/preUpgrade.sh)
```

After the package upgrade, run installer maintenance when the panel stack was previously installed:

```bash
sudo cpn-installer --upgrade
```

Pin a specific tag when needed: `CPN_RELEASE_TAG=v0.2.4-alpha.19` before the one-liner (or export it in the same shell). Default install/upgrade selection uses the newest **non-draft** GitHub Release (**alphas included**). Set `CPN_STABLE_ONLY=1` only when a future non-prerelease Latest exists and you want to skip alphas. Canonical script copies also live under `scripts/preUpgrade.sh` and `scripts/upgrade.sh`.

## After installation

The `cpn` command is the operator CLI for accounts, websites, network settings, plugins, host apps, and hosting packages.

```bash
cpn --help
cpn list
```

See the full **[CPN CLI and installer argument reference](docs/CLI.md)** for commands and flags such as `cpn site`, `cpn app`, `cpn network`, `cpn package`, `cpn-installer --port`, and `--panel-hostname`.

## Documentation

- **[Changelog](docs/CHANGELOG.md)**: release history from the `0.2.x` alphas through **1.0.2**.
- **[CLI Reference](docs/CLI.md)**: `cpn` commands, subcommands, arguments, and `cpn-installer` runtime flags.
- **[Platform Support](docs/SUPPORT.md)**: supported/partial/refused systems, repeat installs, and behavior on hosts with existing software.
- **[Releases and Verification](docs/RELEASES.md)**: release assets, package selection, checksums, GPG signatures, and provenance.
- **[Contributing Guide](CONTRIBUTING.md)**: source builds, development prerequisites, tests, package build helpers, and pull-request workflow.
- **[Security Policy](SECURITY.md)**: vulnerability reporting and operator security guidance.
- **[Code of Conduct](CODE_OF_CONDUCT.md)**: community participation guidelines.

## License

Copyright (C) 2026 CPN contributors.

CPN is distributed under the [GNU General Public License version 3](LICENSE) (`GPL-3.0-only`).
