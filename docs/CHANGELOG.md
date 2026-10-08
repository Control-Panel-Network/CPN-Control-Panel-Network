# Changelog

All notable changes to CPN Control Panel Network are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Mr Agent host policy**: Panel owner toggles on `/plugins/mr-agent` (`POST /plugins/mr-agent/host-policy`), stored at `/var/lib/cpn/mr-agent/host-policy.json`. `allow_host_chat` (default on) gates the panel bubble and `/plugins/mr-agent`. `allow_site_install` (default off) blocks Store Site Install with a clear error. Existing site installs are left in place. Visibility ACL remains separate.
- **Mr Agent Host/Site install modes**: Catalog dual scope (Host + Site). Host install serves panel chat with no site takeover. Site install defaults to folder publish under `/mr-agent/` (site index kept). Vhost takeover requires explicit confirmation in Store/Activate and never runs silently. New panel page `GET /plugins/mr-agent` is the bubble Expand target (does not depend on site docroot). Auto folder publish after site Install when the guest supports symlinks.
- **Plugin float widgets (Mr Agent bubble)**: Panel shell injects Active site plugins that declare `panel_float` (Mr Agent implied). New routes: `GET /plugins/float-asset` (serves `public/assets/panel-float/*` only) and `POST /plugins/float-chat` (ACL-gated PHP bridge for mrAgent). Visibility ACL mirrors plugin settings (`admins_only` / `all_authenticated` / `packages`). Install/enable no longer strips catalog `settings_fields` from `cpn-plugin.json`. Builtin Mr Agent settings fields cover older stripped installs. Sidebar Feedback icon is unchanged.
- **Version Management installer log**: `/settings/version` has a collapsible SSH-style transcript (closed until a job is in progress). `/api/maintenance/status` includes a redacted `log` tail from `/var/lib/cpn/upgrade-session.log` (same stream as CLI cargo/package output). Failures use a `FAILED` prefix. The last run stays until the next upgrade. Tokens are not included.

### Changed

- **Version Management commit copy**: User-facing Version UI says commits instead of tip (`Upgrade to latest commits`, `Building panel from stable commits (cargo)`, `Stable commits:`). CLI `--to stable` is unchanged. Cargo cache dirs still use `tip-<sha>`.
- **Site preview cache TTL**: Successful homepage thumbnails under `/var/lib/cpn/site-previews/` stay Fresh for **7 days** (was 24 hours). JSON sidecars store `expires_at`. `/websites` and `/subdomains` list loads serve disk cache or placeholders only: they never N+1 Microlink (or other remote screenshot APIs). Background auto-capture uses local Chromium only when no usable cache exists. **Refresh preview** still force-recaptures (local first, Microlink only when no browser and remote previews are enabled). Microlink requests are debounced host-wide and reuse Fresh disk cache when `force` is false.
- **Version Management release picker**: Lists all discovered GitHub Releases that have installable assets (paginated API fetch, empty-asset tags skipped), not only the newest 20. Support policy text stays latest two releases only; selecting an older tag still warns outside support. CLI `--to` / `--downgrade` and `/api/releases` use the same full publishable list so operators can upgrade or downgrade later.

### Fixed

- **Post-upgrade verify false fail on tip skip-rebuild**: When the installed binary already matches the tip SHA, upgrade no longer restarts the panel mid-verify or schedules a detached reload. `/login` probes accept HTTP `200` and maintenance `503` (panel up behind the upgrade gate), retry for about 40 seconds before failing, and Version UI marks the job completed when services answer.
- **Version Management Repair false orphan at 96%**: Repair/upgrade package apply and post-apply verify no longer get marked failed because cargo/rustc is absent. The in-process job flag, RPM/DEB/dnf/apt children, and registered child PIDs keep the lock until the worker actually finishes. Real dead workers still clear so retry works. Repair of the selected release reinstalls that package and reports success (not a scare failure).
- **Commit source builds missing CPN_BUILD_SHA**: Local `cargo build --release` now embeds a strings-visible `CPN_BUILD_SHA=` marker (`#[used]` keep-static, `CPN_BUILD_SHA`/`CPN_GIT_SHA` cargo env, `.cpn-git-sha`, git HEAD). After a commit cargo build, unmarked leftover ELFs are stamped only when they have no marker (mismatched SHA still refused). Extracted older trees get a keep module so verify can pass. Unmarked binaries that were not just built for that SHA are still refused.
- **Open preview PHP download and redirect parity**: `/preview/{domain}/content/` no longer serves a PHP index as `application/octet-stream` (browser download). When the primary index is PHP on a public hostname (for example a ddns `index.php` that redirects to apex), Preview fetches the live HTTPS origin, follows redirects, and sets `<base href>` to the final origin so alias/ddns sites match Visit site. Local-only PHP hosts get an HTML explanation instead of a download. Open preview still opens in a new tab (`target=_blank`, `rel=noopener`). Stub or empty docroots keep the live-origin path. Local Chromium remains primary for thumbnails; Microlink stays a fallback for public hostnames only.
- **Version DEB apply on Ubuntu/Debian**: `/settings/version` upgrade, repair, and downgrade now install the guest-matching `.deb` from GitHub Releases (`apt-get install` on a local path, then `dpkg -i` plus `apt-get install -f` if apt refuses the file). The previous stub that refused DEB package apply is gone. SHA-256 verification is unchanged. The running panel is not stopped by `pkill`; detached reload after apply is unchanged.
- **Passkey / WebAuthn after restart**: `/login/2fa` keeps one in-flight `credentials.get` (abort stale requests, no duplicate pending error). Passkey files live under `/var/lib/cpn/mfa/passkeys/` (legacy `$CPN_DATA_DIR/passkeys/*.json` is copied, never deleted). Loopback RP ID stays `localhost` with remembered `localhost` and `127.0.0.1` origins across upgrade, downgrade, repair, and restart. Missing MFA pending session redirects to `/login`. WebAuthn failures log to Main/Error without secrets.
- **Website Open preview**: Opens in a new tab (`target=_blank`, `rel=noopener`). When the document root is the CPN Site ready stub, missing, or has no `index.html` / `index.htm` / `index.php`, `/preview/{domain}/` and Site preview captures use the live public HTTPS origin instead of an empty directory listing. Local Chromium remains primary; Microlink stays a fallback for public hostnames only.

## [1.3.0] - 05/10/2026

Minor release after v1.2.0. Operator-facing features: Dashboard meters follow the signed-in user's assigned package, Database disk is Used of the package quota, Feedback sends branded HTML with a CID logo and public Panel host, and Plugin Store uses a Host/Site install target. Tip-upgrade cargo PATH healing and Postfix loopback Feedback relay land in the same cut.

### Added

- **Assigned package dashboard meters**: Statistics Used of limit cards follow the signed-in user's assigned package (Manage user Account tab picker), not the last-edited package. Websites, mailboxes, databases, FTP/SFTP, storage, and bandwidth use the same package fields.
- **Plugin Store Host/Site install target**: Store filters switch Host vs Site without a full reload. Host is owner/admin only (site dropdown hidden). Email stays in the Email category when the target changes. Tachyon is listed first among Host Email options.
- **Feedback CID logo and public Panel host**: HTML Feedback embeds the CPN logo as a CID image. Panel host uses the public server IP or hostname plus port from `panel_public_url`, never `127.0.0.1`, localhost, or lab NAT guest IPs when a public address exists.

### Changed

- **Dashboard Database disk meter**: Statistics shows Database disk as Used X of Y like Storage. The denominator is the package `database_disk_mb` quota (`-1` = unlimited / infinity, `0` = none allowed). Existing packages without the field default to unlimited. Used is live MariaDB schema size for owned databases.
- **Feedback HTML mail**: `POST /api/panel/feedback` sends `multipart/alternative` (plain text plus branded HTML). Recipients stay `info@newstargeted.com` and `info@discord-bot-network.com`. From display name is `CPN Panel` without changing the envelope address. User fields are HTML-escaped. Body includes a `dd/mm/yyyy` 24-hour UTC timestamp. HTML is last, uses `text/html; charset=utf-8` and 8bit transfer so clients like SnappyMail prefer the branded card over the plain fallback.

### Fixed

- **Version Upgrade to stable tip**: Commit/tip upgrades no longer hard-fail when `cargo` is missing from the systemd PATH. The apply path searches rustup homes (`/home/cpn/.cargo/bin`, `/root/.cargo/bin`, `/usr/local`), tries GitHub Actions tip binaries for the commit SHA, and can install rustup once on lab/source hosts. Failures (missing cargo/npm, compile, package apply, detached reload) are written to `/var/log/cpn/panel.log` (Main Log) and `/var/log/cpn/error.log` (Error logs) with timestamp, module `upgrade_tip`, message, and retry count. Secrets stay redacted. Detached panel reload after apply is unchanged. Binary replace uses a same-directory rename so the running `/usr/bin/cpn-installer` is not truncated (that produced Exec format error).
- **Light theme sidebar contrast**: Light mode now resets readable ink/surface tokens after Theme Store CSS and locks sidebar labels, nested items, search, IP card, and footer icons to dark-on-light. Dark mode tile pairing is unchanged.
- **Feedback mail delivery**: `POST /api/panel/feedback` uses the owner-configured outbound provider (`smtp.json`) when present, otherwise local Postfix. Local injection uses a dedicated `127.0.0.1:2525` listener (no SASL, no virtual-mailbox reject) so support inboxes are not treated as hosted aliases. The send path waits for that listener after `postfix reload` instead of falling back to port 25. Failures return a precise operator error (no secrets).
- **Feedback Postfix 5.1.1 bounce**: The `:2525` injector now uses a dedicated `panelout-cleanup` service with empty virtual maps. Hosted-mail sync no longer lists domains in `virtual_alias_domains` when Postfix `inet_interfaces` is loopback-only (this host cannot be the public MX). A lab mailbox such as `smoke@newstargeted.com` therefore no longer makes `info@newstargeted.com` bounce locally; Feedback relays to the real MX. Existing `master.cf` listeners are rewritten on the next Feedback send.
- **Feedback dialog placement**: The sidebar keeps the Feedback button. The form opens as a viewport-centered modal (body portal, backdrop, Esc, focus trap) so aside overflow and drawer transform cannot pin it to the nav column.

## [1.2.0] - 04/10/2026

Minor release after v1.1.2. Ships package limit schema 2 (`0` hard zero, `-1` unlimited), Dashboard Statistics first, LIVE email provisioning meters, Apps lifecycle, sidebar Feedback, Ubuntu 26.04 OpenLiteSpeed install/repair, and File Manager hardening.

### Changed

- **Package limit semantics**: Only `-1` means unlimited (infinity symbol). `0` means none allowed (Used 0 of 0). Existing packages stored under schema 1 that used `0` as unlimited are migrated to `-1` on first load (schema 2). Package create/edit/bulk copy and Statistics meters use the new rules.
- **Dashboard Statistics**: Stock overview order puts Statistics first. Missing stock widgets (including Statistics) are re-inserted at their default relative positions so a custom layout cannot bury or drop the meters unnoticed.

### Added

