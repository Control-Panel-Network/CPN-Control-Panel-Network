# Contributing to CPN

Thank you for contributing to **CPN (Control Panel Network)**. End-user installation belongs in [README.md](README.md); this document covers source builds, validation, and pull requests.

Please also read:

- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security Policy](SECURITY.md)
- [README](README.md)

## Branch target

- Default branch: **`main`**.
- Open pull requests against `main`.
- Prefer a short-lived topic branch such as `feature/...` or `fix/...`.

## Prerequisites

- Rust stable with `cargo`, `rustfmt`, and `clippy`.
- Node.js 22 and npm for `installer-ui` and `Panel`.
- Git.
- For package/matrix work: a supported Linux guest or Docker/Podman capable of privileged systemd containers.

## Clone and setup

1. Fork [Control-Panel-Network/CPN-Control-Panel-Network](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network) if you do not have write access.
2. Clone your fork or the upstream repository.

```bash
git clone https://github.com/YOUR_USERNAME/CPN-Control-Panel-Network.git
cd CPN-Control-Panel-Network
git checkout -b fix/your-change
```

Install frontend dependencies only when you need those trees:

```bash
cd installer-ui && npm ci && cd ..
cd Panel && npm ci && cd ..
```

## Project layout

| Path | Role |
|---|---|
| `src/` | Rust installer, OS/service detection, install recipes, CLI/backend |
| `installer-ui/` | React + Vite installer UI embedded in the binary |
| `Panel/` | Next.js control panel UI |
| `packaging/` | RPM/DEB service and package inputs |
| `scripts/` | Maintainer build, signing, release and container helpers |
| `tests/` | Functional smoke tests |
| `.github/workflows/` | CI, OS matrix, CodeQL and release automation |

## Local validation

### Rust

```bash
cargo fmt --check
cargo check --locked
cargo test --locked
cargo clippy --locked -- -D warnings
```

### Installer UI

```bash
cd installer-ui
npm ci
npm run lint
npm run build
```

### Panel

```bash
cd Panel
npm ci
npm run lint
npm run typecheck
npm run build
```

### Shell scripts

At minimum, run `bash -n` on any shell script you changed. CI checks the repository's maintained shell entry points.

### Native package builds

These are **developer/release commands**, not end-user installation steps:

```bash
# Build an EL-family RPM in a matching AlmaLinux container
CPN_ALMA_VERSION=9 ./scripts/docker-build-rpm.sh

# Other release majors
CPN_ALMA_VERSION=8 ./scripts/docker-build-rpm.sh
CPN_ALMA_VERSION=10 ./scripts/docker-build-rpm.sh

# Build the apt-family package on a controlled baseline
CPN_BUILD_IMAGE=ubuntu:22.04 ./scripts/docker-build-deb.sh
```

Official tagged releases are built by `.github/workflows/release.yml`; users should download those artifacts rather than build packages locally.

### OS matrix

The functional matrix requires privileged containers and systemd. It is deliberately kept out of untrusted pull-request execution.

```bash
./tests/docker-matrix.sh
```

The manual `OS matrix` workflow exercises additional distro versions. If you change OS detection, packaging, repository bootstrap, web/mail installation, or service behavior, update that matrix rather than only changing the README support table.

## Making changes

- Keep pull requests focused and reviewable.
- Let `rustfmt` format Rust; avoid unrelated whitespace churn.
- Preserve idempotency: an existing valid package/service/configuration should be adopted or validated where safe instead of blindly replaced.
- Never claim a distro as fully supported without an implemented package path and repeatable smoke evidence.
- Update docs when behavior, support tiers, or installation steps change.
- Do **not** commit secrets, API keys, signing keys, installer tokens, or live tokenized installer URLs.

## Pull request process

1. Push the topic branch.
2. Open a pull request targeting `main`.
3. Explain behavior changes and validation performed.
4. Ensure CI passes: Rust format/check/test/clippy, frontend checks, and script syntax.
5. For distro/package changes, include the relevant OS-matrix result when practical.

## Reporting bugs and ideas

- Use GitHub Issues for non-security bugs and feature ideas.
- Include OS/version, package type, selected web/mail components, and reproduction steps.
- For security-sensitive findings, follow [SECURITY.md](SECURITY.md) instead of opening a public issue.

## License

Contributions are licensed under the same terms as the project: [GPL-3.0-only](LICENSE).
