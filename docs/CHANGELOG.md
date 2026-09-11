# Changelog

All notable changes to CPN Control Panel Network are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.2-alpha.1] - 11/09/2026

Prerelease for lab testing. GitHub marks this tag as a prerelease; stable `install.sh` / `preUpgrade.sh` keep selecting **v1.0.0** unless you pin `CPN_RELEASE_TAG=v1.0.2-alpha.1` or set `CPN_INCLUDE_PRERELEASE=1`.

### Fixed

- Password-reset emails no longer use quoted-printable body encoding that turned `token=...` into `token=3D...` in raw MIME (broken clickable links). Bodies use 7bit (or base64 when needed) so query strings stay intact.
- Reset and login links no longer blindly prefer a panel hostname that has no public DNS. Operators can set an **external panel URL** (`panel_public_url`) used first for emails and status URLs; otherwise hostname HTTPS; otherwise `http://127.0.0.1:LISTEN_PORT`.
- When the primary email link differs from hostname HTTPS and/or the guest loopback listen URL, the reset email lists those as alternate links (helps VirtualBox NAT and private hostnames).
- Bootstrap scripts (`install.sh`, `preUpgrade.sh`, `upgrade.sh`) skip GitHub prereleases by default so stable one-liners stay on **v1.0.0** while `v1.0.1` and this alpha remain available for explicit testing.

### Added

- Persist optional external panel base URL: `cpn network set-public-url --url http://127.0.0.1:2089` / `clear-public-url`, installer network step, CLI install prompt, and Settings Change Port form. Stored under `/var/lib/cpn/panel_public_url` (mode 600).
- Optional `CPN_INCLUDE_PRERELEASE=1` for bootstrap scripts when you want the newest prerelease without pinning a tag.

## [1.0.1] - 11/09/2026

Published on GitHub as a **prerelease** (display name: `v1.0.1 (alpha / prerelease)`). Tag stays `v1.0.1`; it is not the Latest stable release. Stable one-liners continue to resolve to **v1.0.0**.

### Fixed

- Forgot-password emails now include a **one-time, time-limited reset URL** (`/reset-password?token=...`) instead of an operator-only notice that only linked to `/login`.
- Reset page validates policy, consumes the token once, and signs the user into the panel after a successful password change.
- Installer cancel unit test accepts English `cancelled` as well as Spanish `cancelada` (English-default installer).

### Added

- After a successful web or CLI install, CPN **enables and starts** `cpn-installer.service` so `/login` stays up after SSH disconnect and across reboot (package `%post` only reloads systemd; it did not enable the unit).
- Optional remote bind persistence: when install/start used `--allow-remote` / `CPN_ALLOW_REMOTE=1`, CPN writes `/var/lib/cpn/allow_remote` and a systemd drop-in (`Environment=CPN_ALLOW_REMOTE=1`). Default remains localhost bind.
- End-of-install / MOTD English hints: login URL, `systemctl` manage lines, and VirtualBox NAT host-forward tip (`2089` -> guest port => `http://127.0.0.1:2089/login` on the host).
- CPN-branded SSH login MOTD (`/etc/profile.d/cpn-motd.sh`): panel version, login URL(s), start hints, and host resource stats (load, CPU load-based, RAM, disk `/`, uptime). Installed after a successful web or CLI install, and self-healed when starting `cpn-installer --web` as root. English only; no CyberPanel branding or passwords.
- Short English "panel ready" summary when starting the panel via `--web` / systemd (URL, port, version; password file path only when applicable elsewhere).
- Interactive SSH/CLI installer path: `sudo cpn-installer --cli` (alias `--ssh`). Prompts for web engine, database, phpMyAdmin, panel port, optional hostname, optional mail, and first account without opening a browser.
- Mode choice on interactive TTY when no front-end flag is passed: Web UI or SSH/CLI. Flags: `--web` / `--ui`, `--cli` / `--ssh`. Non-TTY and systemd default to the web UI (`ExecStart=... --web`).
- After the CLI Summary (and on the web network step): choose **Minimal** or **Full detailed** install logging for that run. Minimal shows high-level progress only; Full streams package-manager output. Failures always surface. Full transcript remains in `installation.log`.

### Fixed (installer)

- Installer UI language defaults to English regardless of guest OS/browser locale (previous fallback incorrectly preferred Spanish). Language selector still offers English, Spanish, and Norwegian.
- SnappyMail HTTP validation: OpenLiteSpeed no longer 301-redirects `/data` before deny; rewrite returns 403 for bare and slashed sensitive paths. Health check also rejects redirect chains that end in HTTP 200.
- Missing legacy `cpn-webmail` systemd unit: disable is skipped quietly when the unit file is absent.
- Proxy-front nginx package install leaves nginx stopped so it does not fight OpenLiteSpeed for `:80` before the front is wired.
- Post-install panel not listening: successful installs now enable/start `cpn-installer.service` instead of leaving the unit disabled.

