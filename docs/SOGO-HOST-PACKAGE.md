# SOGo host package

SOGo is a groupware server (webmail, calendars, contacts, CalDAV/CardDAV, ActiveSync
optional). CPN installs it as an **Email host package** next to Tachyon (default active
webmail), SnappyMail, and Roundcube. Owner/admin installs it once on the Host; it then
answers under the panel at `/SOGo/`.

## What install does

| Step | Detail |
|---|---|
| Packages | EL8/EL9: `sogo`, `sogo-tool`, `sope49-gdl1-mysql`, `memcached` from the Inverse nightly repo (`/etc/yum.repos.d/cpn-sogo.repo`, `gpgcheck=0` because Inverse does not sign that repo metadata). Ubuntu/Debian: distro `sogo` + `sope4.9-gdl1-mysql` + `memcached` first; fallback to Inverse suites `jammy` / `noble` / `bookworm` with a `signed-by` keyring at `/usr/share/keyrings/cpn-sogo.gpg`. |
| Database | MariaDB database and user `cpn_sogo` (random password, stored at `/var/lib/cpn/sogo/db.json`, mode 600). SOGo creates its own tables (`sogo_user_profile`, `sogo_folder_info`, `sogo_sessions_folder`, `sogo_alarms_folder`, `sogo_store`, `sogo_acl`, `sogo_cache_folder`, `sogo_admin`). |
| Users | `sogo_users` table in `cpn_sogo`, rebuilt from the CPN mailbox registry (`/var/lib/cpn/mail-accounts.json`) on install, on mailbox create/enable/disable, and on Change Password. `c_uid` is the full address, `c_password` is the system user's `/etc/shadow` hash, `userPasswordAlgorithm = crypt`. Dovecot keeps `auth_username_format = %Ln`, so the same mailbox password works in SOGo, IMAP, and SMTP. |
| Config | `/etc/sogo/sogo.conf` (OpenStep plist), owned `root:sogo`, mode 640. Carries a `Managed by CPN` header comment; a pre-existing operator file is copied to `sogo.conf.cpn-backup` once. Start keeps an operator-managed file untouched. |
| Services | `memcached` enabled; `sogod` (RPM) or `sogo` (deb) enabled and restarted; waits for `127.0.0.1:20000` and a `GET /SOGo/` 200/302/303. |
| Active webmail | If no PHP webmail client is active, SOGo becomes the active panel webmail (Email > Webmail Open button goes to `/SOGo/`). Otherwise the previous client stays active; use **Set as active** on the SOGo card to switch. The PHP mount (`/tachyon`, `/snappymail`, `/roundcube`) keeps answering either way. |

## Panel proxy

`src/panel_sogo_proxy.rs` runs first in the panel catch-all whenever SOGo is installed:

- `/SOGo` and `/SOGo/...` forward to `http://127.0.0.1:20000` with the headers SOGo needs behind a proxy (`x-webobjects-server-protocol: HTTP/1.0`, `x-webobjects-remote-host`, `x-webobjects-server-name`, `x-webobjects-server-url`, `x-webobjects-server-port`). All WebDAV methods are forwarded; bodies are capped at 64 MB.
- `/SOGo.woa/WebServerResources/...` and `/SOGo/WebServerResources/...` are served from `/usr/lib64/GNUstep/SOGo/WebServerResources` (RPM) or `/usr/lib/GNUstep/SOGo/WebServerResources` (deb) with real MIME types and a traversal guard.
- `/SOGo/so/ControlPanel/Products/<Product>/Resources/<file>` maps to `<GNUstep>/SOGo/<Product>.SOGo/Resources/<file>`.
- `/.well-known/caldav` and `/.well-known/carddav` 301 to `/SOGo/dav/` (autodiscovery for Apple/Thunderbird/DAVx5).
- `Location` headers that point at the loopback listener are rewritten to panel-relative paths.

Mailbox users are not panel users, so no panel session is required for `/SOGo` (same model as Tachyon/Roundcube).

## Client URLs

| Purpose | URL |
|---|---|
| Webmail | `https://PANEL/SOGo/` |
| CalDAV | `https://PANEL/SOGo/dav/<address>/Calendar/` |
| CardDAV | `https://PANEL/SOGo/dav/<address>/Contacts/` |
| Autodiscovery | `https://PANEL/.well-known/caldav`, `/.well-known/carddav` |

Sign in with the full mailbox address and the mailbox password set under Email > Change Password.

## Services, Start/Stop, Uninstall

- `/server/services` shows a logical **sogo** row (resolves `sogod` or `sogo`) and **memcached**. Missing units deep-link to the Store search `sogo`.
- SOGo card: Start, Stop, Set as active, Uninstall (owner/admin). Start re-syncs users and restarts the unit.
- Uninstall stops the unit, removes the SOGo packages and the CPN repo file, and **keeps** `cpn_sogo` plus `/var/lib/cpn/sogo/` so a reinstall restores calendars and contacts. memcached stays installed.

## OS matrix

| Guest | Status | Notes |
|---|---|---|
| AlmaLinux / Rocky / RHEL 8 | Supported | Inverse nightly `rhel/8` |
| AlmaLinux / Rocky / RHEL 9 | Supported | Inverse nightly `rhel/9` |
| AlmaLinux / Rocky / RHEL 10 | Not available yet | Inverse publishes no `rhel/10` packages. The card stays Not installed with a clear warning; Install fails fast with the reason. Use the override below when a third-party EL10 build exists. |
| Ubuntu 22.04 / 24.04 | Supported | distro or Inverse `jammy` / `noble` |
| Ubuntu 26.04 | Supported | universe ships `sogo 5.12.x` (Inverse has no `resolute` suite yet) |
| Debian 12 | Supported | distro or Inverse `bookworm` |
| Windows | Not available | Linux recipe |

### Repository override

Create `/var/lib/cpn/sogo/repo-override.json` (mode 600) to point CPN at your own SOGo repository:

```json
{ "baseurl": "https://mirror.example/sogo/el10/$basearch/", "gpgkey": "https://mirror.example/sogo/RPM-GPG-KEY" }
```

or for apt hosts:

```json
{ "apt_line": "deb [signed-by=/usr/share/keyrings/cpn-sogo.gpg] https://mirror.example/sogo noble noble", "keyring_url": "https://mirror.example/sogo/key.asc" }
```

`gpgkey` enables `gpgcheck=1`; without it the repo stays unsigned like the Inverse nightly channel.

## Limits and follow-ups

- OpenPGP: SOGo has no OpenPGP plugin; use Tachyon/SnappyMail or Roundcube Enigma when end-user PGP matters (`docs/WEBMAIL-OPENPGP.md`).
- ActiveSync (`sogo-activesync`) is not installed by default.
- Shared/public calendars and resources follow SOGo defaults; edit `/etc/sogo/sogo.conf` only if you accept the consequences: while the `Managed by CPN` header stays, Install/Reinstall regenerates the file (Start only restarts units); remove that header line to take ownership so CPN never rewrites it, or delete the file and run Install again to regenerate.
- Troubleshooting: `journalctl -u sogod -n 50` (or `-u sogo`), `curl -i http://127.0.0.1:20000/SOGo/`, `/var/log/sogo/sogo.log`.
