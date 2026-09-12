# Changelog

All notable changes to CPN Control Panel Network are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.6-alpha.27] - 12/09/2026

Guest-matched EL RPM selection for maintenance (Cargo `0.2.6-alpha.27`). Retags the fix that landed on `stable` after `v0.2.6-alpha.26` was already published from the pre-fix tip.

### Fixed

- Maintenance / Version Management installs the guest-matched `.elN` RPM via `compatible_package_asset`, not the first GitHub `.rpm` (often `.el10` before `.el9`). Fixes AlmaLinux 9 labs that failed with `libc.so.6(GLIBC_2.39)` / generic `Package install failed (dnf/rpm)`.
- `install_rpm` surfaces the real dnf/rpm stderr in the UI.
- Version Management progress label shows a numeric percent (example: `60% installing: ...`).
- Installed package prefers live `rpm -q` (mapped to Cargo prerelease) when the install-manifest is stale (example: manifest `0.2.2-alpha.17` vs RPM `0.2.6-alpha.24`).

### Notes

- Host scripts: `https://cpn.newstargeted.com/install.sh` / `upgrade.sh` with `-b` / `--ref` pin; see `docs/INSTALL.md`.

## [0.2.6-alpha.26] - 12/09/2026

Ship Email webmail UX, MTA-STS/BIMI, and tip RPM helpers as a GitHub Release tip (Cargo `0.2.6-alpha.26`). Prior `v0.2.6-alpha.25` tag pointed at pre-webmail tip.

### Notes

- Includes changelog items from `0.2.6-alpha.24` (webmail / MTA-STS / BIMI) and `0.2.6-alpha.25` (Version-Release tip RPM accept).
- EL-matched RPM maintenance fix shipped in **0.2.6-alpha.27** (not in the original `v0.2.6-alpha.26` package binaries).

## [0.2.6-alpha.25] - 12/09/2026

Webmail UX + MTA-STS/BIMI (from alpha.24 line) plus same Version-Release tip RPM accept during maintenance (Cargo `0.2.6-alpha.25`).

### Fixed

- Compare RPM Version-Release before/after dnf so tip re-runs after `upgrade.sh` succeed even when NEVRA string compares fail.

## [0.2.6-alpha.24] - 12/09/2026

Email webmail UX (SnappyMail/Roundcube), MTA-STS/BIMI DNS helpers, and harder same-NEVRA RPM apply (Cargo `0.2.6-alpha.24`).

### Added

- **Email > Webmail**: detect installed SnappyMail/Roundcube on disk (fixes false "Not configured yet" after panel restart). Open client, Admin Panel, regenerate public path, auto-login Email prefill (best-effort), and optional internal iframe at `/email/webmail/app`.
- Panel reverse-proxy for the configured webmail mount (default `/snappymail` or `/roundcube`) to loopback PHP-FPM `127.0.0.1:8080`.
- Built-in settings fields for `snappymailWebmail` / `snappymailAdmin` / `roundcubeWebmail` plugins (auto-login, internal embed, public path) plus Open / Admin / Regenerate actions.
- Sidebar: active webmail plugins with Show in sidebar appear under **Email** (not only Installed plugins).
- **Email > MTA-STS** and **Email > BIMI**: policy storage, recommended DNS, copy-friendly UI, optional Cloudflare add/update push (no unrelated deletes). Honest notes that receivers/MTA and brand indicators matter more than SnappyMail/Roundcube logo support.

### Changed

- Install manifest preserve list includes `webmail-panel.json` and `email-auth/`.

### Fixed

- `install_rpm` checks installed NEVRA before dnf, and falls back to `rpm -Uvh --force` so same-tip maintenance after package upgrade no longer fails with Package install failed (dnf/rpm).

## [0.2.6-alpha.23] - 12/09/2026

Same-NEVRA success during forced same-version upgrade after bootstrap `upgrade.sh` (Cargo `0.2.6-alpha.23`).

### Fixed

- `cpn-installer --upgrade` treats an already-installed tip RPM as success even when the same-version path sets `force` (previously only non-force runs skipped reinstall).

## [0.2.6-alpha.22] - 12/09/2026

Allow leftover retired `1.0.0`/`1.0.1` package identities to move onto current `0.2.x-alpha` via official upgrade, plus `-b`/`--ref` pin for bootstrap scripts (Cargo `0.2.6-alpha.22`).

### Added

- Bootstrap `-b REF` / `--branch REF` / `--ref REF` (and `CPN_BRANCH`) on `install.sh` / `upgrade.sh`: pin packages to a matching GitHub Release tag (`REF` or `vREF`). Shared helpers in `scripts/cpn-bootstrap-lib.sh`. Docs: [INSTALL.md](INSTALL.md).
- Host serves `cpn-bootstrap-lib.sh` next to `install.sh` / `upgrade.sh`.

