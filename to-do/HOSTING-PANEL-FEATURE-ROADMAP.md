# Hosting panel feature hubs (CPN)

CPN-branded tile hubs shipped in the hosting-panel-hubs work. Screenshots from other panels were UX inspiration only. No third-party product branding.

## Sidebar map

| Section | Items |
|---|---|
| **Hosting** | Dashboard, Websites, Email (hub), Databases & FTP (hub), Backups (hub), Apps, Plugins |
| **Account** | Users & Plans (stub; Packages from #86 can coexist at `/packages` when merged) |
| **Administration** | Server (hub), Security (stub), Settings (Change Port) |

## Shipped this PR

### Live (real backend or honest detection)

| Area | Tiles / routes | Behavior |
|---|---|---|
| Server | Services Status | Allowlisted `systemctl` list/start/stop/restart (admin) |
| Server | Top Processes | `ps` snapshot (Linux) |
| Server | PHP Extensions / Configs / Tuning | Detect PHP CLI, modules, php.ini preview (read-only) |
| Server | Package Manager | dnf list/search read-only |
| Server | Docker Apps / Containers / Images | docker/podman CLI or "Docker not installed" |
| Server | Root File Manager | Browse `/home`, `/var/www`, CPN data dir with traversal guards (admin) |
| Server | DNS Zones / Nameservers / Defaults | Zone + NS JSON under CPN data dir |
| Settings | Change Port | Existing `/api/listen-port` JSON migration UI |
| Databases | All / Create / Delete | MariaDB/MySQL client `SHOW`/`CREATE`/`DROP` with ident sanitization |
| Databases | MariaDB Manager | Existing install/detect UI (MariaDB-first naming) |
| Databases | phpMyAdmin | Links to Apps (companion install path) |
| FTP | FTP Accounts | Pure-FTPd/vsftpd detection |
| Email | Email Accounts / Create / Webmail / Delivery | Existing mailbox APIs + stack status |
| Email | Forwarding / Catch-All / DKIM | JSON/file stores under CPN data dir (Postfix map wiring later) |
| Backups | Create | Existing selective backup chooser (concrete scope paths) |
| Backups | Restore | Live archive listing (extract next) |
| Backups | Schedule / Destinations | Local JSON records (timer/GDrive sync later) |

### Scaffold (honest empty / not configured)

| Area | Tiles |
|---|---|
| Backups | Google Drive, Remote Backups |
| FTP | Create / Delete / Reset FTP Account |
| Email | Pattern Forwarding, Email Limits, Change Password, Email Debugger, Mail Queue, SpamAssassin, Rspamd, MailScanner, Email Marketing, Plus-Addressing |
| Account | Users & Plans stub |
| Administration | Security stub |

## Later (roadmap)

- Packages ACL merge (#86) into Account nav
- Default MariaDB + phpMyAdmin install defaults (#88) without conflicting routes
- Postfix map generation from forwarding/catch-all JSON
- Backup restore extract + systemd schedule runner
- Google Drive / remote transfer backends
- FTP account CRUD against Pure-FTPd/vsftpd
- PHP ini write-with-backup and tuning apply
- Package manager install allowlist
- Full file manager edit/upload
- PowerDNS/BIND live DNS instead of file store only
- Security hub (firewall, fail2ban, WAF)

## Tests

- Path traversal / allowlist: `panel_ops_path`
- Service allowlist actions: `panel_ops_services`
- DNS zone name sanitization + CRUD: `panel_ops_dns`
- DB ident sanitization + system DB refuse: `panel_ops_db`
- Backup destinations / mail forwards roundtrip under `CPN_DATA_DIR`
- `cargo test --lib` green

## Authz

- Session required on all hub routes
- Bootstrap admin required for service control, DNS writes, and root file manager
