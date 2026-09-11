# Releases and Verification

Official CPN releases are published through GitHub Releases. End users should install those native artifacts instead of building packages from source.

## Bootstrap one-liners

Preferred end-user path (detects OS, downloads the matching asset, verifies checksums/GPG, installs or upgrades the package):

```bash
# Install (News Targeted host, then GitHub raw)
sh <(curl -fsSL https://cpn.newstargeted.com/install.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh || wget -O - https://cpn.newstargeted.com/install.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh)

# Upgrade (existing cpn-installer install)
sh <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh || wget -O - https://cpn.newstargeted.com/upgrade.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh)
```

Script sources:

- https://cpn.newstargeted.com/install.sh (primary; mirrors `scripts/install.sh`)
- https://cpn.newstargeted.com/upgrade.sh (primary; mirrors `scripts/upgrade.sh`)
- https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh (fallback)
- https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh (fallback)
- https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/preUpgrade.sh (GitHub alias)
- https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/preUpgrade.sh (GitHub alias)

Use `curl -fsSL` (or wget) so a down or non-200 host fails and the next URL in the `||` chain runs. The default production branch is `stable`. Keep `preUpgrade.sh` at the repo root on `stable` so the GitHub alias resolves.

Bootstrap scripts pick the newest **non-draft** GitHub Release by default (**prereleases / alphas included**). Current published tip is **v0.2.4-alpha.19**. Pin with `CPN_RELEASE_TAG=<tag>` when needed. Set `CPN_STABLE_ONLY=1` to skip prereleases once a non-prerelease Latest exists.

Manual download and verification steps below remain valid when you prefer not to use the bootstrap scripts.

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

Most files under `scripts/` and package definitions under `packaging/` are development/release-maintainer tooling. The end-user bootstrap entry points are `scripts/install.sh` and `scripts/upgrade.sh` only.

For source builds and release-development prerequisites, see [CONTRIBUTING.md](../CONTRIBUTING.md).

For installation, see the root [README](../README.md). For release history, see [CHANGELOG.md](CHANGELOG.md). For supported platforms, see [SUPPORT.md](SUPPORT.md). For vulnerability reporting and operator security guidance, see [SECURITY.md](../SECURITY.md).