### Fixed

- Official `upgrade.sh` / `cpn-installer --upgrade` treats installed `1.0.0` or `1.0.1` (GitHub retag leftovers) as a **retag migration** onto published `0.2.x`, not a hostile downgrade. Uses `rpm -Uvh --oldpackage` (erase+install fallback) or `apt-get --allow-downgrades`. Non-interactive; no extra confirmation beyond running the official upgrade path.
- Stops DNF/RPM failures of the form "same or higher version already installed" when replacing those retired identities with tip `0.2.x-alpha` RPMs.

### Notes

- Docker opt-in remains `upgrade.sh --bypass` or `CPN_UPGRADE_BYPASS=1` (unchanged from alpha.21).
- `1.0.0-dev` is an optional tracking branch name, not a stable 1.0 product release.

## [0.2.6-alpha.21] - 11/09/2026

Safer upgrades: auto panel maintenance from `upgrade.sh`, allowlisted stale packaging cleanup, post-upgrade service verify, and opt-in Docker refresh via `--bypass` (Cargo `0.2.6-alpha.21`).

### Added

- After a successful package upgrade, `scripts/upgrade.sh` (and `preUpgrade.sh` when it delegates) **automatically runs** `cpn-installer --upgrade` when a panel install is detected (`/var/lib/cpn/install-manifest.json` or `panel-bootstrap.json`). Primary path no longer asks the operator to run maintenance by hand.
- Post-upgrade **cleanup** of stale CPN packaging/staging only (`/var/tmp/cpn-upgrade-*`, `/var/tmp/cpn-gpg-*`, installer status temp, `.bak`/`.old` installer binaries, obsolete `/opt/cpn-webmail` extract dirs). Preserves websites, apps, Docker stacks/volumes, plugins, SSL, MFA, `/etc/cpn`, and `/var/lib/cpn` configs. When in doubt, keep; skipped preservations are logged.
- Post-upgrade **verification** of panel service + `/login`, enabled web server units, MariaDB/MySQL when present, webmail/php-fpm when installed, and mail units when active. Required failures abort maintenance with a clear English error.
- Opt-in Docker refresh: `cpn-installer --upgrade --bypass` or `CPN_UPGRADE_BYPASS=1` / `upgrade.sh --bypass`. Refreshes only CPN-managed compose under `/var/lib/cpn/docker` and containers labeled `com.cpn.managed=1`. Preserves volumes. Default: leave all Docker stacks as-is.

### Changed

- Install manifest default preserve list expanded (MFA, SSL, docker prefs, listen_port, panel URLs, `/etc/cpn`, `/home`).

## [0.2.6-alpha.20] - 11/09/2026

Version Management UI, empty-asset release skip, and same-NEVRA upgrade tolerance (Cargo `0.2.6-alpha.20`).

### Added

- **Version Management** (`/settings/version`): panel admins can list GitHub releases and run upgrade / downgrade / repair from the web UI with two-step confirm and a live progress bar (`GET /api/maintenance/status`). Panel session auth is accepted alongside the installer token for version-check and maintenance APIs (fixes HTTP 401 on the settings page).

### Changed

- `POST /api/maintenance` requires `confirm_execute: true` for upgrade/downgrade/repair (installer UI and CLI send it). Downgrade still requires `confirm_downgrade`.
- Install/upgrade/preUpgrade release selection skips tags that have no matching package assets yet (for example a just-published prerelease while the Release workflow is still uploading), and falls through to the newest release that has `SHA256SUMS` plus an elN/arch RPM or arch `.deb`.

### Fixed

- `cpn-installer --upgrade` treats an already-installed same NEVRA RPM as success (bootstrap `upgrade.sh` may have installed the package before the installer binary runs).
- CLI help docs note that `--help` is a flag on `cpn` / `cpn-installer`, not a bare shell command.

## [0.2.6-alpha.19] - 11/09/2026

Cool live SSH MOTD and `cpn panel url` (Cargo `0.2.6-alpha.19`). Includes prior tip work from `0.2.5-alpha.19` (password-reset MIME / public URL).

### Added

- Cool CPN-branded interactive SSH MOTD (`/etc/profile.d/cpn-motd.sh`): ASCII banner, "This server has installed CPN", live login URL(s), start hints, load/CPU/RAM/disk/uptime. English only; no CyberPanel branding or passwords.
- Operator commands: `cpn panel url`, `cpn panel status`, `cpn info` (status alias), and `cpn panel install-motd` (root). `--raw` for scripts; `--motd` for indented MOTD embedding.
- MOTD resolves login URL(s) **live** on every interactive SSH login by calling `cpn panel url --motd` (fallback: `/etc/cpn` world-readable mirror, then `/var/lib/cpn`). Changing the panel port in the UI updates the next SSH banner without reinstall.
- Non-secret login facts (`listen_port`, `panel_public_url`, `panel_hostname`) are mirrored to `/etc/cpn/` (mode 644) whenever the panel writes them, so non-root SSH users see the same live URL while `$CPN_DATA_DIR` stays mode 700 for secrets.

