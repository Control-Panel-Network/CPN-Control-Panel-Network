# CPN - Control Panel Network

> [!WARNING]
> **Work in progress (not finished).** This version is experimental and is not ready for production servers.

[![Status: in development](https://img.shields.io/badge/status-in%20development-f59e0b)](#project-status)
[![CI](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml/badge.svg)](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)

CPN installs a server panel on AlmaLinux 9. Rust is used only during installation: it serves the HTTP interface, streams real progress over WebSockets, and when finished leaves a prebuilt Next.js Panel with its React UI as a permanent service.

## Project status

The first phase implements the installer flow and is still in development. It currently includes:

- Selection and installation of OpenLiteSpeed, Caddy, or Nginx.
- Selection of SnappyMail, RainLoop, Roundcube, or Thunderbird.
- Real mail backend with Postfix and Dovecot, plus mailbox management from the Panel.
- One-time automatic access from the Panel into Roundcube; other clients open their normal login.
- Real domain validation and a choice between local DNS or Cloudflare via OAuth.
- Real download, install, and verification progress sent over WebSocket.
- Local access by default; `--allow-remote` temporarily opens `8787` and removes the rule when finished.
- RPM packaging for AlmaLinux 9.
- Persistent Panel on `8090` and webmail on `8888`, served by the chosen web engine; the API lives in Next.js and Rust does not stay resident.
- Cloudflare zone OAuth tokens encrypted with ChaCha20-Poly1305 and key files mode `0600`.
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

# React
cd installer-ui
npm ci
npm run lint
npm run build

# Panel Next.js
cd ../Panel
npm ci
npm run lint
npm run typecheck
npm run build
```

The continuous integration workflow runs these checks on every push and pull request.

## RPM packaging

On AlmaLinux 9:

```bash
./scripts/build-rpm.sh
sudo dnf install ./target/rpmbuild/RPMS/x86_64/cpn-installer-*.rpm
sudo cpn-installer                       # local access, recommended with an SSH tunnel
sudo cpn-installer --allow-remote        # explicit remote access
```

When you run `cpn-installer`, the console immediately prints the full URL. The bootstrap link can be used only once; after that it is replaced by an HttpOnly cookie and the URL is cleaned. By default it listens only on `127.0.0.1:8787`.

## Functional tests in Docker

The matrix requires Docker with privileged containers and systemd support:

```bash
./tests/docker-matrix.sh
```

It checks that Nginx, Caddy, and OpenLiteSpeed respond over HTTP, and that SnappyMail, RainLoop, Roundcube, and Thunderbird pass their specific checks. For mail it also validates Postfix, Dovecot, Panel login, mailbox create/delete, and Roundcube SSO.

SnappyMail, RainLoop, and Roundcube are web clients on the Postfix/Dovecot backend installed by CPN. Thunderbird is a desktop client and does not publish a web URL.

## Security

Do not publish the temporary bootstrap link. CPN makes system changes and should only be run on a dedicated test machine during this stage. Supported external artifacts use pinned versions and SHA-256 hashes and are downloaded into private temporary paths. The global Cloudflare OAuth client secret belongs only to the central bridge and is never included in the RPM; each server keeps only its encrypted zone authorization.

See [SECURITY.md](SECURITY.md) for how to report vulnerabilities.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Copyright (C) 2026 CPN contributors.

This project is distributed under the [GNU General Public License version 3](LICENSE) (`GPL-3.0-only`).
