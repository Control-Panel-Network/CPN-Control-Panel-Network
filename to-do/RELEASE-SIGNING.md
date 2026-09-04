# Release signing and provenance (issue #16)

## What CI always produces

On tag `v*` (and manual `workflow_dispatch` dry-run), `.github/workflows/release.yml`:

1. Builds `dist/cpn-installer` with a pinned Rust toolchain and `--locked`.
2. Writes `dist/build-environment.txt` (`rustc -Vv`, cargo, runner, commit).
3. Builds the RPM via `scripts/docker-build-rpm.sh` (which calls `cargo build --release --locked`).
4. Runs `scripts/publish-checksums.sh` to create `dist/SHA256SUMS`.
5. Attaches GitHub Artifact Attestations via `actions/attest-build-provenance`.
6. Uploads `dist/**` as a workflow artifact.
7. On real tags, publishes a GitHub Release with those files.

Local dry-run:

```bash
./scripts/sync-version.sh
cargo build --release --locked
mkdir -p dist && cp target/release/cpn-installer dist/
./scripts/publish-checksums.sh dist/cpn-installer
cat dist/SHA256SUMS
```

## Signing secrets (required for official tag releases)

| Secret | Purpose |
|--------|---------|
| `GPG_PRIVATE_KEY` | Armored private key for `scripts/sign-release.sh` |
| `GPG_PASSPHRASE` | Passphrase for that key (may be empty) |
| `GPG_KEY_ID` | Optional key id / fingerprint |
| `COSIGN_KEY` | Cosign private key PEM for `scripts/sign-cosign.sh` |
| `COSIGN_PASSWORD` | Cosign key password (may be empty) |

Behavior:

- **Tag `v*` releases:** `CPN_REQUIRE_GPG=1`. Missing `GPG_PRIVATE_KEY` fails the job. Detached `.asc` signatures are required; `rpmsign --addsign` runs when `rpm-sign` is available on the runner.
- **Dry-run (`workflow_dispatch`):** signing may skip when secrets are absent; checksums and provenance still ship.

## Operator verification

```bash
# Checksums
sha256sum -c SHA256SUMS

# GPG (when SHA256SUMS.asc is published)
gpg --verify SHA256SUMS.asc SHA256SUMS

# GitHub provenance attestation
gh attestation verify dist/cpn-installer --repo Control-Panel-Network/CPN-Control-Panel-Network
```

## Remaining ops steps (keeps issue #16 open until done)

Checked 04/09/2026: `gh secret list` for this repo shows **no** org/repo GPG secrets visible to the operator token (empty list). Tag `v*` releases will fail closed until secrets are added (`CPN_REQUIRE_GPG=1` in `.github/workflows/release.yml`).

1. Create a release-only GPG key (not a daily-dev key).
2. Store `GPG_PRIVATE_KEY`, `GPG_PASSPHRASE`, and `GPG_KEY_ID` in the GitHub repo secrets (Settings > Secrets and variables > Actions).
3. Publish the public fingerprint in release notes / SECURITY.md.
4. Push a signed tag `vX.Y.Z` and confirm the Release job uploads `SHA256SUMS.asc` plus `*.rpm.asc` (and `rpmsign` when `rpm-sign` is available).
5. Optionally enable cosign secrets for blob signatures.

Until those secrets exist **and** a signed `v*` GitHub Release is published, do not close #16 as complete.

## Bootstrap policy

Future installers that download remote packages should refuse artifacts that fail `SHA256SUMS` verification. Local lab builds remain unsigned by design.
