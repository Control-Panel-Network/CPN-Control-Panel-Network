# GitHub issues sweep (04/09/2026)

Repo: Control-Panel-Network/CPN-Control-Panel-Network

Branch: `fix/close-issues-6-9-13-16`

## Closable after this PR

| ID | Fix |
|----|-----|
| #6 | PHP-FPM + Nginx/Caddy/OLS reverse proxy on `:8888`; remove `php -S`; root-owned code; `tests/webmail-permissions.sh` |
| #9 | Postfix + Dovecot provisioned for webmail; IMAP/SMTP listener checks required; Thunderbird stays client-only (`mail_backend_ready=false`) |
| #13 | Preflight, install journal under `/var/lib/cpn`, tracked file backup/rollback, honest failure messages, idempotent config writes |
| #16 | `release.yml` + `publish-checksums.sh` + optional `GPG_*` / `COSIGN_*` + `actions/attest-build-provenance`; see `to-do/RELEASE-SIGNING.md` |
