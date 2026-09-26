# CPN CLI Reference

This document covers the operator CLI (`cpn`) and the installer runtime (`cpn-installer`).

> [!NOTE]
> Run `cpn --help`, `cpn <group> --help`, or `cpn-installer --help` on the installed version for the authoritative command syntax for that release.

> [!IMPORTANT]
> Help is a flag on the CPN binaries, not a standalone bash command. Typing bare `--help` in the shell prints `command not found`. Use `cpn --help` or `cpn-installer --help` (also `-h`).

## `cpn`

`cpn` is the post-install operator CLI. Read-only commands can run normally; commands that modify accounts, sites, apps, packages, plugins, or network settings require root privileges.

```bash
cpn --help
cpn-installer --help
cpn list
cpn version
```

Top-level groups:

| Group | Purpose |
|---|---|
| `panel` | Live login URL, panel status, MOTD install helper |
| `info` | Alias for `cpn panel status` |
| `account` | Panel/operator account management |
| `site` | Website records |
| `network` | Listen port, hostname, public URL, and port migration |
| `plugin` | Per-site plugin management |
| `app` | Host applications and services |
| `package` | Hosting packages and account assignments |
| `doctor` | Health checks: CLI paths, panel unit, `/login`, core manifest, host/plugin summary |
| `uninstall` | Remove CPN from this host (same engine as `cpn-installer --uninstall`) |

## Repair, upgrade, uninstall, and `cpn doctor`

There is **no** `cpn --repair`. Repair, upgrade, and product uninstall belong to the installer binary (uninstall is also exposed as `cpn uninstall`):

```bash
sudo cpn-installer --upgrade          # tip package (or --to X.Y.Z); preserves accounts/sites
sudo cpn-installer --repair           # re-apply core files from install-manifest / release
sudo cpn-installer --repair --reset-data   # destructive: also wipes bootstrap/accounts/sites/smtp/secrets
sudo cpn-installer --uninstall        # interactive confirm; removes panel + package + /var/lib/cpn
sudo cpn-installer --uninstall --yes  # non-interactive
sudo cpn uninstall --yes              # same as installer --uninstall --yes
sudo cpn-installer --uninstall --yes --dry-run   # print steps only
```

### Product uninstall (leave CPN)

Default uninstall is **destructive for CPN** but **keeps customer website files** and **keeps host MariaDB / OpenLiteSpeed** packages:

| Removed by default | Kept by default |
|---|---|
| `cpn-installer.service` (stop + disable) | `/home/<domain>/public_html` and other site homes |
| RPM/DEB package `cpn-installer` | Host MariaDB / OpenLiteSpeed packages |
| `/usr/bin/cpn`, `/usr/bin/cpn-installer`, leftover `/usr/local/bin/cpn*` | Unlabeled Docker containers |
| `/var/lib/cpn`, `/etc/cpn` | CPN Docker volumes (unless `--purge-all`) |
| CPN-managed compose/containers (`com.cpn.managed=1`) | `/var/lib/cpn-webmail`, `/opt/cpn-webmail` (unless `--purge-all`) |

Optional flags:

- `--yes` / `-y`: skip confirmation (`yes`/`y` and `no`/`n` are accepted interactively).
- `--keep-data`: leave `/var/lib/cpn` and `/etc/cpn`; skip CPN Docker teardown.
- `--purge-all`: also remove CPN Docker volumes and webmail trees under `/var/lib/cpn-webmail` and `/opt/cpn-webmail`.
- `--purge-sites`: delete registered website homes (second confirmation; never silent).
- `--purge-stack`: attempt to remove common MariaDB / OpenLiteSpeed packages.
- `--dry-run`: print steps without changing the host.

Examples:

```bash
sudo cpn-installer --uninstall --yes
sudo cpn-installer --uninstall --yes --purge-all
sudo cpn uninstall --yes --purge-sites   # also wipes site homes after second confirm
```

What **repair** does today:

- Overwrites **core packaged files** listed in `/var/lib/cpn/install-manifest.json` (installer + CLI binaries, unit files when listed).
- Preserves accounts, bootstrap, SMTP, MFA, sites, docker volumes, and `/home` docroots unless `--reset-data`.
- Runs post-apply cleanup and health verify (panel unit, `/login`, CLI binaries).
- Does **not** reinstall every plugin or Host package. Use `cpn plugin` / `cpn app` / Plugins UI for those.

What **upgrade** adds:

- Installs a newer (or retargeted) RPM/package, refreshes CPN-managed host packages via dnf/apt when already installed, optional `--bypass` for CPN-managed Docker only.