- **Email provisioning meters**: Package limits and Statistics rows for mailing lists, autoresponders, forwarders, and email filters. LIVE panel routes at `/email/lists`, `/email/autoresponders`, `/email/filters`, plus Postfix map apply for forwarders. Database disk size is shown on Statistics (informational MariaDB schema sum).
- **Sidebar feedback modal**: Signed-in users can open Feedback beside the sidebar theme toggle and send categorized feedback without leaving the current page. `POST /api/panel/feedback` validates the session, same-origin request, HMAC CSRF token, required fields, length limits, and a five-per-hour account rate limit, then uses the configured SMTP or local Postfix path to deliver identical messages to both support inboxes with user, host, and panel-version context.
- **Website Manage Apps tab**: Clean URL `/websites/manage?domain=...&tab=apps` (also `/websites/apps`). Per-site cards for CMS Made Simple (2.2.x installer in the site document root), Redis (host install once, site Activate/attach), Node, and Python (version picker, jailed app path, start/status). Host engines stay host-wide. Site users cannot uninstall host packages. WordPress stays under WordPress.
- **Apps backup-first lifecycle**: Apps cards and `cpn apps` now show installed and source-available versions, support install/update/upgrade/downgrade, and list/restore backups. CMS Made Simple uses its official installer source; Redis, Node, and Python use configured OS repositories. Mutating version changes create a restore point before package or site files change.
- **Category hub overview tiles**: Sidebar category overviews render as LIVE/SCAFFOLD tile hubs (Users-style) instead of dumping into list pages.

### Fixed

- **Ubuntu 26.04 OpenLiteSpeed install and repair**: OLS apt setup now temporarily disables the source while bootstrapping prerequisites, verifies both official LiteSpeed signing-key fingerprints, installs a repository-scoped keyring with `signed-by`, and only then refreshes apt. This removes the first-run `NO_PUBKEY` loop on `resolute`. Runtime ownership resolves the `nobody` user's actual primary group (`nogroup` on Ubuntu), canonical `lshttpd` service units are preferred over linked `lsws` aliases, and vendor-generated WebAdmin credentials are redacted from installer events and logs. Installer stack selection configures and starts the normal OLS vhost/listeners, while System Repair can refresh the keyring/repository and restart an existing OLS install without silently switching web servers.
- **Root File Manager (`/server/files`)**: The GET page no longer walks `/home` (or other restore-sized trees) on first load. Listing is a bounded JSON call (`/server/files/list`) with a time and entry cap. Slow listings return **503** JSON/UI instead of a hung connection that the browser reports as **408**. File Manager CSS/JS are versioned (`?v=panel-version-fm1`) so upgrades cache-bust without hashed asset filenames that 404. POST `/server/files/op` and `/server/files/upload` stay compatible for open tabs after update.
- **File Manager DOM XSS**: File Manager row builders use safe DOM APIs (no HTML string assembly of names/paths) and ship `fm.js` as UTF-8 text.
- **Sidebar Theme Store contrast**: Sidebar nav stays readable across Theme Store color modes (paired light/dark tile background and text after design CSS vars).

## [1.1.2] - 03/10/2026

Patch release after v1.1.1. Ships Website/WordPress list splits, origin vs Cloudflare SSL badges, live Site preview for stub sub-domains, storage unit scaling, sidebar hub overviews first, WordPress install/WP-CLI fixes, and origin Let's Encrypt backup with auto retry.

### Changed

- **Sidebar hub overview first**: Every expandable category (Settings, Email, Websites, WordPress, Users, Security, Databases, FTP, Plugins, Docker, Logs, LiteSpeed, and the rest) now lists `{Category} overview` as the first child under the parent, linking to the hub URL. Feature children stay after that (Email Accounts, List Websites, Version Management, and so on). This reverses Email overview-after-children.

- **Website SSL badges**: **Valid** remains origin certificate files on this host with a real expiry (`dd/mm/yyyy`). **CF SSL** is shown when origin files are missing and either Cloudflare orange-cloud proxy is on for the FQDN or the site SSL provider is Cloudflare CA (issuance setting, not a Valid origin cert). **NONE** is neither origin files nor those Cloudflare signals. Lab NAT grey-cloud records stay honest: n/a expiry still means no origin files. List and SSL tab copy is shorter. The old **INSECURE** label is gone.

- **Hosting package names**: New packages (Create and Duplicate) are stored as `{username}_{customname}` using the package owner username (Create form owner selector; defaults to the signed-in admin). Typing an existing `{owner}_` prefix is not doubled. The reserved **Default** package (`pkg-default`) stays named exactly `Default`. Edit keeps the owner prefix and only changes the custom part. CLI `cpn package create` requires `--owner`.

### Added

- **Origin Let's Encrypt backup**: SSL tab, websites list, and Manage SSL offer **Issue origin backup** so origin HTTPS still works if Cloudflare proxy stops. POST `/security/ssl/origin-backup`. Does not treat Cloudflare edge TLS as a local Valid origin cert. Auto retry (bounded background pass on `/websites` and `/subdomains` load, plus a 15 minute loop) issues origin Let's Encrypt when origin files are missing or Cloudflare/chosen SSL is not working: heals certbot if missing, prefers HTTP-01 for apex/SAN, uses DNS-01 only for wildcard when a Cloudflare token is present, persists the last error, and backs off so ACME is not spammed.

- **Website list cards**: Disk is this site's home/docroot used storage of the package disk allowance (`Used 19.3 GB of 488.3 GB`). Package bandwidth uses the same wording (never a bare `Quota` without used). Units never print `B` (KB floor). Cards also show Package, PHP, and IP when already stored.
- **WordPress install plugin ZIP upload**: `/wordpress/install` accepts plugin ZIP files (base64 staging under `/var/tmp/cpn-wp-plugin-uploads`) in addition to slug/URL sources. Full site ZIP restore stays under Backups / Restore.
- **WP-CLI public phar for site-user installs**: Ensure WP-CLI publishes a world-readable copy at `/usr/local/lib/cpn/wp-cli.phar` so site-user WP-CLI runs can open the phar when `/var/lib/cpn` is mode 700.
- **Websites and WordPress list split (one sidebar group each)**: Main domains at `/websites` (create at `/websites/create`). Nested sites at `/subdomains` (create at `/subdomains/create`). Manage Domains tab still shows parent/child cards. WordPress uses `/wordpress` vs `/wordpress/subsites` (matching install paths). Sidebar has one **Websites** group (List/Create Website plus List/Create Sub-domain) and one **WordPress** group (sites, install, sub-sites, sub-site install). Hiding Websites or WordPress in ACL also gates the nested list routes.
- **List search (`q=`)**: `/websites`, `/subdomains`, `/wordpress`, and `/wordpress/subsites` filter by domain (and parent on sub lists), case-insensitive, with a mobile-friendly search row matching other CPN lists.
- **Delete on split lists**: `/subdomains` cards keep Manage plus POST `/websites/delete` (same as `/websites` and Manage Overview). `/wordpress` and `/wordpress/subsites` site cards include Delete WordPress (POST `/wordpress/delete`). Successful site delete returns to `/websites` or `/subdomains`; WordPress delete returns to `/wordpress` or `/wordpress/subsites`.

### Fixed

- **Site preview for sub-domains**: `/preview/{domain}/` no longer serves the CPN **Site ready** placeholder when the local docroot is still the default stub. Preview fetches the live public URL (`https://` for internet hostnames) with an 8s timeout, injects a `<base href>` so CSS/images load from origin, and keeps Preview Mode chrome. Thumbnails on `/subdomains` Refresh from that live origin with a local headless browser (no loopback rewrite). Cache is dropped on refresh. Remote screenshot quota / PRO-plan copy is not shown; Microlink runs only when no browser binary exists. System Repair can install `chromium-headless` / Chromium. Discover AlmaLinux `headless_shell` paths and cache under `/var/lib/cpn/site-previews/`.
- **WordPress install / WP-CLI**: Plugin ZIP upload on install/create, public WP-CLI phar fetch, and Ensure showing the real WP-CLI version (not a blank or stale string).
- **WordPress one-click install**: Admin username defaults to the signed-in CPN account, failed installs retain form values, and WP-CLI root warnings are sanitized. Core download prefers curl/tar with longer WP-CLI timeouts.

## [1.1.1] - 03/10/2026

Patch release after v1.1.0. Hardens Email MTA-STS/BIMI under load, Tachyon Admin About and webmail proxy heal, large restore plan timeouts, System Repair responsiveness, Email Accounts sidebar order, Version Management clarity, and login-service-gate test races.

### Fixed

- **Email MTA-STS / BIMI white page or 408**: `/email/mta-sts` and `/email/bimi` now render via `html_blocking` (20s hub budget) so unlock checks and `panel_shell` host probes do not pin an Actix worker under disk load. Unlock prefers the O(1) host flag / host-plugin path before scanning site plugin trees. Each page reads the sites registry once for the default domain and dropdown.
- **Email MTA-STS / BIMI Recommended DNS overflow**: Recommended DNS no longer uses a crushed `data-table` (Name/Value letter-wrap). Records render as full-width padded cards (Websites-list style) with sensible wrapping; policy and push actions stay usable on narrow viewports.
- **Large classic restore plan timeout**: `/backups/restore/plan` no longer fails after 15s when inventorying multi-GB source control-panel archives. Listing uses a 2h budget, filters deep website noise, caches members under `/var/lib/cpn/backup-inventory/`, and large restores prefer selective path extract to reduce staging disk use.

- **System Repair hang / Busy stub**: `/server/system-repair` no longer runs the full probe suite inside the hub HTML render (which hit the 20s `gathering host status` 503). The page shell returns immediately and loads check cards from `/server/system-repair/api` with a 16s budget, 20s result cache, and 6s fail-fast per collector group. Cloudflare live verify is skipped on this path (use DNS Test connection). Login/getenforce probes use short timeouts.

- **Tachyon Admin About Json reset / empty PHP table**: Tachyon `DoAdminInfo()` calls GitHub Releases before returning system load and PHP extensions. Slow or failed outbound HTTPS stalled admin Json (browser `ERR_CONNECTION_RESET` / empty PHP table / load averages `0` / "Cannot access the repository"). The same local stub directory now includes `core-github.json`, and Tachyon `Repository::httpGet()` is patched to read it (and `packages.json`) first. Webmail PHP-FPM `open_basedir` also includes `/proc` for loadavg.

- **Webmail panel proxy heal stall**: first `/tachyon/` (or `/snappymail/`) request after panel start ran `heal_webmail_loopback_config()` inside `Once` on the request thread (SELinux `restorecon` / sieve sync). That blocked proxy workers until heal finished, so the browser saw timeouts / connection resets. Heal now runs on a background thread.
- **Email Accounts sidebar order**: Email hub overview sits after mailbox children so **Email Accounts** (`/email/accounts`) is not easy to miss-click.
- **Version Management UX**: clearer Current vs installed package, installed-at timestamp, Refresh without blank flash, and red/orange/green package age colors.
- **Mobile sidebar**: drawer stays above the nav backdrop so links stay clickable on small screens.
- **Login service gate cache**: `CPN_LOGIN_SERVICE_GATE=0` always bypasses a hot not-ready cache (stops flaky login 503 in CI). Listen-port tests recover from a poisoned `DATA_DIR_TEST_LOCK`.

## [1.1.0] - 02/10/2026

