# GitHub issues sweep (03/09/2026)

Repo: Control-Panel-Network/CPN-Control-Panel-Network

Branch: `fix/issues-security-reliability` (rebased on main after #53/#54).

## Closable after merge

| ID | Status |
|----|--------|
| #1 | Fixed: loopback default; `--allow-remote` for 0.0.0.0; firewall only when remote |
| #2 | Fixed: LiteSpeed repo file (no curl\|bash); SHA-256 for SnappyMail/Roundcube |
| #3 | Already fixed on main (#53 RainLoop removal) |
| #4 | Fixed: EL9 uses `php:8.2` |
| #5 | Fixed: PDO SQLite + sqlite.initial.sql + users table |
| #7 | Fixed: real Vite embed build; Panel next build; cargo audit best-effort |
| #8 | Fixed: POST login; dashboard requires `?preview=1` |
| #10 | Fixed: exclusive `/var/tmp/cpn-dl-*` files |
| #11 | Already fixed via os_support guest detection |
| #14 | Fixed: vendor `lsws`/`lshttpd`; no CPN wrapper unit |
| #15 | Fixed: CPN vhost on :80 |
| #17 | sync-version.sh + RPM discovery (already present) |
| #18 | Fixed: kill_on_drop + command timeouts |
| #19 | Fixed: db.sqlite 0600 + ownership |
| #20 | Already enforced server_ready + 409 |
| #21 | Fixed: open http/https; journal under `/var/lib/cpn` |

## Partial (comment only; leave open)

| ID | Why |
|----|-----|
| #6 | Root-owned code + narrowed writable dirs; still transitional `php -S` |
| #9 | Client vs backend messaging; no Postfix/Dovecot yet |
| #13 | Honest failure message + firewall journal; full rollback later |
| #16 | checksum script + docs; GPG/SBOM need release keys |