### Why `cpn --help` can say `/usr/local/bin/cpn: No such file or directory`

RPM packages install `/usr/bin/cpn`. Lab hot-deploys sometimes copy binaries to `/usr/local/bin/cpn`, which can shadow PATH. If that file is later removed while bash still has it **hashed**, the shell prints exactly:

```text
-bash: /usr/local/bin/cpn: No such file or directory
```

while `cpn-installer --help` may still work from `/usr/bin`.

Quick heal:

```bash
hash -r
sudo cpn doctor --heal          # removes /usr/local/bin/cpn* overrides
type -a cpn                     # should show /usr/bin/cpn (and/or /bin/cpn)
cpn --help
```

If `/usr/bin/cpn` itself is missing:

```bash
sudo dnf reinstall cpn-installer
# or
sudo cpn-installer --repair
```

Recommended "make sure everything runs" flow:

```bash
hash -r
sudo cpn doctor --heal
sudo cpn-installer --upgrade
sudo cpn-installer --repair
cpn doctor
cpn panel status
```

Read-only check anytime: `cpn doctor`. Heal local-bin shadows: `sudo cpn doctor --heal` then `hash -r`.

## Panel URL and status

Login URLs are resolved **live** from the same preference files the panel UI writes (`$CPN_DATA_DIR/listen_port`, `panel_public_url`, `panel_hostname`), with a world-readable mirror under `/etc/cpn/` so non-root SSH sessions can read them. Changing the listen port in Settings updates the next SSH MOTD and `cpn panel url` without reinstalling.

```bash
cpn panel url
cpn panel url --raw
cpn panel status
cpn info
sudo cpn panel install-motd
```

Example (`cpn panel url`):

```text
Login URL     : http://127.0.0.1:2089/login
Local login   : http://127.0.0.1:2087/login
Listen port   : 2087
Public URL    : http://127.0.0.1:2089
Lab tip       : Windows host may use the public URL when NAT forwards the guest port
```

Resolution order for the primary login URL: `panel_public_url` if set, else `https://<panel_hostname>/login` if set, else `http://127.0.0.1:<listen_port>/login`.

## Accounts

```bash
cpn account list

sudo cpn account create \
  --username admin \
  --email admin@example.com \
  --language en \
  --generate

sudo cpn account passwd --username admin --generate
sudo cpn account rename --username siteuser --to newname
sudo cpn account deactivate --username siteuser --yes
sudo cpn account enable --username siteuser --yes
sudo cpn account delete --username siteuser --yes
```

`cpn account list` prints one row per account with `status=active|disabled` and `role=admin|user` (usernames are omitted from stdout to avoid cleartext logging).

Rename rejects reserved / blacklisted usernames using the same live GitHub list (24h cache + bundled fallback) as account create and first-admin setup. Case rules match create (case-insensitive collision checks; reserved match is case-insensitive).

Deactivate blocks sign-in and rejects existing sessions. Re-enable restores login. Deactivating the last active panel admin (bootstrap owner) is blocked unless `--force` is passed (CLI prints a clear warning). Prefer enabling another admin path before forcing. The bootstrap admin cannot be deleted from the Other accounts UI; rename of the bootstrap account is allowed when the new name is not reserved.

Options used by account commands:

- `--username <NAME>` : account username.
- `--to <NAME>` : new username for `account rename`.
- `--email <EMAIL>` : recovery/account email on create.
- `--language <LANG>` : account language; otherwise follows the detected locale.
- `--password-stdin` : read the password from stdin instead of argv.
- `--generate` : generate a password that satisfies the default password policy.
- `--yes` : skip destructive-operation confirmation where supported (`y`/`yes` also accepted at the prompt).
- `--force` : required with `account deactivate` when targeting the last active panel admin.

## Passkeys

Manage WebAuthn passkeys for panel accounts from SSH (no web UI). Output never includes credential secrets; only id, label, and timestamps.

```bash
sudo cpn passkey list --username admin
sudo cpn passkey remove --username admin --id <credential-id>
sudo cpn passkey clear --username admin --yes
```

- `list`: print registered passkeys for the account (`id`, `label`, `created_unix`, `last_used_unix`).
- `remove`: delete one passkey by `--id` from `list`.
- `clear`: delete every passkey for the account; confirm with `yes`/`y` (case-insensitive) or pass `--yes` to skip the prompt. `no`/`n` (or empty) aborts.
- All mutations and list require root (`sudo cpn …`), or `CPN_ALLOW_NONROOT=1` with a lab data dir.

