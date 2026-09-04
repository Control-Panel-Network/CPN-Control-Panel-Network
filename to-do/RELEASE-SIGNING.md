# Release signing and provenance (issue #16)

## Official signing identity

| Field | Value |
|-------|-------|
| Fingerprint | `EA5B57B1230FFA37F4426A873EC7121204EB5515` |
| Public key | [`packaging/RPM-GPG-KEY-CPN`](../packaging/RPM-GPG-KEY-CPN) |
| Key purpose | Release-only (RPM + `SHA256SUMS` + binaries). Not a daily development key. |

Import and confirm the fingerprint before trusting any package:

```bash
gpg --import packaging/RPM-GPG-KEY-CPN
gpg --fingerprint EA5B57B1230FFA37F4426A873EC7121204EB5515
```

## What CI produces on tag `v*`

`.github/workflows/release.yml` (and manual `workflow_dispatch` dry-run):

1. Builds `dist/cpn-installer` / `dist/cpn` from the tagged commit with a pinned Rust toolchain and `--locked`.
2. Writes `dist/build-environment.txt` (`rustc -Vv`, cargo, runner, commit, fingerprint).
3. Builds the RPM via `scripts/docker-build-rpm.sh` (AlmaLinux container, `cargo build --release --locked`).
4. **Embeds an RPM package signature** with `rpmsign` inside AlmaLinux 9 (`scripts/sign-rpm-almalinux.sh`), then writes detached `.asc` files.
5. Writes `dist/SHA256SUMS` **after** RPM signing (so hashes match the signed packages).
6. Detached-signs `SHA256SUMS` and binaries; regenerates/re-signs after the CycloneDX SBOM is added.
7. Attaches GitHub Artifact Attestations (`actions/attest-build-provenance`).
8. Publishes a GitHub Release with all of `dist/**` (including `RPM-GPG-KEY-CPN` and `GPG-FINGERPRINT.txt`).

Tag releases **fail closed** without `GPG_PRIVATE_KEY`. Dry-run may skip signing when secrets are absent.

## Verify before running as root

```bash
# Download release assets into a clean directory, then:
sha256sum -c SHA256SUMS
gpg --import RPM-GPG-KEY-CPN
gpg --verify SHA256SUMS.asc SHA256SUMS
rpm --import RPM-GPG-KEY-CPN
rpm --checksig ./*.rpm
./scripts/verify-release.sh ./*.rpm ./SHA256SUMS ./SHA256SUMS.asc
gh attestation verify ./cpn-installer --repo Control-Panel-Network/CPN-Control-Panel-Network
```

## Installer / upgrade verification

Remote upgrade/repair downloads (`src/upgrade.rs`) verify SHA-256 against the release `SHA256SUMS` by default.

| Env | Default | Effect |
|-----|---------|--------|
| `CPN_VERIFY_RELEASE` | on | Refuse install if checksums are missing or do not match |
| `CPN_VERIFY_GPG` | off | When `1`, also require a valid `SHA256SUMS.asc` (needs `gpg`) |
| `CPN_VERIFY_RELEASE=0` | | Lab-only opt-out for unsigned local builds |

## GitHub Actions secrets

| Secret | Required for tag `v*` | Purpose |
|--------|------------------------|---------|
| `GPG_PRIVATE_KEY` | yes | Armored private key for `scripts/sign-release.sh` / `sign-rpm-almalinux.sh` |
| `GPG_PASSPHRASE` | yes (may be empty) | Passphrase for that key |
| `GPG_KEY_ID` | recommended | Fingerprint / key id (`EA5B57B1230FFA37F4426A873EC7121204EB5515`) |
| `COSIGN_KEY` | optional | Cosign PEM for `scripts/sign-cosign.sh` |
| `COSIGN_PASSWORD` | optional | Cosign password (may be empty) |

Set with:

```bash
gh secret set GPG_PRIVATE_KEY --repo Control-Panel-Network/CPN-Control-Panel-Network < private.asc
gh secret set GPG_PASSPHRASE --repo Control-Panel-Network/CPN-Control-Panel-Network
gh secret set GPG_KEY_ID --repo Control-Panel-Network/CPN-Control-Panel-Network --body EA5B57B1230FFA37F4426A873EC7121204EB5515
```

Never commit the private key. Rotate and update `packaging/RPM-GPG-KEY-CPN` if the release key is compromised.

## Local checksum dry-run (unsigned)

```bash
./scripts/sync-version.sh
cargo build --release --locked
mkdir -p dist && cp target/release/cpn-installer dist/
./scripts/publish-checksums.sh dist/cpn-installer
cat dist/SHA256SUMS
```