Second stable release after v1.0.0. Ships System Repair, Version tip-commit updates, branded upgrade maintenance page, MariaDB password/delete, Services status improvements, login/2fa passkey session fixes, backup restore plan hardening, Activity Board polish with editable dashboard layout, and faster phpMyAdmin Open auto-login.

### Added

- **Version Management stable tip updates**: `/settings/version` compares the running panel commit (build embed, install-manifest `source_commit`, or installed release tag SHA) to the configured repo `stable` branch HEAD. Shows **Update available** when the tip SHA differs even if the semver string is still `1.0.0`. Operators can **Upgrade to stable tip** (commit/source build labeled `stable @ abc1234`) beside the existing release picker. No new GitHub Release tag is required for incremental `stable` merges.
- **Install-manifest vs RPM identity**: stale `0.2.x-alpha.*` `install-manifest.json` no longer wins over a live `1.0.0` RPM; version resolve and reconcile prefer the newer package identity.
- **System Repair** (owner diagnostics): Panel hub at `/server/system-repair` (Settings tile redirects from `/settings/system-repair`) with pass/warn/fail cards and safe **Heal** actions for CLI shadows, panel service, email stack/ports/firewall, phpMyAdmin, firewalld, and Docker/Podman. Extends `cpn doctor` with aliases `troubleshoot` / `repair` / `system-repair`, subcommands `check` / `heal`, `--json`, and `--id`. MFA storage is checked only (never wiped).
- **Panel upgrade maintenance page**: While an owner upgrades or repairs from **Settings > Version** or `cpn-installer --upgrade` / `--repair` / `--downgrade`, CPN writes `/var/lib/cpn/maintenance.json` and shows every visitor a dark branded maintenance card (progress, auto-retry, optional admin/bypass). Flag clears on success, failure, TTL (45m), startup heal after restart, or `cpn doctor --heal`. See `docs/PANEL-UPGRADE-MAINTENANCE.md`.
- **Dashboard overview layout**: Signed-in users can Edit overview, drag widgets (Sites, gauges, Tools, health, Activity Board) on a wide screen, Save the order, or Restore default (with confirmation). Order is stored in per-user prefs under `/var/lib/cpn/user-prefs/`. JSON APIs: `GET`/`POST /api/panel/dashboard-layout` and `POST /api/panel/dashboard-layout/restore`.
- **Dashboard Activity Board polish**: Default page size is 5 on every tab. Traffic and Disk IO show grouped numbers and static share charts (Minimalist mode stays snapshot-until-refresh). Top Process truncates long commands and offers Manage for the full line plus Server → Top Processes. Each row has More/Manage details. The board is collapsed by default and remembers expand via the same layout prefs.
- **Open phpMyAdmin auto-login latency**: `/databases/phpmyadmin/open` and the `/phpmyadmin/` proxy no longer re-run configuration-storage SQL (`create_tables.sql`), recursive TempDir `chown`, package queries, or OpenLiteSpeed/php-fpm rewrites when the sign-on bridge, storage marker, FPM socket, and `:8081` listener are already healthy. Optional Docker/Podman probes stay off this path. The dedicated FPM pool keeps a spare worker (`pm = dynamic`) so the first PHP request is not an ondemand cold start. Heal still runs when the sock, listener, or runtime stamp is missing (no restart thrash).
- **Services status**: /server/services shows Active / Inactive / Enabled / Deactivated / Not installed with Plugin Store install CTAs and accurate Docker vs Podman detection.
- **MariaDB Manager**: Change database passwords and confirmed delete with impact preview (protected users never touched).
- **Auth login/2fa**: Passkey session expiry and blank 408 handling for /login and /login/2fa.
- **Backup restore plan**: Bounded inventory scan so restore planning does not reset the connection.


## [1.0.0] - 01/10/2026

First stable release. Version identity moves from the 0.2.6-alpha.50 line to 1.0.0 (Cargo, RPM and DEB metadata). This cut contains everything merged on `stable` up to the alpha.50 tip, including the Logs hub with real access, FTP/SFTP and ModSecurity viewers, per-site monthly bandwidth metering, admin-only Users and ACL pages, the unified Plugins Store, the Docker Active Containers UI, webmail clients under Email, and the Docker Hub tags `latest`, `almalinux9`, `almalinux10` and `ubuntu26.04`. See the sections below for per-feature detail.

### Added

- **Package bandwidth metering**: Monthly transfer is now measured from each site's access log into a per-site ledger under `/var/lib/cpn/bandwidth/` (incremental reads, survives log rotation, resets each calendar month). **Packages > Your limits** shows used and limit with a percentage and an Over limit badge, the site Overview bandwidth card shows the month total against the package limit, and creating a new website is blocked while the account is over its monthly bandwidth limit (the same soft policy as disk). The "metering later" placeholder is gone.
- **Admin only Users & ACL pages**: Non-admin accounts no longer see Create New User, Create ACL, or Modify ACL in the sidebar or the Users & Plans hub. Opening those URLs directly shows an in-page 403 that names the signed-in account, and denied actions redirect with the short code `error=admin-only` (no spaces or `%20` in the URL).
### Changed

