# Contributing to CPN

Thank you for contributing to **CPN (Control Panel Network)**. End-user installation belongs in [README.md](README.md); this document covers source builds, validation, and pull requests.

Please also read:

- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security Policy](SECURITY.md)
- [README](README.md)

## Branch target

- Default branch: **`stable`**.
- Open pull requests against `stable`.
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

### Rust on Windows (developer builds)

Linux is the production panel runtime; Windows Server 2016+ is a limited **Phase A** target (installer UI, Windows service, account bootstrap; no web/mail package recipes). You can still develop and run the Rust test suite on a Windows machine.

`webauthn-rs` (passkeys) links OpenSSL through `openssl-sys`, so a Windows build needs an OpenSSL SDK or the vendored feature:

```powershell
# Option 1: system OpenSSL SDK (the "Light" installer has no SDK; install the full package)
winget install --id ShiningLight.OpenSSL --exact   # or: choco install openssl -y

# Set OPENSSL_DIR and OPENSSL_LIB_DIR for this session (add -Persist for new terminals)
. .\scripts\windows-dev-env.ps1

cargo check --locked
cargo test --locked
```

`OPENSSL_DIR` alone is not enough for the common `OpenSSL-Win64` layout; `OPENSSL_LIB_DIR` must point at `lib\VC\x64\MD`, which the script resolves for you (same discovery as `release.yml`).

```powershell
# Option 2: build OpenSSL from source (no SDK needed; requires Perl on PATH, several minutes)
cargo build --features vendored-openssl
```

The vendored feature is opt-in and also works on Linux (for example a static developer build); it needs a C compiler and a full Perl (`dnf install perl-core` on EL, `apt install perl` on Debian/Ubuntu, Strawberry Perl on Windows). Release packages keep linking the system OpenSSL.

Notes:

- The embedded installer UI must exist before a Rust build: `cd installer-ui; npm ci; npm run build`.
- `cargo clippy -- -D warnings` is a Linux CI gate. On Windows, `cfg(unix)` helpers show up as dead-code warnings; run clippy in WSL or on a Linux guest when you need the exact CI result.
- The test suite is expected to pass on Windows (helpers probe `PATH` and `.exe` where Linux uses `which`). Report a Windows-only failure as a bug rather than skipping it.
- WSL (AlmaLinux or Ubuntu) is the quickest way to get the exact Linux CI behaviour, including `clippy` and OS-specific paths. Use a target dir inside the WSL filesystem (for example `CARGO_TARGET_DIR=~/cpn-target`) for speed.
- Pull-request CI stays Linux-only for speed. The manual/weekly `Windows check` workflow (`.github/workflows/windows-check.yml`) runs `cargo check` and `cargo test` on `windows-latest` so a Windows build break is caught before a tagged release builds the Phase A zip.

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
2. Open a pull request targeting `stable`.
3. Explain behavior changes and validation performed.
4. Ensure CI passes: Rust format/check/test/clippy, frontend checks, and script syntax.
5. For distro/package changes, include the relevant OS-matrix result when practical.

## Reporting bugs and ideas

- Use GitHub Issues for non-security bugs and feature ideas.
- Include OS/version, package type, selected web/mail components, and reproduction steps.
- For security-sensitive findings, follow [SECURITY.md](SECURITY.md) instead of opening a public issue.

## License

Contributions are licensed under the same terms as the project: [GPL-3.0-only](LICENSE).