### Changed

- Installer console, recipe titles, wait heartbeats, and install-engine errors default to English (same English-default policy as the UI locale).
- Long package-manager waits keep progress messaging that reads as in-progress, not failed/hung.
- README and CLI docs describe Web UI vs SSH/CLI invocation.

## [1.0.0] - 11/09/2026

First stable release after the `0.2.x` alpha line. Install and upgrade from signed GitHub Release packages (EL9/EL10 RPM, Ubuntu/Debian `.deb`, Windows Phase A zip). Prefer a disposable test host; keep backups.

### Highlights

- Official install one-liner via `https://cpn.newstargeted.com/install.sh`, with GitHub raw fallback on `stable`.
- Upgrade one-liner via root `preUpgrade.sh` / `scripts/upgrade.sh` (pin with `CPN_RELEASE_TAG` when needed).
- Signed releases: `SHA256SUMS`, GPG signatures, RPM signing, SBOM, and GitHub provenance attestation.
- Web installer embeds the React UI, reports progress over WebSockets, and can bind remotely with `--allow-remote` (HTTP; trusted networks only).
- Operator CLI (`cpn`) for accounts, websites, network, plugins, host apps, and packages.
- Panel UI: responsive sidebar (hamburger/drawer), nested nav, light/dark toggle, traffic-light CPU/RAM/Disk gauges, feature-gated hub tiles.
- Hosting foundations: OpenLiteSpeed / Nginx / Caddy recipes, MariaDB Manager defaults with phpMyAdmin, Postfix fallback when SMTP is unset, SnappyMail webmail path.
- Per-site SSL modes (Let's Encrypt, ZeroSSL, Cloudflare CA, custom, none), Cloudflare DNS helpers, Wildcard/SAN coverage options.
- Safe jailed SFTP per website and subdomain; site registry and backups under domain homes.
- Plugins and themes catalogs (Control-Panel-Network org), domain-keyed install paths, ACL for install/reinstall/uninstall.
- Account security: TOTP 2FA and Passkeys (WebAuthn); MFA material stored per install under `/var/lib/cpn/mfa/`.
- Default panel port **2087** (Cloudflare-friendly), choosable at install and changeable later.

### Install and packaging (0.2.x → 1.0.0)

- Bootstrap scripts detect AlmaLinux / Rocky / RHEL (EL9/EL10) or Ubuntu / Debian and refuse unsupported OS versions closed.
- Manual RPM/DEB/Windows zip install paths documented in the README and [RELEASES.md](RELEASES.md).
- Release workflow publishes matching `el9` / `el10` RPMs, `.deb`, Windows zip, checksums, and signatures for each tagged release.
- `scripts/sync-version.sh` keeps Cargo.toml and RPM Version/Release aligned (no hyphen in RPM Version).

### Panel and operator UX (selected 0.2.x work)

- Dashboard gauges with green/orange/red thresholds.
- Users & Plans, Security, and Settings hubs with real icons and populated tiles.
- Notifications popover that is not clipped by the sidebar.
- Sidebar brand, page/feature search, and privacy-blurred host IP reveal/copy.
- Installer language select (en/es/nb); first-account setup with lowercase default `admin`.
- Login Remember me stores username only.

### Platform notes

- Primary smoke targets: AlmaLinux 9 / 10 and Ubuntu 22.04 / 24.04.
- Windows Server remains Phase A (no Linux web/mail recipe parity).
- EL8 has no native release RPM while the OpenSSL 1.1 / WebAuthn constraint remains.
- See [SUPPORT.md](SUPPORT.md) for supported, partial, and refused guests.

### Upgrade from 0.2.2 alphas

```bash
sh <(curl https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/preUpgrade.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/preUpgrade.sh)
sudo cpn-installer --upgrade
```

Pin this release with `CPN_RELEASE_TAG=v1.0.0` if another tag is temporarily newest.

## [0.2.2] alphas (summary)

Pre-1.0 development line (`v0.2.2-alpha.1` … `v0.2.2-alpha.18`). Notable themes:

- Install/upgrade bootstrap one-liners and News Targeted `/install.sh` mirror.
- Release signing, checksums, GPG, SBOM, and provenance.
- Panel auth (session cookie, login without installer token after bootstrap).
- Sidebar/layout, gauges, hubs, plugins/themes, Cloudflare DNS / SSL coverage.
- MariaDB + phpMyAdmin defaults, SnappyMail, CLI surface, MFA/Passkeys foundations.
- Dependency and CI hardening through the alpha.18 cut.

For per-tag PR lists, see the corresponding [GitHub Releases](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases) notes.

[1.0.2-alpha.1]: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases/tag/v1.0.2-alpha.1
[1.0.1]: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases/tag/v1.0.1
[1.0.0]: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases/tag/v1.0.0
[0.2.2]: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases?q=0.2.2-alpha
