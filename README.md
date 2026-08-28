# CPN - Control Panel Network

> [!WARNING]
> **Work in progress (not finished).** This version is experimental and is not ready for production servers.

[![Status: in development](https://img.shields.io/badge/status-in%20development-f59e0b)](#project-status)
[![CI](https://github.com/KraoESPfan1n/CPN-Control-Panel-Network/actions/workflows/ci.yml/badge.svg)](https://github.com/KraoESPfan1n/CPN-Control-Panel-Network/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)

CPN is a web installer for preparing server panel components on AlmaLinux 9. A single Rust process serves the HTTP interface, streams real progress over WebSockets, and embeds the React app in the final binary.

## Project status

The first phase implements the installer flow and is still in development. It currently includes:

- Selection and installation of OpenLiteSpeed, Caddy, or Nginx.
- Selection and installation of SnappyMail, RainLoop, Roundcube, or Thunderbird.
- Real download, install, and verification progress sent over WebSocket.
- VPS detection and opening port `8787` in `firewalld` or `ufw` when those are active.
- RPM packaging for AlmaLinux 9.
- Service tests in clean AlmaLinux 9.8 containers.

Recipes, security, and compatibility still need review before CPN can be considered production-ready.

## Structure

- `installer-ui/`: React and Vite interface.
- `Panel/`: React and Next.js control panel based on Stitch screens.
- `src/`: Actix Web server, WebSocket, environment detection, and install recipes.
- `packaging/`: RPM specification.
- `scripts/build-rpm.sh`: builds the binary and RPM on AlmaLinux 9.
- `tests/docker-matrix.sh`: functional matrix for web servers and mail clients.

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

## RPM packaging

On AlmaLinux 9:

```bash
./scripts/build-rpm.sh
sudo dnf install ./target/rpmbuild/RPMS/x86_64/cpn-installer-*.rpm
sudo cpn-installer
```

When you run `cpn-installer`, the console immediately prints the full installer web URL, including the reachable IP and a temporary token. The service listens on `0.0.0.0:8787`.

## Functional tests in Docker

The matrix requires Docker with privileged containers and systemd support:

```bash
./tests/docker-matrix.sh
```

It checks that Nginx and Caddy respond over HTTP, and that SnappyMail, RainLoop, Roundcube, and Thunderbird are installed and pass their specific checks.

## Security

Do not publish the temporary token that appears in the installer URL. CPN makes system changes and should only be run on a dedicated test machine during this stage.

See [SECURITY.md](SECURITY.md) for how to report vulnerabilities.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Copyright (C) 2026 CPN contributors.

This project is distributed under the [GNU General Public License version 3](LICENSE) (`GPL-3.0-only`).
