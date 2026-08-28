# Contributing to CPN

Thank you for your interest in contributing to **CPN (Control Panel Network)**. This document explains how to work on the project and submit changes.

Please also read:

- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security Policy](SECURITY.md)
- [README](README.md)

## Branch target

- Default branch: **`main`**
- Open pull requests against `main`
- Prefer a short-lived topic branch for each change (for example `feature/...` or `fix/...`)

## Getting started

### Prerequisites

- Rust toolchain (stable) with `cargo`, `rustfmt`, and `clippy`
- Node.js and npm (for `installer-ui` and `Panel`)
- AlmaLinux 9 or a compatible environment for RPM builds and Docker matrix tests
- Git

### Clone and setup

1. Fork [Control-Panel-Network/CPN-Control-Panel-Network](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network) on GitHub if you do not have write access.
2. Clone your fork (or the upstream repo if you are a collaborator):

```bash
git clone https://github.com/YOUR_USERNAME/CPN-Control-Panel-Network.git
cd CPN-Control-Panel-Network
```

3. Create a topic branch from `main`:

```bash
git checkout main
git pull origin main
git checkout -b feature/your-change
```

4. Install front-end dependencies when you touch those trees:

```bash
cd installer-ui && npm ci && cd ..
cd Panel && npm ci && cd ..
```

## Project layout

| Path | Role |
|------|------|
| `src/` | Rust installer server (Actix Web, WebSocket, install recipes) |
| `installer-ui/` | React + Vite installer UI (embedded in the binary) |
| `Panel/` | React + Next.js control panel UI |
| `packaging/` | RPM packaging |
| `scripts/` | Build helpers (for example `build-rpm.sh`) |
| `tests/` | Functional checks (for example `docker-matrix.sh`) |

## Local validation

Run the checks that apply to your change before opening a pull request.

### Rust

```bash
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings
```

### Installer UI

```bash
cd installer-ui
npm ci
npm run lint
```

### Panel

```bash
cd Panel
npm ci
npm run lint
npm run typecheck
```

### Optional Docker matrix

Requires Docker with privileged containers and systemd support:

```bash
./tests/docker-matrix.sh
```

## Making changes

- Keep pull requests focused and reviewable
- Match existing code style in the area you edit
- Update `README.md` or related docs when behavior or setup steps change
- Do **not** commit secrets, API keys, or the temporary installer token shown in the console URL
- Do not paste live installer URLs with tokens into issues or pull requests

### Commit messages

Use clear, descriptive messages. Conventional style is welcome:

```
type(scope): brief description

Optional longer explanation of why the change is needed.
```

Examples of `type`: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`.

## Pull request process

1. Push your topic branch to GitHub.
2. Open a pull request targeting **`main`**.
3. Describe what changed and how you tested it.
4. Ensure CI checks pass (Rust fmt/check/test/clippy and front-end lint/typecheck as configured).
5. Respond to review feedback promptly.

Maintainers may request smaller commits, extra tests, or documentation updates before merging.

## Reporting bugs and ideas

- Use GitHub Issues for non-security bugs and feature ideas
- Include OS version, how you ran CPN (binary, RPM, Docker), and steps to reproduce
- For security-sensitive findings, follow [SECURITY.md](SECURITY.md) instead of opening a public issue

## License

By contributing, you agree that your contributions are licensed under the same terms as the project: [GPL-3.0-only](LICENSE).