- **Docker Hub**: Retired the `master3395/cpn-installer` tags `ubuntu22.04` and `ubuntu24.04` (Docker Scout Medium/Low findings in base-distro packages with no Canonical fix yet, issue #353). Docker Hub now publishes `almalinux9`, `almalinux10`, `ubuntu26.04`, `latest`, and the release semver. Ubuntu 22.04 and 24.04 hosts remain supported through the native DEB install; only the container runtime tags changed.

### Added

- **Settings > Site messages**: Default suspend copy and site-ready templates now use the same Markdown/HTML editor as **Error messages** (formatting toolbar, Preview, and View HTML source). Suspend messages are stored as Markdown and rendered safely for visitors. Factory defaults include tasteful **CPN Control Panel Network** / News Targeted branding. Shared Markdown preview POST: `/settings/markdown/preview`; site-ready iframe preview: `/settings/site-messages/preview-site-ready`.

### Changed

- **Docker Active Containers UI**: Create Container moved to dedicated `/docker/create` (GET+HEAD); `/docker` lists containers only with a **+ Create Container** link. Container status shows **Running** (green, with uptime) or **Stopped** / **Dead** (red). Active Containers, Manage Images, and Compose Stacks use a stacked card list at all panel widths (no horizontal table scroll); actions wrap inside each card.
- **Settings > Error messages**: Richer built-in 403/404/500 Markdown defaults and dark-mode friendly rendered pages. **View HTML source** toggles the sanitized HTML fragment beside Preview.

### Fixed

- **Docker bind mount host paths**: Create Container and Compose stack provisioning now create bind-mount host directories under `/var/lib/cpn/docker-data/` with mode `775` and ownership aligned to the container image user when inspect succeeds (default UID/GID 1000), so empty root-owned mounts no longer cause permission denied crash loops on first start.
- **Docker Active Containers UI**: Status shows **Restarting** in red when the engine reports a restart loop, separate from **Running** and **Stopped**.

- **Docker routes and HEAD monitoring**: `/docker`, `/docker/images`, `/docker/stacks`, `/docker/view/{name}`, legacy `/server/docker/*`, and related GET pages now register HEAD alongside GET so `curl -I` returns the same redirect or success status as GET (no 404 from the catch-all).
- **Podman / docker-compatible pulls without TTY**: Hub search, manual pull, Create Container, and compose stack refresh now qualify unqualified Docker Hub names to `docker.io/...` before `pull`/`run`, strip podman-docker shim noise from panel errors, and heal Podman hosts with `/etc/containers/nodocker` plus `unqualified-search-registries = ["docker.io"]` on Docker Host package install when Podman is the engine.
- **Docker Host package compose provider**: Install/heal now pulls in `docker-compose-plugin` (Docker CE / moby) or `podman-compose` (Podman + podman-docker) so `docker compose` works for `/docker/stacks` Pull & Recreate. Re-running Install on an engine-only host adds the missing compose packages without touching CPN-managed stacks.
- **Compose on Podman**: CPN runs `docker compose` from the stack project directory with a relative `-f` file (no `--project-directory`) so `podman-compose` works on AlmaLinux Podman labs.

### Added

- **Site preview thumbnails via screenshot API fallback**: List Websites (`/websites`) and Manage Overview now show a real homepage thumbnail for public domains even when the host has no local headless browser. Local Chromium capture stays primary and keeps writing to the authenticated cache under `/var/lib/cpn/site-previews/`. When no cached shot exists, the card image points at the Microlink screenshot API (`https://api.microlink.io/?url=https://<domain>&screenshot=true&embed=screenshot.url`, 24h `ttl`) and falls back to the existing SVG placeholder if that request fails. `Refresh preview` still triggers a server-side capture: local backends first, then a forced remote shot stored in the cache with its real content type. Only publicly resolvable hostnames are sent: loopback, private IPs, and reserved suffixes such as `.local`, `.test`, and `.internal` stay local-only. New Websites toggle `Turn off screenshot service` (POST `/websites/preview-prefs`, pref `remote_site_previews` in `panel-ui.json`) disables the remote path for operators who do not want site hostnames leaving the server. Background captures are de-duplicated per domain so repeated list loads do not stack capture runs.
- **Website staging (Manage > Clone/Staging)**: `POST /websites/clone` creates a separate staging site (default `staging.<domain>` or `staging-<label>.<parent>`) with toggles (default on) for files, MariaDB (registry + wp-config), site plugins/host activations, cron jobs, and site-linked Docker compose stacks. Staging gets new DB users/passwords (never logged), wp-config remap when detected, `.cpn-staging-db-map.json` (mode 600) under the site home, Cloudflare DNS via the normal create flow, and `staging_of` on site JSON. Linked compose stacks clone to a new project under `<data>/docker/` with copied `<data>/docker-data/` and `docker compose up -d`; production stacks are left running. Manage shows production/staging links. Promote/sync to production remains phase 2.

- **Docker Compose stacks (persistent data)**: `/docker/stacks` creates CPN-managed compose projects under `<data>/docker/<stack>/` with bind-mounted host data under `<data>/docker-data/<stack>/data`. Quick templates default to upstream images (nginx:alpine, mariadb:11, redis:7-alpine). **Pull & Recreate** runs `docker compose pull` then `up -d` (volumes preserved). Hub search ranks Docker Official Images first; copy steers operators toward maintainer images, not custom CPN-only images. Create Container accepts optional bind mounts. Upgrade `--bypass` reuses the same compose refresh helper.

- **Docker Hub auto-publish**: `.github/workflows/docker-hub.yml` runs on every `v*` tag push (GitHub Releases from `GITHUB_TOKEN` do not trigger `release:published` for other workflows). Pushes `master3395/cpn-installer` tags `almalinux9`, `almalinux10`, `ubuntu22.04`, `ubuntu24.04`, `ubuntu26.04`, `latest` (AlmaLinux 9 baseline), and the release semver. Dockerfiles live under `docker/`. Documented in `docs/RELEASES.md` and `docs/OS-SUPPORT-MATRIX.md`.
- **Ubuntu 26.04 support**: `os_support`, maintainer `.deb` builds, bootstrap install checks, OS matrix smoke, and Hub runtime images treat Ubuntu 26.04 / 26.04.1 as **Supported** (suite `resolute`). `docs/SUPPORT.md` no longer describes CPN 1.0.0 as the current stable line.
- **Plugin Store install target**: Store tab adds **Install target** toggle (**Host** | **Site**). Host hides the site dropdown and explains one-time server installs; Site keeps the domain picker for site plugins and Activate. Host category links default to Host target.
- **Version Management release dates**: `/settings/version` shows GitHub release dates (`dd/mm/yyyy`, 24h when available) for Running, Installed package, and each tag in the searchable picker. Copy states CPN supports only the latest two published releases; older tags show an outside-support hint on apply (lab downgrade still allowed).
- **Sidebar search Docker discovery**: menu search lists Docker Containers/Images when the docker Host package is installed, or Install Docker / Host packages store links when it is not.

- **Docker Host package**: Plugins Store lists free Host package `docker` (search `q=docker`, Host / Utility / Featured). Installs Docker Engine or Podman with a docker-compatible CLI via dnf/apt (operator confirm). Manage UI at clean `/docker` (Active Containers table: name, owner, image, tag, status, start/stop/restart/remove/logs) and `/docker/images`. Sidebar Server > Docker appears when installed. Containers labeled `com.cpn.managed=1` cannot be removed from the UI; `/var/lib/cpn/docker` stacks stay untouched on install/uninstall. Legacy `/server/docker/*` routes still work.
- **Cloudflare site DNS on create/delete**: when Cloudflare OAuth or API token is connected, website create (`/websites/create` and `cpn site create`) upserts an A for the site FQDN (and `www.<fqdn>` CNAME when missing) under the matching zone. Public IPs are orange-cloud proxied; RFC1918/CGNAT lab IPs stay DNS-only because Cloudflare rejects proxied private targets. Site delete removes those CPN-managed A/AAAA/www records only (idempotent; TXT/MX/mail left alone). Zone lookup walks parent labels so subdomains like `test2.example.com` resolve the apex zone. Create/delete UI notices surface DNS success or warnings. Heal existing sites with `cpn site ready --domain <fqdn>`.
- **Account rename / deactivate**: Other accounts (admin) on Modify User gains Rename and Deactivate/Enable beside Reset password and Delete. CLI: `cpn account rename|deactivate|enable|list` with reserved-username rejection, flexible `y`/`yes` confirm, and last-admin safety (`--force` required to deactivate the last active bootstrap admin).
- **Lab/source build disk cleanup**: `scripts/cleanup-old-build-trees.sh` removes abandoned `/home/cpn/cpn-build-*` worktrees (skips the active keep dir, in-use process cwd/cmdline, and the main `CPN-Control-Panel-Network` clone) and stale `/tmp`/`/var/tmp` `cpn-*` extract dirs older than a TTL. `scripts/build-rpm.sh` and `scripts/build-deb.sh` call it before compile and after a successful package build (`--keep-count 0`). Agents should run the same helper when cloning a new `cpn-build-*` tree outside those scripts.

### Changed

- **SnappyMail-family Contacts**: install/heal provisions a dedicated local **MariaDB** database and user per client (`cpn_snappymail_ab`, `cpn_tachyon_ab`, and NextSnapMail when present). Admin UI Storage type remains **MySQL** (PDO) pointed at `127.0.0.1:3306`; credentials are stored only under `/var/lib/cpn/webmail-contacts/` (mode 600). Branding stays on the webmail Admin **Branding** sidebar tab (`/?admin#/branding`); heal still sets title/loading/favicon to CPN Webmail / CPN Panel on every lineage data root. SQLite is used only when MariaDB is unavailable.
- **Email > Change Password** (`/email/password`): mailbox dropdown includes a **Webmail admin** option (label shows live `admin_login` from each installed SnappyMail-family client). Selecting Admin updates bcrypt admin passwords for SnappyMail, Tachyon, and NextSnapMail when present (`/snappymail/?admin`, `/tachyon/?admin`, and NextSnapMail data). Selecting a mailbox only resets that mailbox. Copy is family-wide (not SnappyMail-only). Reloading the page heals lineage prefs and re-reads admin usernames from `application.ini`.
- **SnappyMail-family operator defaults** (Markdown, AllowStyles, Sieve/ManageSieve domain prefs, branding, Contacts, system folders) apply via a shared helper to every installed data root under `/var/lib/cpn-webmail/{snappymail,tachyon,…}` and discovered NextSnapMail data. Tachyon install/heal now receives the same defaults as SnappyMail (not SnappyMail-only). Roundcube is unchanged.

### Fixed

- **MFA enroll pending: profile access**: while administrators must still enroll TOTP or a passkey before Dashboard and other Hosting areas, the Users & Plans nav (`active = "users"`) stays reachable the same way Settings already does. Modify User shows its Account / Security / Other tabs (not the full-page enroll shell); enroll-2fa and Settings remain available. Forced password change still blocks these areas until the password is changed.
- **Password policy blocked-list link**: Change password (Modify User), Create User, and Security hub policy copy now link **blocked-password list** to the live GitHub raw list (`docs/blocked-passwords.txt` on `stable`), opening in a new tab with `rel="noopener noreferrer"`. Dark mode uses readable accent link styling.
- **SnappyMail / Tachyon system folders**: mailbox create / email install / webmail heal now create IMAP **Sent**, **Drafts**, **Junk** (Spam role), **Trash**, and **Archive** (Maildir++ plus `doveadm`), enable Dovecot `auto = subscribe` with SPECIAL-USE (`\Sent`, `\Drafts`, `\Junk`, `\Trash`, `\Archive`), and pre-fill `settings_local` under both `/var/lib/cpn-webmail/snappymail/` and `/var/lib/cpn-webmail/tachyon/` (`JunkFolder` → `Junk`, UI label Spam). Existing empty mappings migrate once so compose/send is not stuck on "Select system folders" with Spam = "Choose one".

### Added

- **Host install + per-site Activate**: panel admin installs Host packages (`/plugins?view=host`) and host-scoped catalog plugins once. Sites and subdomains **Activate** / **Deactivate** the shared host install (no second full copy). Non-admins cannot **Uninstall** host-owned packages (Deactivate only). Domain lists stay jailed via existing site ACL.
- **phpMyAdmin domain jail**: `/databases/phpmyadmin/open?domain=` mints an ephemeral MariaDB user granted only databases registered for that domain. Host open without `domain=` remains admin-only with full grants.
- **Active webmail switching**: Host packages and `cpn app activate --name <client>` switch the active panel webmail among Tachyon, SnappyMail, Roundcube, and NextSnapMail (when installed). Preference is stored in `/var/lib/cpn/active-webmail.json`; panel-proxied clients also update `/opt/cpn-webmail/current`, `webmail-panel.json` public path, and PHP-FPM/proxy. Postfix/Dovecot mailboxes are unchanged.
- **Nextcloud host package + NextSnapMail dependency chain**: `cpn app install --name nextcloud` (or Install Nextcloud first / Install Nextcloud + NextSnapMail on the NextSnapMail card) downloads Nextcloud under `/opt/nextcloud`, then installs the NextSnapMail app into `apps/nextsnapmail`. OCC/web setup remains an operator step for production.
- **Roundcube Email host package**: Roundcube is listed under Plugins > Host packages (Email) alongside SnappyMail and Tachyon. Install path is `/opt/cpn-webmail/roundcube` with panel proxy `/roundcube/` (IMAP `localhost:143`). CLI: `cpn app install --name roundcube` · `cpn app activate --name roundcube`. The Plugin Store `roundcubeWebmail` card no longer installs; it redirects operators to Host packages (CPN host paths only).
- **Uninstall confirmation with impact list**: Installed plugins and Host packages (`/plugins?view=host`) require a Confirm / Cancel dialog that lists services and features that will stop or become unavailable. POST `/plugins/uninstall` and `/apps/uninstall` reject requests without `confirm=1`. Host impacts live in `host_packages_catalog` (`uninstall_impacts`); plugins may declare them in catalog `meta.xml` (`<uninstall_impact>`) or `cpn-plugin.json`, with built-in maps for known ids (fail2ban, mtaSts, bimi, and similar) and a generic fallback otherwise.

### Changed

- **Default CPN webmail is Tachyon** (not SnappyMail) for fresh installer/CLI/Host packages copy. Existing labs keep the current active client when preference or `/opt/cpn-webmail/current` is already set.
- **SnappyMail-lineage admin password sync** also updates Tachyon data under `/var/lib/cpn-webmail/tachyon/` when present.


### Changed

- **Host database policy**: CPN installs **MariaDB only** as the MySQL-compatible host database. Oracle MySQL is no longer a Host packages card, installer option, or `cpn app` target. Legacy `--database mysql` / JSON `mysql` map to MariaDB. PostgreSQL remains an opt-in coexistence package. phpMyAdmin continues to target MariaDB.

### Fixed

- **Backup / restore with MariaDB-only host DB**: selective database backups prefer `mariadb-dump` (fallback `mysqldump`); SQL restore/import uses the MariaDB client and Host packages MariaDB (no Oracle MySQL app id). cPanel-style `mysql/` dump folders still import into MariaDB; UI copy clarifies compatibility paths.
- **SnappyMail Extensions / About repository hang**: upstream `snappymail.eu` package repo is often unreachable (connect timeout). Admin UI then waited until the browser aborted (~30s RequestTimeout / blank Extensions list). CPN now ships a local stub under `/var/lib/cpn-webmail/snappy-repo/v2/` and patches `Repository::get()` to read it first so installed plugins still list and core update checks return quickly. SELinux module `cpn_webmail_imap` **1.2** also allows `httpd_t` → `http_port_t` (HTTPS 443) for when the upstream repo returns. Webmail PHP-FPM sets `default_socket_timeout=8`; panel proxy caps admin Json at 35s.

### Added

- **Host packages store-like UI** (`/plugins?view=host`): card grid with category pills (including Featured), search, and pagination/scrollbar controls matching the Plugin Store. Domain scope stays a header control.
- **Plugin Store metadata**: Released and Updated dates (`dd.mm.yyyy`), Featured badge, and Featured category filter. Featured = explicit `featured` in catalog meta, or `install_count >= 25`, or top 5 by `install_count`.
- **Webmail host packages / installer options**: SnappyMail (LIVE), Tachyon (LIVE, SnappyMail-lineage install under `/opt/cpn-webmail/tachyon`), NextSnapMail (honest Nextcloud gate, not standalone LIVE), SOGo (SCAFFOLD registry/UI). Wired through `cpn app install` and installer mail selection.

### Changed

- **SnappyMail defaults** for all accounts: **Convert HTML to Markdown** and **Allow styles** (`AllowStyles` / `<style>` CSS) default On (user-overridable). Existing accounts migrate once; later toggles are preserved.
- **SnappyMail admin password** (`/?admin`) stays in sync with the password set via **Email > Change Password** (`/email/password`), and also when the CPN panel account password changes (forced change, profile change, reset, or first-account setup). Same operator password for panel mail ops and SnappyMail admin.
- **SnappyMail Login**: **Try to determine user domain** defaults On (short login + multi-domain). Language selection / determine-language stay On.
- **SnappyMail Branding**: page title **CPN Webmail**, loading text **CPN Panel**, favicon `/favicon.ico` (panel logo). Applied on install and heal/upgrade (CPN branding only).
- **SnappyMail Contacts**: enabled by default; prefer dedicated MariaDB AddressBook databases per lineage client (see Unreleased). Older releases used SQLite `AddressBook.sqlite` under webmail data.

### Added

- **Email hub LIVE tools** (former SCAFFOLD cards): Pattern Forwarding (`/email/pattern-forwarding` with Postfix virtual maps), Email Limits (`/email/limits`), Change Password (`/email/password`), Email Debugger (`/email/debugger`), Mail Queue (`/email/queue` via allowlisted `postqueue`/`postsuper`), SpamAssassin / Rspamd / MailScanner status+enable pages, Email Marketing MVP (`/email/marketing`), and Plus-Addressing (`/email/plus-addressing` via `recipient_delimiter`). Admin ACL + CSRF on POSTs; MailScanner may show Unavailable on AlmaLinux 9 when the package is missing.
- **DNS Zones** create/manage UX at `/server/dns/zones` and `/server/dns/zones/create`: domain-only create (strips `http`/`www`), auto-seed SOA + NS from Default Nameservers plus apex A when the host IP is known, structured record table (A, AAAA, CNAME, MX, TXT, NS, SRV) with add/delete, optional Advanced raw zone editor. Zones persist as JSON + `.zone` under the CPN DNS data directory. Admin-only POSTs with CSRF and same-origin checks.
- **Nameservers** (`/server/dns/nameservers`): create/list/delete NS hostnames with glue A/AAAA.
- **Default Nameservers** (`/server/dns/defaults`): choose which NS hostnames are assigned to newly created zones. Server hub tiles and sidebar links stay live for Zones / Nameservers / Default Nameservers.

### Added

- **Site preview** thumbnails on `/websites` (and Manage Overview): cached homepage screenshots under `/var/lib/cpn/site-previews/`, 24h TTL, authenticated image + Refresh preview routes (registry domains only). Headless Chromium or wkhtmltoimage when installed; otherwise an honest placeholder. Minimalist mode skips auto-capture (refresh on demand). Lab hosts map the domain to loopback via Chromium host-resolver rules.
- Website Manage **Domain Alias** (`?tab=alias`): add/list/remove hostnames for a site, persist in the site registry, apply OLS map / Apache ServerAlias / nginx `server_name` when present, and optionally create Cloudflare CNAME or A records when `/var/lib/cpn/cloudflare.json` is configured. CSRF + site ACL on all POSTs.
- Website Manage **Cron Jobs** (`?tab=cron`): per-site schedule editor (list/add/edit/delete) with commands jailed under the site home, synced to `/etc/cron.d/cpn-site-*` (mirror under `/var/lib/cpn/site-crons/`). Domains tab cards and the header Cron Jobs button open the working UI (scaffold copy removed).
- Site Manage **Open Terminal**, **Manage Git**, and **Clone/Staging** (no longer greyed out): web terminal over authenticated WebSocket (`/api/websites/terminal/ws`, xterm.js UI, shell under site home via `script` PTY), Git tab with allowlisted status/pull/push/commit/init/clone (`POST /websites/git`), and staging clone to a subdomain or custom target (`POST /websites/clone`) with site registry update. CSRF, same-origin, site ACL, and rate limits apply.
- Site Manage **Logs** tab: Access/Error cards open a searchable, paginated modal (10/25/50 lines per page, Prev/Next, go-to-page, Refresh) via `/api/websites/manage/logs`. Domain-jailed to each site home `logs/access.log` and `error.log` only (parent never reads child subdomain logs; no aggregate all-logs view). New sites still get a `logs/` directory; opening Logs wires OLS/LSE/nginx vhost log paths when possible. Admin **Log retention** at `/settings/logs` (default **30 days**, optional max size MB) persists `/var/lib/cpn/log-retention.json` and writes `/etc/logrotate.d/cpn-site-logs` when writable.

### Fixed

- **SnappyMail IMAP login on SELinux hosts**: enable `httpd_can_network_connect`, set Dovecot `auth_username_format = %Ln` (email local-part for PAM), prefer `ssl = yes` over `required`, and point SnappyMail domain defaults at `127.0.0.1` with `shortLogin` so webmail can authenticate local Maildir users.
- **PHP-FPM IMAP `name_connect`**: ship local SELinux module `cpn_webmail_imap` so `httpd_t` may connect to Dovecot `pop_port_t` (143/993) and Postfix `smtp_port_t`. On AlmaLinux 9 the network-connect boolean alone still denied dest=143 (SnappyMail: Can't connect to host tcp://localhost:143). Module **1.1** allows `sieve_port_t` (ManageSieve 4190); **1.2** also allows `http_port_t` (HTTPS 443 for the package repository).
- **Webmail panel proxy**: parse curl responses as raw bytes so WOFF/fonts and other binary assets are not UTF-8-corrupted; preserve multiple `Set-Cookie` headers.
- **Change Password PRG**: `/email/password/save` redirects to clean `/email/password` with an HttpOnly flash cookie notice (no long `?notice=` query wall); Open Webmail links use `/snappymail/` instead of forcing `index.php`.

- **Set as host default** now runs `dnf module switch-to` (not enable-only) so Remi php/php-fpm packages actually upgrade or downgrade with the chosen stream. After apply, CPN verifies `php-fpm -v` matches the persisted branch so phpMyAdmin cannot stay on 8.5 while `/var/lib/cpn/php-default.json` claims 8.4.
- PHP Configurations **Set as host default** no longer leaves a blank 404 on `/server/php/configs/set-default`: the form POSTs to `/server/php/configs` with `op=set-default` (PRG back to the configs page), GET on the legacy `/set-default` path 303-redirects, and Actix registers GET+POST on one `web::resource` so the second method is not dropped.
- phpMyAdmin Open auto-login **503** after host PHP default / php-fpm thrash: `ensure_fpm_socket_for_ols` no longer unconditionally restarts php-fpm (reload when healthy; `systemctl reset-failed` + start when in `start-limit-hit`). Panel `/phpmyadmin` proxy heals a missing `cpn-phpmyadmin.sock` and retries once on backend 503 so `cpn-signon.php` reaches the UI again. Open auto-login no longer runs a full OLS listener refresh on every remint (that restart loop hit start-limit when SignonURL pointed at `/databases/phpmyadmin/open`).
- PHP Configurations unsaved-changes modal: use panel card surface (`--canvas`) so dark mode title/body stay readable; Cancel (secondary) and Abandon (danger) match pill button styles instead of unstyled borders.
- phpMyAdmin SSO after **Set as host default** (php-fpm restart): SignonURL now remints via `/databases/phpmyadmin/open` while the CPN panel session is valid; `cpn-signon.php` redirects there instead of plain-text "Sign-on token missing or expired." Proxy Location rewrite no longer prefixes panel routes with `/phpmyadmin`. Open clears stale PMA cookies; host-default apply refreshes the sign-on bridge, TempDir ownership, and OLS FPM socket.

### Changed

- Default panel password policy: minimum length **8** (was 12); special characters optional (uppercase and number still required; max 256; blocked-password list enforced). Installer/first-account copy and Security / Create User / Change password hints follow the policy.
- PHP Configurations (`/server/php/configs`): changing **Select PHP Version** auto-loads that version's settings. Unsaved basic/advanced edits show a modal (Cancel / Abandon changes / Save first). Load button remains as a `<noscript>` fallback.
- Server **Top Processes** (`/server/processes`): card toolbar with Refresh, truncated commands (full path in `title` tooltip), high-CPU highlight, and a stacked card layout under ~720px so CPU/MEM stay readable without horizontal scroll.

### Added

- **Site File Manager** full page at `/websites/files?domain=…` (alias `/filemanager/site`): reuses the classic File Manager UI jailed to the site home (`/home/<domain>` or nested subdomain home). Manage banner and Files tab open that page (not Root FM). Path traversal and sibling sites are blocked.
- **Root File Manager** sidebar leaf under Administration (admins): top-level nav item to `/server/files`, also listed in menu search.
- **Root File Manager** (classic hosting file manager): full toolbar (Upload, New File, New Folder, Delete, Copy, Move, Rename, Edit, Compress, Extract), directory tree, and file table under `/server/files`, with aliases `/filemanager` and `/server/filemanager`. Admin-only; CSRF and same-origin checks on mutations; path traversal blocked; protected system paths refuse overwrite/delete; rate-limited dangerous ops. Starts at `/` for the panel owner (documented risk).
- Dashboard **Activity Board** under Recent Activity: admin-only tabs for Recent SSH Logins, Recent SSH Logs (with light SSH security review and hardening tips), Top Process (snapshot plus link to `/server/processes`), Traffic (`/proc/net/dev` counters), Disk IO (`/proc/diskstats`), and CPU Usage. Log lines are sanitized; mobile tab strip wraps or scrolls. Table tabs include search, default **10** per page, page indicator, Prev/Next, and Go to page (CPU Usage stays KPI-only).
- **Firewall manager** at `/security/firewall` (tabs: `?tab=rules`, `?tab=banned`, `?tab=trusted`): Start/Stop/Reload for firewalld, CPN-managed port rules with import/export, banned IPs (fail closed on trusted addresses), and SSH trusted / never-block IPs. Server IP and first admin login IP are auto-seeded and cannot be banned; panel listen port is kept open. Admin-only POSTs with CSRF + same-origin checks. Persists under `/var/lib/cpn/firewall-manager.json`.
- Password policy hints on Security hub, Create User, and Change password (min length from policy, max 256, uppercase/number required by default, special optional, blocked-password list).

### Notes

- Earlier `/security/firewall` was status-only (live firewalld dump + optional Enable http/https) because rule/ban management was deferred. This release adds the CPN-native manager UI.

### Added

- Server **PHP Configurations** (`/server/php/configs`): Basic Settings and Advanced php.ini editor per PHP version, Save Changes (backup under `/var/lib/cpn/php-ini-backups/`), Restart PHP, and **Set as host default** for system php-fpm / phpMyAdmin. Sidebar entry under Server; cross-linked with PHP Extensions. Admin-only POSTs with CSRF + same-origin checks.
- Installer failure UI **Open GitHub issue** opens `/issues/new` with a prefilled title and body (CPN version, OS, arch, kernel, install mode, failed step, sanitized error). Host IP addresses, MACs, tokens, passwords, and usernames are omitted or redacted. Status now exposes safe `os_pretty_name`, `arch`, and `kernel` on `environment` for that template.
- Server **PHP Extensions** manager (`/server/php/extensions`): select PHP version (default from `/var/lib/cpn/php-default.json`, prefer 8.5), Load Extensions, searchable install/uninstall table. Prefer an already-installed LiteSpeed `lsphpXX` tree; otherwise Remi/AppStream `php-*` (same surface as php-fpm / phpMyAdmin). Admin-only POSTs with CSRF + same-origin checks. **Set as host default** persists `php-default.json` and retargets php-fpm; does not force-install `lsphp` when Remi PHP is present (shared-path conflicts on EL).

### Fixed

- Host PHP default **Set as host default** now writes `/var/lib/cpn/php-default.json` before module apply so the Extensions/Configurations UI does not keep showing "not persisted yet" after a successful save. Module/package apply failures still surface as errors while the preference remains on disk.
- PHP Extensions on narrow viewports: card/stack layout so Install/Uninstall stay visible without horizontal scrolling; toolbar buttons stack instead of forcing an ultra-wide row.
- GET `/account/security/enroll-2fa/begin` (browser refresh after POST) no longer falls through to the installer SPA ("Could not query the installer"). It redirects to `/account/security/enroll-2fa`. Successful begin uses PRG to the same GET page. Extensionless unknown paths no longer serve installer `index.html`.

### Fixed

- Passkey register from **Edit profile** (or View/Edit URLs while the MFA gate overlays them) returns to `/account/users/modify` with a success notice so another passkey can be added. Dedicated `/account/security/enroll-2fa` enroll still unlocks to `/dashboard`. Register finish accepts an allowlisted `next` (same-origin relative `/account/users/...`, `/account/security...`, or `/dashboard` only).

### Changed

- Mandatory admin MFA enroll (`/account/security/enroll-2fa`): show an in-page **Register passkey** path alongside TOTP (same WebAuthn APIs as Modify User). Completing either TOTP or a passkey clears `totp_required` / unlocks the dashboard. Removed the dead "use Modify User after TOTP" hint (that page stays gated until MFA is enrolled).

- Install default PHP is **8.5** on AlmaLinux/RHEL 9+ (Remi). CLI and web installers offer 8.5 / 8.4 / 8.3 / 8.2. Missing packages fall back to the next branch with a clear log line; choice is stored in `/var/lib/cpn/php-default.json` and applied as the default `php_version` on new sites. See `to-do/PHP-INSTALL-DEFAULT-85.md`.

### Added

- Live reserved panel usernames list (`docs/reserved-usernames.txt` on `stable`) fetched with 24h disk cache and bundled fallback; blocked at first-admin setup, `cpn account create`, and rename.
- Live blocked passwords list (`docs/blocked-passwords.txt` on `stable`) with the same cache/fallback pattern; enforced via `password_meets_policy` on install, create, change, reset, and forced first-login change.
- Install first-admin UX: required non-reserved username; empty password auto-generates a strong secret shown once (CLI end / installer Complete screen); `must_change_password` when generated; panel admins require TOTP/passkey before full dashboard (`totp_required`, migration `0007_account_security_flags`).

### Notes

- GitHub raw URLs (override with `CPN_RESERVED_USERNAMES_URL` / `CPN_BLOCKED_PASSWORDS_URL`; offline tests: `*_OFFLINE=1`).
- Generated passwords are never logged; shown once in the installer UI/CLI only.

## [0.2.6-alpha.49] - 25/09/2026

Passkey Type column after `v0.2.6-alpha.48` (Cargo `0.2.6-alpha.49`).

### Added

- **Passkey authenticator Type**: Modify User (and related list surfaces) show a read-only Type beside Label and Created. Registration persists WebAuthn hints (`authenticatorAttachment`, `transports`, AAGUID when attested, backup/`credProps.rk`, and which register path succeeded). Friendly labels include Windows Hello, Security key (USB/NFC/Bluetooth), Browser / synced passkey, Platform authenticator, Security key, and Passkey when metadata is missing. Existing credentials without metadata show Passkey until re-registered (no invented types). Single Register passkey button unchanged.

## [0.2.6-alpha.48] - 24/09/2026

Windows Hello passkey registration on the single Register passkey button after `v0.2.6-alpha.47` (Cargo `0.2.6-alpha.48`).

### Fixed

- **Windows Hello enroll on localhost**: Presence-only create options used `userVerification: discouraged`, so Edge/Chrome Hello failed with `NotAllowedError` after the picker. Register now tries a platform ceremony first (UV required + congruent CredProtect, client sets `authenticatorAttachment=platform`), then silently retries presence-only security-key on `NotSupportedError` / `NotAllowedError`. One Register passkey button remains.

## [0.2.6-alpha.47] - 24/09/2026

Version UI reconnect (#302), single Register passkey (#303), and upgrade "Already up to date" / newest-release selection (#304) after `v0.2.6-alpha.46` (Cargo `0.2.6-alpha.47`).

### Fixed

- **Version Management UI post-upgrade reconnect** (#302): UI package upgrade/repair no longer calls `systemctl stop` on the live MainPID (that left the unit inactive and the browser on ERR_CONNECTION_RESET / "Waiting for panel after restart"). After a successful apply the panel schedules a detached reload (`systemd-run` / nohup), marks maintenance completed first, and the Version page polls the same browser origin (`localhost` vs `127.0.0.1`) until `/login` is back with a clear success or timeout message.
- **Passkey enroll UX** (#303): restore one **Register passkey** button on enroll-2fa and Modify User (no Windows Hello vs security-key split). Registration uses one presence-only ceremony (no CredProtect UV-required, no required resident key, no platform UV-required passkey path that steered Edge into Microsoft Password Manager). Client strips `hints` for a unified OS/browser picker; on `NotSupportedError` it silently retries with a cross-platform attachment hint. Status text is **Waiting for authenticator...**.
- **Upgrade Already up to date / newest tip** (#304): default `cpn-installer --upgrade` (no `--to`) picks the newest publishable GitHub Release (version-ordered; hollow empty-asset tips skipped) and refreshes the Releases cache when the cached tip lags the installed version. When the resolved tip equals installed (or is still older without an explicit `--to`), exit 0 with `Already up to date (X.Y.Z)` instead of a downgrade suggestion. Downgrade wording only when `--to` / `--downgrade` explicitly targets an older release.

## [0.2.6-alpha.46] - 24/09/2026

Passkey CredProtect fix after `v0.2.6-alpha.45` (Cargo `0.2.6-alpha.46`).

### Fixed

- **Passkey registration CredProtect incongruence**: Chrome/Edge rejected security-key create options when `userVerification` was preferred but the CredProtect extension required UV (`NotSupportedError: Requested protection policy is inconsistent or incongruent`). Security-key registration now uses presence-only keys (no UV-required CredProtect). Enroll UI momentarily offered separate Windows Hello and security-key buttons (superseded by `0.2.6-alpha.47`).

## [0.2.6-alpha.45] - 24/09/2026

Passkey registration fix after `v0.2.6-alpha.44` (Cargo `0.2.6-alpha.45`, #300).

### Fixed

- **Passkey / WebAuthn registration on lab NAT**: register with the security-key ceremony and `userVerification: preferred` so YubiKeys (touch; PIN optional) and Windows Hello work on `http://localhost:<panel_port>` (including host NAT port 2091). Credentials still store as Passkeys.
- **Clearer WebAuthn errors**: stop mapping every Chrome `NotAllowedError` (and W3C URL text) to "cancelled or timed out"; emit distinct friendly messages for Abort, NotAllowed, SecurityError, NotSupported, and InvalidState. Keep RP ID `localhost` plus `localhost`/`127.0.0.1` origin handling and `panel_public_url` support.

## [0.2.6-alpha.44] - 24/09/2026

Full CLI product uninstall after `v0.2.6-alpha.43` (Cargo `0.2.6-alpha.44`, #299).

### Added

- **CLI product uninstall**: `sudo cpn-installer --uninstall` (and `sudo cpn uninstall`) stops/disables `cpn-installer.service`, removes the `cpn-installer` package and leftover `/usr/bin/cpn*` / `/usr/local/bin/cpn*` binaries, and deletes `/var/lib/cpn` plus `/etc/cpn` unless `--keep-data`. Default keeps `/home/<domain>` website files and host MariaDB/OpenLiteSpeed. Optional `--purge-all` (CPN Docker volumes + webmail trees), `--purge-sites` (registered site homes, second confirmation), `--purge-stack` (common MariaDB/OLS packages), and `--dry-run`. Confirmation accepts `yes`/`y` or `no`/`n`; `--yes` skips prompts. Documented in `docs/CLI.md` and `docs/INSTALL.md`.

## [0.2.6-alpha.43] - 22/09/2026

`cpn doctor` and `/usr/local/bin` shadow cleanup after hot-deploy (Cargo `0.2.6-alpha.43`, #298).

### Added

- **`cpn doctor`**: health checks for `/usr/bin/cpn` and `/usr/bin/cpn-installer`, stale `/usr/local/bin` overrides, panel unit, `/login`, install-manifest core paths, and a host/plugin summary. Optional `sudo cpn doctor --heal` removes hot-deploy `/usr/local/bin/cpn*` overrides (then run `hash -r` in open shells).

### Fixed

- **Stale `/usr/local/bin/cpn` after hot-deploy cleanup**: upgrade/repair cleanup now removes `/usr/local/bin/cpn` and `cpn-installer` overrides (and `cpn.bak.*` leftovers) so PATH prefers RPM `/usr/bin`. Post-upgrade verify also checks CLI binaries. Documented in `docs/CLI.md` and `cpn-installer --help`.

## [0.2.6-alpha.42] - 22/09/2026

Upgrade self-kill fix after `v0.2.6-alpha.41`: orphan cleanup no longer `pkill`s the in-flight CLI `--upgrade`, and `upgrade.sh` heals active `/login` as success (Cargo `0.2.6-alpha.42`, #297).

### Fixed

- **Upgrade self-kill / false maintenance failure**: `stop_orphan_panel_listeners` no longer `pkill -x cpn-installer` the running CLI `--upgrade` process (lab symptom: `Terminated` / `Terminert`, unit left inactive, `upgrade.sh` "Panel maintenance was started but exited with an error"). Orphan cleanup now terminates other `cpn-installer` PIDs only. `upgrade.sh` heals the unit after maintenance and treats active `/login` as success when an older binary still exits early. Post-upgrade verify waits for the unit and HTTP before failing.

## [0.2.6-alpha.41] - 22/09/2026

MFA hardening after `v0.2.6-alpha.40`: survive MFA key loss with clear recovery, tolerant TOTP/backup code input, and lab passkey origin fixes (Cargo `0.2.6-alpha.41`).

### Fixed

- **MFA key loss / decrypt recovery**: do not mint a new AES key over existing TOTP ciphertext under `/var/lib/cpn/mfa/`. Decrypt failures show an operator recovery hint instead of silent invalid-code or rate-limit noise.
- **TOTP and backup code input**: accept spaced authenticator codes and backup codes with or without dashes (legacy dashed hashes still verify).
- **Passkey / WebAuthn lab origins**: allow `panel_public_url` origins; auto-redirect `127.0.0.1` to `localhost` before passkey login, register, and 2FA so the browser RP ID matches. Clearer SecurityError copy for labs.


## [0.2.6-alpha.40] - 22/09/2026

TOTP enroll dark-mode and backup-code Copy/Download, MFA enroll return to Modify User, sidebar ACL with owner markdown error messages, Version Management fork update source, Host packages sidebar cleanup, and related CI/fixups since `v0.2.6-alpha.39` (Cargo `0.2.6-alpha.40`).

### Added

- **Version Management fork source**: operators can point upgrades at a GitHub fork via `/var/lib/cpn/update-source.json` (mode 600) and optional token at `/var/lib/cpn/secrets/github-token`. Default remains `Control-Panel-Network/CPN-Control-Panel-Network`. Version check shows configured source tip and upstream official tip when using a fork. Panel APIs: GET/POST `/api/version-source` (POST admin-only). Per-fork release caches live beside the legacy `github-releases-cache.json` for the official repo.
- **Sidebar ACL visibility**: plans and ACL control which left-sidebar items a user can see or open; unauthorized deep links return 403. CPN owner can edit forbidden/error copy from Settings with a markdown editor.
- **cpn.newstargeted.com landing**: product introduction homepage under `site/` while `/install.sh`, `/upgrade.sh`, and `/cpn-bootstrap-lib.sh` keep serving bootstrap scripts. Sync with `scripts/sync-cpn-host-site.sh`; deploy notes in [HOST-SITE.md](HOST-SITE.md).
- **CLI MFA management**: `cpn totp status|disable|clear --username <user>` and `cpn mfa clear --username <user> --yes` clear TOTP and/or passkeys (plus pending WebAuthn ceremonies) from SSH without printing secrets. Passkey clear alone does not remove TOTP; use these when `/login/2fa` Authenticator code should stop after password sign-in.

### Changed

- **CLI destructive confirmation**: interactive prompts for `cpn mfa clear`, `cpn totp clear|disable`, `cpn passkey clear`, and other shared `confirm_delete` callers accept case-insensitive `yes`/`y` (and clear affirmatives like `yeah`/`ok`) or `no`/`n` (plus empty abort). `--yes` still skips the prompt. Previously only exact `YES` was accepted, which aborted lowercase `yes`.
- **Plugins sidebar**: remove the separate Host packages sidebar leaf; Host packages remain under Plugins (`/plugins?view=host`) with Store-matching Installed sections.

### Fixed

- **Upgrade panel restart on labs**: post-upgrade verification stops orphan foreground `cpn-installer` listeners (AddrInUse) before `systemctl restart`, so a leftover `sudo cpn-installer --web` no longer fails `cpn-installer.service is not active after restart` when the package apply already succeeded.
- **TOTP / 2FA enroll dark mode contrast**: form labels use `var(--ink)` instead of hardcoded slate; backup-codes and TOTP setup panels use theme-aware `.mfa-codes-panel` / `.mfa-totp-setup` (readable in light and dark). Applies to `/account/security/enroll-2fa` and Edit profile TOTP enrollment. Backup codes confirm adds **Copy to clipboard** (`navigator.clipboard` with `execCommand` fallback) and **Download** (`cpn-backup-codes.txt` via Blob).
- **MFA enroll return path**: after TOTP or passkey enroll from Modify User / Edit profile, return to `/account/users/modify` so more factors can be added (dedicated `/account/security/enroll-2fa` still unlocks to `/dashboard`).
- **Version Management fetch errors**: browser "Failed to fetch" on version checks and maintenance polls now surfaces actionable network/auth/service messages instead of opaque text.
- **Activity Board / dashboard hang on EL10**: when `firewalld` is installed but inactive, `firewall-cmd` can block forever on D-Bus (`Waiting on dbus connection...`). That blocked the whole `/dashboard` render after Activity Board started calling `firewall_status()`. Probes now check `systemctl is-active firewalld` first and apply a short timeout to host command helpers. Package `%posttrans` also `try-restart`s `cpn-installer` so RPM upgrades do not leave a deleted-inode process serving old UI.


## [0.2.6-alpha.39] - 13/09/2026

phpMyAdmin panel proxy assets, TempDir, and session binding (Cargo `0.2.6-alpha.39`).

### Fixed

- Panel `/phpmyadmin/` proxy no longer UTF-8-decodes response bodies (PNG/theme icons were corrupted to broken images; CSS/layout recovered).
- Create and chown `/var/lib/phpMyAdmin/{temp,upload,save,cache}` for the php-fpm pool user; set `$cfg['TempDir']` so the TempDir warning clears.
- Refresh TempDir ownership on every `/phpmyadmin` panel proxy request so a prior listener boot cannot leave dirs missing.
- Align phpMyAdmin cookie `Max-Age` and `$cfg['LoginCookieValidity']` with the CPN panel session TTL (12 hours).

### Security

- `/phpmyadmin/` continues to require a live CPN panel session.
- CPN `/logout` clears phpMyAdmin cookies under `/phpmyadmin` so PMA cannot stay open after panel logout.


## [0.2.6-alpha.38] - 13/09/2026

Auto SSL and SPF/DKIM/DMARC on domain create, DKIM store auto-create, mail client defaults, and CPN mail onboarding (Cargo `0.2.6-alpha.38`).

### Added

- On website create: ensure `/var/lib/cpn/dkim`, generate per-domain DKIM keys, upsert SPF/DKIM/DMARC (and MX/A when IP known) into local DNS zones, push to Cloudflare when configured (add-only).
- Auto-issue Let's Encrypt (or the install default SSL provider) after create; install `certbot` via dnf/apt when missing; fall back to SAN/HTTP-01 when Cloudflare DNS-01 is unavailable.
- `cpn site ready --domain` re-runs domain readiness for existing sites.
- Setup Wizard onboarding: hostname/rDNS notes, local vs external mail, skip rDNS checkbox (CPN branding only).
- Email Accounts: default Mail Client Configuration (POP3/IMAP/SMTP/Sieve) plus mailbox password for local Postfix/Dovecot provisioning.
- Migration `0006_mail_onboarding_ssl_defaults` persists Let's Encrypt as the default SSL provider for new sites (after WordPress `0004` and site-messages `0005`).
- Dedicated `/phpmyadmin` and `/phpmyadmin/{path}` panel routes so the mount cannot fall through to the installer SPA (`503 The web interface is not embedded`).
- phpMyAdmin configuration storage: create `phpmyadmin` DB, `pma__*` tables from `create_tables.sql`, and a local `cpn_pma` controluser. Control password is stored only under `/var/lib/cpn/phpmyadmin/control.secret` (mode 600) and wired into `/etc/phpMyAdmin/config.inc.php`.

### Fixed

- Proxy retries once after re-wiring the OLS `:8081` listener when the backend is briefly unreachable.

### Notes

- Cloudflare IP allowlists remain add-only.
- Coordinates with Email MTA-STS/BIMI plugins for additional DNS records.
- Lab Let's Encrypt FAIL without public DNS is expected and does not block create/readiness.
- Open auto-login and OLS listener setup both call the phpMyAdmin storage ensure path.
- CPN branding only; control credentials are never logged or shown in URLs.


## [0.2.6-alpha.37] - 13/09/2026

WordPress installer plus phpMyAdmin Open auto-login via panel reverse-proxy (Cargo `0.2.6-alpha.37`).

### Fixed

- WP-CLI phar runs via `php -d memory_limit=512M` (and `WP_CLI_PHP_ARGS` for system `wp`) so `wp core download` does not die on 128M PHP CLI defaults.
- **Open phpMyAdmin (auto-login)** no longer redirects to guest-only `http://127.0.0.1:8081/` (hangs under VirtualBox NAT). It redirects to same-origin `/phpmyadmin/cpn-signon.php?token=...` on the panel port.
- Sign-on config is written to distro `/etc/phpMyAdmin/config.inc.php` (EL loads that path, not only the share copy), with `PmaAbsoluteUri=/phpmyadmin/`.
- Make `/etc/phpMyAdmin` and `config.inc.php` readable by php-fpm (`nobody`) so sign-on auth actually loads (previously root-only, so Open fell back to the cookie login form).

### Added

- Editable suspend messages and site-ready placeholder under Settings (migration `0005_site_messages`).
- CPN-native WordPress installer and manager under Hosting (WP-CLI install, plugin preinstall, manage tabs, MariaDB provisioning). Migration `0004_wordpress_sites`.
- Panel reverse-proxy mount `/phpmyadmin/` to loopback OLS/nginx `:8081` (session required), matching the webmail proxy pattern.
- Open control uses `target=_blank` so the panel page stays open.

### Notes

- Host `:8081` NAT forward is optional; panel port alone is enough.
- CPN branding only; secrets are never shown in the Open URL.

### Changed

- **Email -> MTA-STS** and **Email -> BIMI** are gated behind free Plugin Store packages mtaSts and bimi (CPN-Plugins). Sidebar and hub tiles stay hidden until install; direct URLs show an install-from-store message. Policy/DNS behavior is unchanged after unlock.

## [0.2.6-alpha.36] - 13/09/2026

Cloudflare OAuth DNS link, panel API tokens, versioned SQL migrations, and OpenLiteSpeed WebAdmin alignment with the CPN admin account (Cargo `0.2.6-alpha.36`).

### Added

- At first-account setup, when OpenLiteSpeed is installed, WebAdmin htpasswd is set to the CPN admin username and the same password (re-hashed as apr1/bcrypt for OLS). CPN panel PBKDF2 hashes are not reversible and are never copied into htpasswd.
- `/server/openlitespeed`: Reset WebAdmin to CPN admin account (confirm checkbox + password confirmation) for later OLS installs or drifted credentials. Username field prefills the CPN admin. Copy: "Uses your CPN admin account by default."
- OLS install journal notes when alignment happens at first-account setup vs when a Reset is required.
- Panel API Access (`/account/api-access`): issue, list, and revoke opaque `cpn_` tokens (SHA256 hash at rest; scopes read/dns/admin). Bearer tokens authenticate panel routes alongside session cookies.
- Cloudflare OAuth on API Settings: PKCE connect via `dash.cloudflare.com/oauth2/*`, manual API token fallback, scopes `zone.read dns.write offline_access`. OAuth is DNS link only; CPN account login stays primary.
- Versioned migrations `0001` to `0003` under `sql/` with ledger `schema_migrations.json`; JSON store hooks plus optional `sqlite3 panel.db` apply. Migrations run at panel startup and after upgrade/repair.

### Notes

- Passwords are never logged or shown after save. Guests and manual Set WebAdmin password remain available.
- Lab smoke: after Reset or fresh install+account, `https://127.0.0.1:7080/login.php` accepts the CPN admin credentials (NAT forward `:7080` if needed).
- Register a Cloudflare OAuth app redirect URI matching your panel public URL (for example `http://127.0.0.1:2090/dns/cloudflare/oauth/callback` in NAT labs). See `to-do/CLOUDFLARE-OAUTH.md`.
- Set panel public URL (`cpn network set-public-url`) when OAuth callbacks must use a host NAT port instead of guest loopback.
- Tip `0.2.6-alpha.35` is the BOM/RPM packaging fix only; this tip ships CF OAuth + API Access after that merge.

## [0.2.6-alpha.35] - 12/09/2026

Fix RPM packaging failure: strip UTF-8 BOM from the installer spec so EL9/EL10 builds succeed (Cargo `0.2.6-alpha.35`).

### Fixed

- `packaging/cpn-installer.spec` no longer starts with a UTF-8 BOM (rpmbuild reported `Unknown tag: Name` on alpha.33/alpha.34).
- Restored correct UTF-8 Spanish changelog/description text that had been double-encoded on Windows.
- `scripts/sync-version.sh` and `scripts/build-rpm.sh` strip a leading BOM before writing or rpmbuild; `scripts/check-version-sync.sh` fails CI if a BOM is present.

### Notes

- `v0.2.6-alpha.34` remains tagged without EL9/EL10 RPM assets (retag avoided). Use alpha.35 RPM assets after this release, or DEB/Windows from tips where those jobs succeeded.

## [0.2.6-alpha.34] - 12/09/2026

Finish Appsâ†’Plugins UI unification: one Plugins system in sidebar, website manage, and redirects (Cargo `0.2.6-alpha.34`).

### Fixed

- Website manage Plugins tab no longer splits **Site Apps** vs **Plugins**; one Plugins tile (plus Backups).
- Sidebar search and dashboard tools no longer advertise a standalone Apps destination.
- `/apps` issues a **301** to `/plugins?view=host`; with `domain=` it lands on site Plugins (`/plugins?domain=...`).
- Scaffold `PanelShell` nav matches Rust: Plugins with Host packages child, no Apps leaf.

### Notes

- `cpn app` CLI remains an alias; Host packages UI copy stays under Plugins.

## [0.2.6-alpha.33] - 12/09/2026

Clean2 lab epic: Plugin Store without a site, Apps folded into Plugins, feature gates, firewall enable, OLS WebAdmin users, phpMyAdmin auto-login, malware paid/free paths (Cargo `0.2.6-alpha.33`).

### Fixed

- Plugin Store (`/plugins?view=store` and legacy `?view-store`) loads the CPN-Plugins catalog even when no website exists yet; install still requires a domain.
- `upgrade.sh` / `preUpgrade.sh` retry with `rpm --oldpackage` / apt `--allow-downgrades` when an explicit `-b` / `CPN_RELEASE_TAG` pin targets an older package than installed.

### Added

- Plugins **Host packages** tab (former `/apps`); `/apps` redirects to `/plugins?view=host`; `cpn app` CLI unchanged.
- Sidebar/hub gates for Fail2ban, Firewall tooling, Malware scan, and removal of standalone Apps link until/unless package-backed.
- Firewall page: enable firewalld + http/https on AlmaLinux; writes CPN firewall journal.
- Open OLS: set WebAdmin password, add/remove guests via htpasswd (passwords never echoed after save).
- phpMyAdmin: OpenLiteSpeed loopback `:8081` wiring and panel auto-login sign-on token.
- Malware: free ClamAV status; paid News Targeted API probe from `/var/lib/cpn/malware.json` (`api_token`, optional `api_base`).

### Notes

- Fail2ban remains a CPN-Plugins catalog entry (`fail2ban`); sidebar appears after the host package/plugin is installed.
- Cloudflare DNS still needs `/var/lib/cpn/cloudflare.json` token on the lab before Sync works. IP allowlists stay add-only.

## [0.2.6-alpha.32] - 12/09/2026

Restore email-based panel password recovery on the web UI (Cargo `0.2.6-alpha.32`).

### Fixed

- `/forgot-password` again shows a username/email form that posts to the reset API and emails a one-time `/reset-password?token=...` link (SMTP or local Postfix when available).
- Re-registers `forgot_password_submit`, `reset_password_page`, and `reset_password_submit` routes removed by the installer/login UX change.
- Keeps `sudo cpn password` as an operator SSH/console fallback on the same page, not the only recovery path.
- Installer recovery-email hint copy again describes the forgotten-password email flow.

### Notes

- Ack responses stay non-enumerating: matching accounts get mail when delivery works; the page does not confirm whether an account exists.
- Lab without public DNS should use a reachable panel base (for example forwarded `http://127.0.0.1:2090`) so reset links open.

## [0.2.6-alpha.31] - 12/09/2026

GitHub Releases rate-limit resilience: authenticated API when configured, last-good cache, and direct CDN tip fallback when the API returns 403/429 with an empty cache (Cargo `0.2.6-alpha.31`).

### Fixed

- Version Management / `--version-check` no longer leave Latest empty under unauthenticated API 403 when tip packages are still downloadable from GitHub Releases CDN URLs.
- Running / Installed stay populated from local RPM/binary; UI softens the status line when a tip was resolved via cache or direct download.
- `install.sh` / `upgrade.sh` send `CPN_GITHUB_TOKEN` / `GITHUB_TOKEN` / `/var/lib/cpn/secrets/github-token` on API calls, and fall back to direct `SHA256SUMS` tip resolution when the list API fails.

### Notes

- Never commit GitHub tokens. Lab smoke: `sudo install -m 600` the token file under `/var/lib/cpn/secrets/`.
- Host scripts: `https://cpn.newstargeted.com/install.sh` / `upgrade.sh` with `-b` / `--ref` pin; see `docs/INSTALL.md`.

## [0.2.6-alpha.30] - 12/09/2026

OpenLiteSpeed / LiteSpeed Enterprise WebAdmin entry points in the CPN panel sidebar, plus LiteSpeed plan tier and package upgrade/downgrade management (Cargo `0.2.6-alpha.30`).

### Added

- Sidebar **Server** links: **Open OLS** (when OpenLiteSpeed is installed) and **Open OLSE** (when LiteSpeed Enterprise is installed), gated like phpMyAdmin/webmail.
- Server hub tiles and Dashboard Software tools for the same Open OLS / Open OLSE / LiteSpeed plans routes.
- `/server/openlitespeed` and `/server/litespeed-enterprise` pages with **Open WebAdmin** (default `https://127.0.0.1:7080`, or admin_config / panel override). Notes TLS self-signed lab certs.
- `/server/litespeed` LiteSpeed plans & versions: owned LSWS tiers (Web Host Lite / Essential / Professional / Enterprise / Elite), deep links to LiteSpeed store owned + support catalogs, serial apply to `serial.no`, WebAdmin URL override, OLS package upgrade/downgrade. Secrets in `/var/lib/cpn/litespeed.json` (mode 600); purchase stays on the LiteSpeed store.

### Notes

- Does not scrape LiteSpeed store credentials. Plan purchase happens on store.litespeedtech.com; CPN applies serial + package ops.
- Host scripts: `https://cpn.newstargeted.com/install.sh` / `upgrade.sh` with `-b` / `--ref` pin; see `docs/INSTALL.md`.

## [0.2.6-alpha.29] - 12/09/2026

Ship merged SnappyMail panel proxy fixes (#168) and quiet optional httpd/caddy upgrade probes (#169) as a tip RPM (Cargo `0.2.6-alpha.29`). Published `v0.2.6-alpha.28` was cut before those merges, so labs that upgraded to `.28` still got empty `404` on `/snappymail/` (catch-all method AND bug) and HTML MIME for `/snappymail/v/*/static/**`.

### Fixed

- Tip package now includes the webmail catch-all `guard::Any` OR methods, `/snappymail` + `/snappymail/` + `/snappymail/index.php` proxy to login HTML, and `/snappymail/v/{ver}/static/**` passthrough with real JS/CSS Content-Type.
- Missing `httpd`/`caddy` units on OpenLiteSpeed hosts are skipped quietly during upgrade verify (#169).

### Notes

- Host scripts: `https://cpn.newstargeted.com/install.sh` / `upgrade.sh` with `-b` / `--ref` pin; see `docs/INSTALL.md`.

## [0.2.6-alpha.28] - 12/09/2026

Combined tip: SnappyMail blank-page proxy fix + preferred mailbox picker, plus GitHub Releases disk cache packages from #164 (Cargo `0.2.6-alpha.28`). Published `v0.2.6-alpha.27` still pointed at the #166 packaging commit, so RPMs lacked `github-releases-cache`.

### Fixed

- Panel catch-all used chained `.method()` guards that AND together, so `/snappymail/*` never reached the PHP-FPM proxy (empty 404 / blank SnappyMail). Methods are now OR'd via `guard::Any`.
- Nginx webmail config accepts `index.php/` PATH_INFO for the SnappyMail SPA, and heals loopback docroot when SnappyMail is preferred but Roundcube was still rooted on `:8080`.
- Open SnappyMail / Roundcube lands on the login UI (with optional Email / `_user` prefill), not `#/mailbox/INBOX` before authentication.
- Panel mount strip collided with SnappyMail's own `/snappymail/v/{ver}/static/**` asset URLs: `/snappymail/v/...` became `/v/...`, nginx fell back to `index.php` (HTML), and the browser refused JS/CSS MIME types. Proxy now keeps the `/snappymail/v/...` backend path.

### Changed

- Email > Webmail preferred open account is a searchable mailbox picker from Email Accounts (clearable). Prefill only; true SSO still needs a future secure secret store.

### Notes

- Includes Releases cache / searchable Version Management picker already on `stable` at `47f2f6f`.
- Host scripts: `https://cpn.newstargeted.com/install.sh` / `upgrade.sh` with `-b` / `--ref` pin; see `docs/INSTALL.md`.

## [0.2.6-alpha.27] - 12/09/2026

Guest-matched EL RPM selection, Version Management searchable picker, and GitHub Releases disk cache (Cargo `0.2.6-alpha.27`). Retags the EL fix that landed on `stable` after `v0.2.6-alpha.26` was already published from the pre-fix tip.

### Added

- Disk cache at `/var/lib/cpn/github-releases-cache.json` (TTL default **30 minutes**, `CPN_RELEASES_CACHE_TTL_SECS`). Manual "Check for updates" min interval **60 seconds** (`CPN_RELEASES_CHECK_MIN_INTERVAL_SECS`).
- On GitHub HTTP 403/429 (or network failure): serve last good cache when present; English note such as "Checked recently; showing cached results" / try again in N seconds. No tight retry loop.
- Optional higher API limits via `CPN_GITHUB_TOKEN` / `GITHUB_TOKEN` or `/var/lib/cpn/secrets/github-token` (never committed). ETag / If-None-Match supported.
- Version Management searchable release typeahead; MariaDB/OLS/PHP refresh on upgrade when already installed (no database drops).

### Fixed

- Maintenance / Version Management installs the guest-matched `.elN` RPM via `compatible_package_asset`, not the first GitHub `.rpm` (often `.el10` before `.el9`). Fixes AlmaLinux 9 labs that failed with `libc.so.6(GLIBC_2.39)` / generic `Package install failed (dnf/rpm)`.
- `install_rpm` surfaces the real dnf/rpm stderr in the UI.
- Version Management progress label shows a numeric percent (example: `60% installing: ...`).
- Installed package prefers live `rpm -q` (mapped to Cargo prerelease) when the install-manifest is stale (example: manifest `0.2.2-alpha.17` vs RPM `0.2.6-alpha.24`).
- Phantom Installed package `1.0.0`/`1.0.1` when RPM is already `0.2.x` (manifest reconcile + RPM NVR to Cargo prerelease mapping).

### Notes

- Cache applies to Version Management, `--version-check`, and upgrade release discovery (`list_releases` / `find_release`).
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

- Cool CPN-branded interactive SSH MOTD (`/etc/profile.d/cpn-motd.sh`): ASCII banner, "This server has installed CPN", live login URL(s), start hints, load/CPU/RAM/disk/uptime. English only; CPN branding only; no passwords.
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
- CPN-branded SSH login MOTD (`/etc/profile.d/cpn-motd.sh`): panel version, login URL(s), start hints, and host resource stats (load, CPU load-based, RAM, disk `/`, uptime). Installed after a successful web or CLI install, and self-healed when starting `cpn-installer --web` as root. English only; CPN branding only; no passwords.
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

### Install and packaging (0.2.x Ã¢â€ â€™ 0.2.3-alpha.19)

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

Pre-1.0 development line (`v0.2.2-alpha.1` Ã¢â‚¬Â¦ `v0.2.2-alpha.18`). Notable themes:

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

