# Release signing and provenance (issue #16)

## Current state

Publish SHA-256 checksums without a signing key:

```bash
./scripts/sync-version.sh
./scripts/build-rpm.sh
./scripts/publish-checksums.sh target/rpmbuild/RPMS/*/*.rpm
```

Attach `SHA256SUMS` to the GitHub Release.

## Remaining (needs secrets / infra)

1. GPG release key separate from day-to-day development keys.
2. `gpg --detach-sign --armor SHA256SUMS` and publish `SHA256SUMS.asc`.
3. `rpmsign --addsign` on official RPMs.
4. Tag release job: pinned Rust version, `rustc -Vv` notes, SBOM, optional SLSA attestations.
5. Document the public fingerprint operators must verify.

Until those land, treat unsigned CI/local RPMs as lab-only.
