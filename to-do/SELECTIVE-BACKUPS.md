# Selective backups

Panel Backups uses a chooser instead of a single full panel-data button.

## Scope

| Scope | Archive directory |
|---|---|
| Panel config | `/home/cpn-panel/backups/` |
| Website domain | `/home/<domain>/backups/` |
| Subdomain | `/home/<parent>/<sub.fqdn>/backups/` |

Do not present `/var/lib/cpn/backups/` as the primary path. Older archives under the data dir may still be noted as legacy only.

## Contents

- Panel config (bootstrap, accounts, site records, prefs, SMTP when present)
- Website files (docroot)
- Backups folder
- Plugins folder
- Databases (mysqldump when a local DB exists)
- FTP content/users: honest stub (disabled until FTP is modeled)

## UI / code

- HTML: `src/panel_backups.rs`
- Archive logic: `src/backups.rs`
- Routes: `GET /backups`, `POST /backups/run`
