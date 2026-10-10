# CPN Â· Control Panel Network

> [!WARNING]
> **v1.4.1** is the current stable release (first stable was **v1.0.0**) of CPN Control Panel Network, following the 0.2.x alpha line (last alpha tip v0.2.6-alpha.50). Keep backups, test upgrades on a staging host first, and read [Platform Support](docs/SUPPORT.md) before important hosts.

[![CI](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml/badge.svg?branch=stable)](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml)
[![Docker Hub](https://img.shields.io/badge/docker-master3395%2Fcpn--installer-2496ED?logo=docker&logoColor=white)](https://hub.docker.com/r/master3395/cpn-installer)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)

**CPN** (Control Panel Network) is a Rust web installer and hosting control panel from [News Targeted](https://newstargeted.com), with contributions and support from [Discord Bot Network](https://discord-bot-network.com). Install on AlmaLinux, Rocky, RHEL, Ubuntu, or Debian; manage sites, mail, databases, SSL, and more from the panel. Windows Server has a limited Phase A path.

<p align="center">
  <img src="docs/images/cpn-dashboard.png" alt="CPN Panel dashboard with usage gauges" width="900">
</p>

<p align="center">
  <img src="docs/images/cpn-websites.png" alt="CPN Panel websites list with pagination and Open preview" width="440">
  &nbsp;
  <img src="docs/images/cpn-login.png" alt="CPN Panel sign-in" width="440">
</p>

## Install

On a supported Linux guest (the script requests `sudo` automatically when needed):

```bash
curl -fsSL https://cpn.newstargeted.com/install.sh | bash
```

If the host is down, use GitHub raw:

```bash
curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh | bash
```

Then start the installer:

```bash
sudo cpn-installer          # interactive Web or SSH/CLI
sudo cpn-installer --web    # browser UI (default 127.0.0.1:2087)
sudo cpn-installer --cli    # terminal wizard
```

SSH tunnel for remote hosts: `ssh -L 2087:127.0.0.1:2087 root@your-server`, then open the URL printed by `--web`.

Pins (`-b` / `--ref`), wget hosts, `--bypass`, env vars, and retag notes: **[docs/INSTALL.md](docs/INSTALL.md)**.

## Docker

Official images on [Docker Hub `master3395/cpn-installer`](https://hub.docker.com/r/master3395/cpn-installer) track **v1.4.1** (`stable`). Tags: `almalinux9`, `almalinux10`, `ubuntu26.04`, `latest` (AlmaLinux 9), and the release semver (`1.4.1`). There are no `ubuntu22.04` or `ubuntu24.04` Hub tags; those hosts use native DEB from GitHub Releases.

```bash
docker pull master3395/cpn-installer:almalinux9
docker pull master3395/cpn-installer:almalinux10
docker pull master3395/cpn-installer:ubuntu26.04
docker pull master3395/cpn-installer:latest
docker pull master3395/cpn-installer:1.4.1
```

Privileged systemd run, `scripts/docker-run.sh` (local build), and native install vs containers: **[docs/INSTALL.md](docs/INSTALL.md)**. Prefer RPM/DEB on production hosts.

## Upgrade

```bash
curl -fsSL https://cpn.newstargeted.com/upgrade.sh | bash
```

Pin tip (example):

```bash
curl -fsSL https://cpn.newstargeted.com/upgrade.sh | bash -s -- -b v1.4.1
```

After the package upgrade, `upgrade.sh` auto-runs `cpn-installer --upgrade` when a panel install is detected. Full options: [docs/INSTALL.md](docs/INSTALL.md).

## Uninstall

```bash
sudo cpn-installer --uninstall --yes
# or: sudo cpn uninstall --yes
```

Default removes the panel package and `/var/lib/cpn`, and keeps website files under `/home/<domain>` plus host MariaDB/OLS. Details: [docs/CLI.md](docs/CLI.md).

## After install

```bash
cpn --help
cpn panel url
sudo cpn password            # reset an account password from the terminal
```

## Docs

Host landing + script mirror: **[https://cpn.newstargeted.com/](https://cpn.newstargeted.com/)** (source `site/`, deploy notes [docs/HOST-SITE.md](docs/HOST-SITE.md)).

All guides: **[docs/](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/tree/stable/docs)**

| Doc | Topic |
| --- | --- |
| [INSTALL.md](docs/INSTALL.md) | One-liners, Docker Hub images, pins, fallbacks, `--bypass` |
| [SUPPORT.md](docs/SUPPORT.md) | Supported / partial / refused OS |
| [CLI.md](docs/CLI.md) | `cpn` and `cpn-installer` flags |
| [RELEASES.md](docs/RELEASES.md) | Packages, checksums, GPG |
| [CHANGELOG.md](docs/CHANGELOG.md) | Release history |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Builds and PRs |
| [SECURITY.md](SECURITY.md) | Vulnerability reporting |

## License

Copyright (C) 2026 CPN contributors.

Distributed under the [GNU General Public License version 3](LICENSE) (`GPL-3.0-only`).