## TOTP and MFA

Clear or inspect authenticator (TOTP) state from SSH. Output never includes secrets or backup codes.

```bash
sudo cpn totp status --username admin
sudo cpn totp disable --username admin --yes
sudo cpn totp clear --username admin --yes
sudo cpn mfa clear --username admin --yes
```

- `totp status`: prints `totp_enabled=` and `pending_enroll=` (boolean flags only).
- `totp disable` / `totp clear`: remove the TOTP secret and pending enrollment without an authenticator code; confirm with `yes`/`y` or pass `--yes` to skip.
- `mfa clear`: clears TOTP, all passkeys, and pending WebAuthn ceremony files; same confirmation rules (`yes`/`y`/`YES`, or `--yes`; `no`/`n` aborts).
- Destructive confirmations are case-insensitive and trim whitespace (shared helper for account/site/plugin clears too).
- After clearing all MFA for an admin with `totp_required`, the next password login may land on `/account/security/enroll-2fa` instead of `/login/2fa`. That is expected until the admin enrolls again.
- Passkey clear alone does **not** remove TOTP; use `totp` or `mfa clear` when the Authenticator code page should stop appearing.

## Sites

```bash
cpn site list

sudo cpn site create \
  --domain example.com \
  --owner admin \
  --ssl-provider letsencrypt

sudo cpn site modify --domain example.com --disable
sudo cpn site modify --domain example.com --enable
sudo cpn site delete --domain example.com --yes
```

`site create` accepts:

- `--domain <DOMAIN>`
- `--owner <ACCOUNT>` : defaults to `admin`.
- `--docroot <PATH>`
- `--engine <ENGINE>`
- `--notes <TEXT>`
- `--ssl-provider <PROVIDER>` : `letsencrypt`, `zerossl`, `cloudflare_ca`, `custom`, or `none`.

`site modify` accepts the same mutable fields plus `--enable` or `--disable`. Do not pass both in the same command.

## Network

```bash
cpn network show

sudo cpn network set-port \
  --port 9443 \
  --old-port-policy redirect_1m

sudo cpn network set-hostname --hostname panel.example.com
sudo cpn network clear-hostname
sudo cpn network clear-migration
```

`network set-port` options:

- `--port <PORT>` : preferred listen port.
- `--old-port-policy <MODE>` : `redirect_1m`, `redirect_3m`, or `deny` when changing from the current port.
- `--from-port <PORT>` : optional previous port override used when recording the migration.

After changing the preferred port, restart the installer/panel process on the new port when instructed.

## Plugins

```bash
cpn plugin list
cpn plugin list --domain example.com

sudo cpn plugin install --domain example.com --id <plugin-id>
sudo cpn plugin enable --domain example.com --id <plugin-id>
sudo cpn plugin disable --domain example.com --id <plugin-id>
sudo cpn plugin remove --domain example.com --id <plugin-id> --yes
sudo cpn plugin migrate --domain example.com
```

## Apps

Supported app IDs currently include MariaDB, PostgreSQL, phpMyAdmin, Email, RabbitMQ, and webmail packages (SnappyMail, Tachyon, and related). CPN does not offer Oracle MySQL as a host package.

```bash
cpn app list
sudo cpn app install --name postgresql
sudo cpn app start --name postgresql
sudo cpn app stop --name postgresql
sudo cpn app reinstall --name postgresql --yes
sudo cpn app uninstall --name postgresql --yes
```

Install, reinstall, and uninstall can optionally target a site:

```bash
sudo cpn app install --name phpmyadmin --domain example.com
sudo cpn app install --name phpmyadmin --subdomain db.example.com
```

Relevant options:

- `--name <APP>` : application ID.
- `--domain <DOMAIN>` : optional site scope.
- `--subdomain <HOST>` : optional subdomain scope.
- `--yes` : skip confirmation for reinstall/uninstall.

## Hosting packages

```bash
cpn package list

sudo cpn package create \
  --name starter \
  --disk-mb 1000 \
  --bandwidth-mb 1000 \
  --domains 20 \
  --emails 1000 \
  --databases 1000 \
  --ftp-accounts 1000 \
  --fqdn-enabled true

sudo cpn package assign --username admin --package starter
cpn package show --username admin
sudo cpn package delete --id <package-id> --yes
```

`package create` defaults to 1000 MB disk, 1000 MB bandwidth, 20 domains, 1000 email accounts, 1000 databases, 1000 FTP accounts, and FQDN enabled unless values are overridden.

