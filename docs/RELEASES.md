# Releases and Verification

Official CPN releases are published through GitHub Releases. End users should install those native artifacts instead of building packages from source.

## Release assets

Current release paths include:

- Enterprise Linux: `cpn-installer-...el8...rpm`, `...el9...rpm`, or `...el10...rpm`.
- Ubuntu/Debian: `cpn-installer_...amd64.deb`.
- Windows Server Phase A: `cpn-windows-x86_64.zip`.
- checksum/signature/provenance files used to verify the release.

Always match the package to the operating-system family, Enterprise Linux major version, and architecture. The suffix is strict: AlmaLinux 9/RHEL 9/Rocky 9 require `...el9.x86_64.rpm`; EL10 requires `...el10.x86_64.rpm`. Do not install an EL9 RPM on EL10, or an EL10 RPM on EL9, merely because it is the newest RPM in the release.

If a release does not contain an asset for your target OS and architecture, treat that target as unavailable for that release.

## Release verification

Official tagged releases publish `SHA256SUMS`, a signed checksum manifest, the public release key, provenance information, and signatures for signed artifacts.

Release signing fingerprint:

```text
FE70B9718F63B10BB70A6F70BECBB7488AE5C3E5
```

After downloading the release files:

```bash
sha256sum -c SHA256SUMS
gpg --import RPM-GPG-KEY-CPN
gpg --verify SHA256SUMS.asc SHA256SUMS
```

For RPM packages, also verify the RPM signature:

```bash
sudo rpm --import RPM-GPG-KEY-CPN
rpm --checksig ./cpn-installer-*.rpm
```

A failed checksum or signature verification should be treated as a failed installation prerequisite. Do not run an artifact that does not match the signed release metadata.

## Maintainer build scripts

Files under `scripts/` and package definitions under `packaging/` are development/release-maintainer tooling. They are not part of the normal end-user installation process.

For source builds and release-development prerequisites, see [CONTRIBUTING.md](../CONTRIBUTING.md).

For installation, see the root [README](../README.md). For supported platforms, see [SUPPORT.md](SUPPORT.md). For vulnerability reporting and operator security guidance, see [SECURITY.md](../SECURITY.md).
