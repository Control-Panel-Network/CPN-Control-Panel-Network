# Changelog

All notable changes to CPN Control Panel Network are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Version Management fork source**: operators can point upgrades at a GitHub fork via `/var/lib/cpn/update-source.json` (mode 600) and optional token at `/var/lib/cpn/secrets/github-token`. Default remains `Control-Panel-Network/CPN-Control-Panel-Network`. Version check shows configured source tip and upstream official tip when using a fork. Panel APIs: GET/POST `/api/version-source` (POST admin-only). Per-fork release caches live beside the legacy `github-releases-cache.json` for the official repo.

### Fixed

- **Version Management fetch errors**: browser "Failed to fetch" on version checks and maintenance polls now surfaces actionable network/auth/service messages instead of opaque text.

### Added

- **cpn.newstargeted.com landing**: product introduction homepage under `site/` while `/install.sh`, `/upgrade.sh`, and `/cpn-bootstrap-lib.sh` keep serving bootstrap scripts. Sync with `scripts/sync-cpn-host-site.sh`; deploy notes in [HOST-SITE.md](HOST-SITE.md).

- **CLI MFA management**: `cpn totp status|disable|clear --username <user>` and `cpn mfa clear --username <user> --yes` clear TOTP and/or passkeys (plus pending WebAuthn ceremonies) from SSH without printing secrets. Passkey clear alone does not remove TOTP; use these when `/login/2fa` Authenticator code should stop after password sign-in.

### Fixed

- **Activity Board / dashboard hang on EL10**: when `firewalld` is installed but inactive, `firewall-cmd` can block forever on D-Bus (`Waiting on dbus connection...`). That blocked the whole `/dashboard` render after Activity Board started calling `firewall_status()`. Probes now check `systemctl is-active firewalld` first and apply a short timeout to host command helpers. Package `%posttrans` also `try-restart`s `cpn-installer` so RPM upgrades do not leave a deleted-inode process serving old UI.

### Added

- **Lab/source build disk cleanup**: `scripts/cleanup-old-build-trees.sh` removes abandoned `/home/cpn/cpn-build-*` worktrees (skips the active keep dir, in-use process cwd/cmdline, and the main `CPN-Control-Panel-Network` clone) and stale `/tmp`/`/var/tmp` `cpn-*` extract dirs older than a TTL. `scripts/build-rpm.sh` and `scripts/build-deb.sh` call it before compile and after a successful package build (`--keep-count 0`). Agents should run the same helper when cloning a new `cpn-build-*` tree outside those scripts.

### Changed

- **SnappyMail-family Contacts**: install/heal provisions a dedicated local **MariaDB** database and user per client (`cpn_snappymail_ab`, `cpn_tachyon_ab`, and NextSnapMail when present). Admin UI Storage type remains **MySQL** (PDO) pointed at `127.0.0.1:3306`; credentials are stored only under `/var/lib/cpn/webmail-contacts/` (mode 600). Branding stays on the webmail Admin **Branding** sidebar tab (`/?admin#/branding`); heal still sets title/loading/favicon to CPN Webmail / CPN Panel on every lineage data root. SQLite is used only when MariaDB is unavailable.
- **Email > Change Password** (`/email/password`): mailbox dropdown includes a **Webmail admin** option (label shows live `admin_login` from each installed SnappyMail-family client). Selecting Admin updates bcrypt admin passwords for SnappyMail, Tachyon, and NextSnapMail when present (`/snappymail/?admin`, `/tachyon/?admin`, and NextSnapMail data). Selecting a mailbox only resets that mailbox. Copy is family-wide (not SnappyMail-only). Reloading the page heals lineage prefs and re-reads admin usernames from `application.ini`.
- **SnappyMail-family operator defaults** (Markdown, AllowStyles, Sieve/ManageSieve domain prefs, branding, Contacts, system folders) apply via a shared helper to every installed data root under `/var/lib/cpn-webmail/{snappymail,tachyon,…}` and discovered NextSnapMail data. Tachyon install/heal now receives the same defaults as SnappyMail (not SnappyMail-only). Roundcube is unchanged.

### Fixed

- **SnappyMail / Tachyon system folders**: mailbox create / email install / webmail heal now create IMAP **Sent**, **Drafts**, **Junk** (Spam role), **Trash**, and **Archive** (Maildir++ plus `doveadm`), enable Dovecot `auto = subscribe` with SPECIAL-USE (`\Sent`, `\Drafts`, `\Junk`, `\Trash`, `\Archive`), and pre-fill `settings_local` under both `/var/lib/cpn-webmail/snappymail/` and `/var/lib/cpn-webmail/tachyon/` (`JunkFolder` → `Junk`, UI label Spam). Existing empty mappings migrate once so compose/send is not stuck on "Select system folders" with Spam = "Choose one".

### Added

- **Host install + per-site Activate**: panel admin installs Host packages (`/plugins?view=host`) and host-scoped catalog plugins once. Sites and subdomains **Activate** / **Deactivate** the shared host install (no second full copy). Non-admins cannot **Uninstall** host-owned packages (Deactivate only). Domain lists stay jailed via existing site ACL.
- **phpMyAdmin domain jail**: `/databases/phpmyadmin/open?domain=` mints an ephemeral MariaDB user granted only databases registered for that domain. Host open without `domain=` remains admin-only with full grants.
- **Active webmail switching**: Host packages and `cpn app activate --name <client>` switch the active panel webmail among Tachyon, SnappyMail, Roundcube, and NextSnapMail (when installed). Preference is stored in `/var/lib/cpn/active-webmail.json`; panel-proxied clients also update `/opt/cpn-webmail/current`, `webmail-panel.json` public path, and PHP-FPM/proxy. Postfix/Dovecot mailboxes are unchanged.
- **Nextcloud host package + NextSnapMail dependency chain**: `cpn app install --name nextcloud` (or Install Nextcloud first / Install Nextcloud + NextSnapMail on the NextSnapMail card) downloads Nextcloud under `/opt/nextcloud`, then installs the NextSnapMail app into `apps/nextsnapmail`. OCC/web setup remains an operator step for production.
- **Roundcube Email host package**: Roundcube is listed under Plugins > Host packages (Email) alongside SnappyMail and Tachyon. Install path is `/opt/cpn-webmail/roundcube` with panel proxy `/roundcube/` (IMAP `localhost:143`). CLI: `cpn app install --name roundcube` · `cpn app activate --name roundcube`. The Plugin Store `roundcubeWebmail` card no longer installs; it redirects operators to Host packages (no CyberPanel paths).
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
- **SnappyMail Branding**: page title **CPN Webmail**, loading text **CPN Panel**, favicon `/favicon.ico` (panel logo). Applied on install and heal/upgrade (never CyberPanel strings).
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
- Site Manage **Open Terminal**, **Manage Git**, and **Clone/Staging** (no longer greyed out): web terminal over authenticated WebSocket (`/api/websites/terminal/ws`, xterm.js UI, shell under site home via `script` PTY), Git tab with allowlisted status/pull/push/commit/init/clone (`POST /websites/git`), and file clone to a staging subdomain or custom target (`POST /websites/clone`) with site registry update. CSRF, same-origin, site ACL, and rate limits apply. Database clone is not included (files only; note in UI).
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
- No CyberPanel branding; control credentials are never logged or shown in URLs.


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
- No CyberPanel branding; secrets are never shown in the Open URL.

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