### Changed

- Panel-ready banner prefers live `panel_public_url`, then hostname, then loopback listen port, and points operators to `cpn panel url`.

## [0.2.5-alpha.19] - unreleased (folded into 0.2.6 tip)

Password-reset MIME / DNS reachability fix line (Cargo was `0.2.5-alpha.19`). Not published as a GitHub Release; carried into `0.2.6-alpha.19`.

### Fixed

- Password-reset emails no longer use quoted-printable body encoding that turned `token=...` into `token=3D...` in raw MIME (broken clickable links). Bodies use 7bit (or base64 when needed) so query strings stay intact.
- Reset and login links no longer blindly prefer a panel hostname that has no public DNS. Operators can set an **external panel URL** (`panel_public_url`) used first for emails and status URLs; otherwise hostname HTTPS; otherwise `http://127.0.0.1:LISTEN_PORT`.
- When the primary email link differs from hostname HTTPS and/or the guest loopback listen URL, the reset email lists those as alternate links (helps VirtualBox NAT and private hostnames).
- Trailing slash is accepted on `panel_public_url` (normalized away). Old-port redirect helpers keep using the request Host instead of forcing loopback.

### Added

- Persist optional external panel base URL: `cpn network set-public-url --url http://127.0.0.1:2089` / `clear-public-url`, installer network step, CLI install prompt, and Settings Change Port form. Stored under `/var/lib/cpn/panel_public_url` (mode 600).

### Changed

- Bootstrap scripts and in-panel upgrade default to the newest **non-draft** GitHub Release including **alphas**. Optional `CPN_STABLE_ONLY=1` skips prereleases when a future non-prerelease Latest exists.
- Official install/upgrade one-liners use `cpn.newstargeted.com` first, then GitHub raw (`stable/scripts/install.sh` / `upgrade.sh`) when the site is down (`curl -fsSL` / wget chain). Prefer `/upgrade.sh` in user-facing copy; `preUpgrade.sh` remains a GitHub alias.

## [0.2.4-alpha.19] - 11/09/2026

Renamed from former **`v1.0.1`** (tag and release removed). Same signed artifacts; package filenames inside the release still say `1.0.1` so SHA256SUMS / GPG stay valid. GitHub prerelease only (no stable Latest).

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

## [0.2.3-alpha.19] - 11/09/2026

Renamed from former **`v1.0.0`** (tag and release removed). Same signed artifacts; package filenames inside the release still say `1.0.0` so SHA256SUMS / GPG stay valid. GitHub prerelease only (no stable Latest).

First post-`0.2.2` alpha packaging cut after the `0.2.x` line. Install and upgrade from signed GitHub Release packages (EL9/EL10 RPM, Ubuntu/Debian `.deb`, Windows Phase A zip). Prefer a disposable test host; keep backups.

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

### Install and packaging (0.2.x → 0.2.3-alpha.19)

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

Pin with `CPN_RELEASE_TAG=v0.2.3-alpha.19` or `CPN_RELEASE_TAG=v0.2.4-alpha.19` when you need a specific cut.

## [0.2.2] alphas (summary)

Pre-1.0 development line (`v0.2.2-alpha.1` … `v0.2.2-alpha.18`). Notable themes:

- Install/upgrade bootstrap one-liners and News Targeted `/install.sh` mirror.
- Release signing, checksums, GPG, SBOM, and provenance.
- Panel auth (session cookie, login without installer token after bootstrap).
- Sidebar/layout, gauges, hubs, plugins/themes, Cloudflare DNS / SSL coverage.
- MariaDB + phpMyAdmin defaults, SnappyMail, CLI surface, MFA/Passkeys foundations.
- Dependency and CI hardening through the alpha.18 cut.

For per-tag PR lists, see the corresponding [GitHub Releases](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases) notes.

[0.2.6-alpha.21]: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases/tag/v0.2.6-alpha.21
[0.2.6-alpha.20]: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases/tag/v0.2.6-alpha.20
[0.2.6-alpha.19]: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases/tag/v0.2.6-alpha.19
[0.2.5-alpha.19]: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/compare/v0.2.4-alpha.19...v0.2.6-alpha.19
[0.2.4-alpha.19]: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases/tag/v0.2.4-alpha.19
[0.2.3-alpha.19]: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases/tag/v0.2.3-alpha.19
[0.2.2]: https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases?q=0.2.2-alpha
