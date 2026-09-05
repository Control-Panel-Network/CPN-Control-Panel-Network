# CPN — Control Panel Network

> [!WARNING]
> **CPN is under active development and is not ready for production servers.** Use a test VPS or VM and keep backups of any machine you modify.

[![CI](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml/badge.svg)](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)

CPN is a Rust-based web installer and server-control project. The installer embeds its React UI, reports real progress over WebSockets, and prepares web, database, mail, and panel components on supported Linux guests. Windows Server currently has a limited Phase A path.

## Installation

End users install CPN from packages published in [GitHub Releases](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases). You do not need Rust, Node.js, Docker, `rpmbuild`, or a source checkout to install an official release.

For supported operating systems, package selection, Linux/Windows steps, SSH access, existing-host behavior, and release verification, see:

**[Installation Guide →](docs/INSTALLATION.md)**

## Documentation

- **[Installation Guide](docs/INSTALLATION.md)** — supported systems, release packages, Linux/Windows installation, remote access, repeat installs, and signature verification.
- **[Contributing Guide](CONTRIBUTING.md)** — source builds, development prerequisites, tests, package build helpers, and pull-request workflow.
- **[Security Policy](SECURITY.md)** — vulnerability reporting and operator security guidance.
- **[Code of Conduct](CODE_OF_CONDUCT.md)** — community participation guidelines.

## Project structure

- `src/` — Rust installer, detection, install/configuration logic, CLI and panel backend.
- `installer-ui/` — React/Vite installer UI embedded into the Rust binary.
- `Panel/` — Next.js panel frontend.
- `packaging/` — native package and service definitions.
- `scripts/` — maintainer build, signing and container helpers.
- `tests/` — functional installation smoke tests.
- `docs/` — end-user and operator documentation.
- `.github/workflows/` — CI, OS matrix, CodeQL and release automation.

## Development

Source builds are for contributors, not the normal installation path.

See [CONTRIBUTING.md](CONTRIBUTING.md) for Rust/Node prerequisites, validation commands, package build helpers, and repository workflow.

The privileged Docker/Podman systemd containers in this repository are development and smoke-test environments. They are not the recommended end-user installation method.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Copyright (C) 2026 CPN contributors.

CPN is distributed under the [GNU General Public License version 3](LICENSE) (`GPL-3.0-only`).