`package update` requires the package ID plus the complete replacement values for its configurable limits.

## `cpn-installer`

The installer can run as a temporary **web UI** or as an interactive **SSH/CLI** wizard. It uses a saved choice first, then detects the browser locale and the guest system locale (`LANG`/`LC_*`). Unsupported locales fall back to English.

```bash
sudo cpn-installer                 # TTY: choose Web UI or SSH/CLI; non-TTY: Web UI
sudo cpn-installer --web          # Web UI explicitly
sudo cpn-installer --cli          # SSH/CLI wizard (interactive terminal required)
sudo cpn-installer --port 9443    # Web UI listen port (with --web or after choosing Web UI)
sudo cpn-installer --panel-hostname panel.example.com
```

Options:

| Option | Meaning |
|---|---|
| `--web` / `--ui` | Start the web installer UI |
| `--cli` / `--ssh` | Interactive SSH/CLI installer (no browser) |
| `--port <PORT>` | Web UI listen port; default is `2087` |
| `--panel-hostname <HOST>` | Persist the public panel hostname/subdomain |
| `--old-port-policy <MODE>` | Port-change behavior: `redirect_1m`, `redirect_3m`, or `deny` |
| `--allow-remote` | Bind `0.0.0.0` for the web UI; HTTP without TLS |
| `--listen-all` | Alias for `--allow-remote` |
| `--uninstall` | Remove CPN from this host (confirm unless `--yes`) |
| `--keep-data` | With `--uninstall`: leave `/var/lib/cpn` and `/etc/cpn` |
| `--purge-all` | With `--uninstall`: also remove CPN Docker volumes and webmail trees |
| `--purge-sites` | With `--uninstall`: delete registered website homes (second confirm) |
| `--purge-stack` | With `--uninstall`: remove common MariaDB / OLS packages when present |
| `--dry-run` | With `--uninstall`: print steps only |
| `--yes` / `-y` | Skip confirmation for uninstall / downgrade |
| `-h`, `--help` | Show help |
| `-V`, `--version` | Show version |

Port resolution order is:

1. `--port`
2. `CPN_LISTEN_PORT`
3. saved CPN port preference
4. default `2087`

`CPN_ALLOW_REMOTE=1` is the environment-variable equivalent of `--allow-remote`.

For remote installation with the web UI, SSH forwarding is safer than exposing the temporary installer directly. See the root [README](../README.md) for installation and first-access steps.

The SSH/CLI path covers the main AlmaLinux install decisions (web engine, MariaDB/none, phpMyAdmin, panel port, optional hostname, optional mail, first account). Installation logging is always fully detailed and persists in `/var/lib/cpn/installation.log`, ready to attach to a GitHub issue. Advanced web-only UI options remain available via `--web`.

Installer progress includes explicit command verification results. The selected/detected account language is saved for the panel.

## SSH login MOTD and panel-ready banner

After a successful install (web or `--cli`), CPN installs `/etc/profile.d/cpn-motd.sh` (also under `/usr/lib/cpn/cpn-motd.sh`) and **enables/starts** `cpn-installer.service` so the panel stays up after SSH disconnect and across reboot. Reinstall the script anytime with `sudo cpn panel install-motd`.

Interactive SSH logins show a cool English CPN ASCII banner with:

- "This server has installed CPN", panel version, and service active state
- **Live** login URL(s) via `cpn panel url --motd` on every login (reads current listen port / public URL / hostname; not a static `/etc/motd` baked at install)
- `systemctl` / `cpn-installer --web` / `--cli` start hints, plus `cpn panel url` anytime
- VirtualBox NAT tip when no hostname/public URL is set (example host forward `2089` -> guest listen port => `http://127.0.0.1:2089/login` on the host)
- Host facts: time, load average, CPU (load-based), RAM, disk on `/`, uptime
- Optional last login / recent auth-fail counts when system logs allow

Starting the panel with `sudo cpn-installer --web` (including systemd `ExecStart=... --web`) prints a short English "panel ready" summary (URL, port, version). It never prints account passwords; generated passwords remain path-only via `generated_password_file=...`.

When the installer/start used `--allow-remote` or `CPN_ALLOW_REMOTE=1`, that choice is persisted (`/var/lib/cpn/allow_remote` plus a systemd drop-in). Production default remains `127.0.0.1`. For VirtualBox NAT labs that need host port forwards without SSH `-L`, start or re-enable with `--allow-remote`.

MOTD language is English for now (panel UI language can differ). There is no CyberPanel branding in these banners.
