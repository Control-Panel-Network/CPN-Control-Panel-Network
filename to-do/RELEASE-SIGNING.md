# Release signing and provenance (issue #16)

## What CI always produces

On tag `v*` (and manual `workflow_dispatch` dry-run), `.github/workflows/release.yml`:

1. Builds `dist/cpn-installer` with a pinned Rust toolchain.
2. Writes `dist/build-environment.txt` (`rustc -Vv`, cargo, runner, commit).
3. Runs `scripts/publish-checksums.sh` to create `dist/SHA256SUMS`.
4. Attaches GitHub Artifact Attestations via `actions/attest-build-provenance`.
5. Uploads `dist/**` as a workflow artifact.
6. On real tags, publishes a GitHub Release with those files.

Local dry-run:

```bash
./scripts/sync-version.sh
cargo build --release
mkdir -p dist && cp target/release/cpn-installer dist/
./scripts/publish-checksums.sh dist/cpn-installer
cat dist/SHA256SUMS
```

## Optional secrets (never invent or commit values)

| Secret | Purpose |
|--------|---------|
| `GPG_PRIVATE_KEY` | Armored private key for `scripts/sign-release.sh` |
| `GPG_PASSPHRASE` | Passphrase for that key (may be empty) |
| `GPG_KEY_ID` | Optional key id / fingerprint |
| `COSIGN_KEY` | Cosign private key PEM for `scripts/sign-cosign.sh` |
| `COSIGN_PASSWORD` | Cosign key password (may be empty) |

When secrets are absent, signing steps **skip successfully**. Checksums and provenance still ship.

## Operator verification

```bash
# Checksums
sha256sum -c SHA256SUMS

# GPG (when SHA256SUMS.asc is published)
gpg --verify SHA256SUMS.asc SHA256SUMS

# GitHub provenance attestation
gh attestation verify dist/cpn-installer --repo Control-Panel-Network/CPN-Control-Panel-Network
```

## RPM GPG signing (operator / offline)

Official RPMs should use a release-only key:

```bash
rpmsign --addsign dist/*.rpm
```

Document the public fingerprint in the release notes. Until a release key is configured in GitHub secrets, treat unsigned lab RPMs as non-production.

## Bootstrap policy

Future installers that download remote packages should refuse artifacts that fail `SHA256SUMS` verification. Local lab builds remain unsigned by design.

## Hard requirements (issue #16)

- `cargo build --release --locked` (no unlocked fallback)
- RPM build must succeed (no `continue-on-error`)
- Real CycloneDX SBOM (no placeholder JSON)
- Provenance attestation covers binary, RPM, checksums, SBOM, and build-environment notes
- GPG/cosign remain optional until release secrets exist; absence skips signing without inventing keys

