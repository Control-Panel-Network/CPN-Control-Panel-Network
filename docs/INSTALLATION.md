# CPN Installation Guide

> [!WARNING]
> **CPN is under active development and is not ready for production servers.** Use a test VPS or VM and keep backups of any machine you modify.

This document is the canonical end-user installation guide for CPN. Contributors who want to build CPN from source should use [CONTRIBUTING.md](../CONTRIBUTING.md) instead.

## Download a release

Download the package for your operating system from [GitHub Releases](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases).

End users do **not** need Rust, Node.js, Docker, `rpmbuild`, or a clone of this repository to install an official release.

Current release package paths are:

- Enterprise Linux family: `cpn-installer-...el8...rpm`, `...el9...rpm`, or `...el10...rpm`.
- Ubuntu/Debian: `cpn-installer_...amd64.deb`.
- Windows Server 2016+ Phase A: `cpn-windows-x86_64.zip`.

> [!NOTE]
> If the newest release does not contain an asset for your target OS and architecture, treat that target as unavailable for that release. Files under `scripts/` are maintainer/development tools, not an installation requirement.

## Linux installation

### AlmaLinux, Rocky Linux, RHEL and compatible EL distributions

Download the RPM matching the Enterprise Linux major version of the server, then install it with `dnf`:

```bash
sudo dnf install ./cpn-installer-*.rpm
```

Do not install an EL8 or EL9 build on EL10, or otherwise mix package major versions.

### Ubuntu and Debian

Download the `.deb` release package and install it with `apt`:

```bash
sudo apt install ./cpn-installer_*.deb
```

### Start the installer

After the native package is installed:

```bash
sudo cpn-installer
```

The installer listens on `127.0.0.1:2087` by default and prints a temporary URL containing its access token.

For a remote server, prefer SSH port forwarding instead of exposing the installer directly:

```bash
ssh -L 2087:127.0.0.1:2087 root@your-server
```

Open the URL printed by `cpn-installer` in a browser on your local machine.

`--allow-remote` binds the temporary installer to all interfaces. Use it only on a trusted network because the installer UI currently uses HTTP rather than TLS.

Never publish or share the temporary installer token printed in the console URL.

## Windows Server Phase A

Windows support is currently limited compared with the Linux installation path.

1. Download `cpn-windows-x86_64.zip` from GitHub Releases.
2. Extract the archive.
3. Open PowerShell as Administrator.
4. Run:

```powershell
.\Install-Cpn.ps1
```

The default Windows mode binds only to loopback and does not create an inbound firewall rule.

Remote HTTP exposure is opt-in:

```powershell
.\Install-Cpn.ps1 -AllowRemote
```

Use `-AllowRemote` only on a trusted network.

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

CPN recognizes but refuses new installs on Ubuntu 20.04 and Debian 11 because their normal security-support windows have ended. It also refuses openEuler because CPN's third-party web/mail repository stack has not been validated there.

Windows Server 2012/2012 R2 and unknown distributions are refused.

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

## Verify an official release

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
```

For RPM packages, also verify the package signature:

```bash
sudo rpm --import RPM-GPG-KEY-CPN
rpm --checksig ./cpn-installer-*.rpm
```

See [SECURITY.md](../SECURITY.md) for vulnerability reporting and additional operator guidance.

## Docker and Podman are development tools

The privileged systemd containers in this repository are development and smoke-test environments. They are **not** the recommended end-user installation method.

Maintainers can find the relevant workflow in [CONTRIBUTING.md](../CONTRIBUTING.md).
