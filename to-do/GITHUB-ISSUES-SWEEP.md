# GitHub issues and PRs sweep (03/09/2026)

Repo: Control-Panel-Network/CPN-Control-Panel-Network

## Addressed in this local change set

| ID | Title | Status |
|----|-------|--------|
| #8 | Panel login GET credentials | Fixed: form uses POST to `/api/login`; preview link uses `?preview=1`; `/forgot-password` added. Full session cookies still pending. |
| #11 | Validate AlmaLinux VERSION_ID | Already fixed earlier (accept 9 and 10); left in place. |
| #17 | Duplicate versions Cargo/RPM/tests | Partial: `scripts/sync-version.sh`, `cpn-installer --version`, status `version` field, docker-matrix already discovers RPMs. |
| #19 | Roundcube SQLite permissions | Partial: DSN mode 0600 and chmod on db/config. |
| #20 | Enforce installer transitions | Partial: mail requires `server_ready`; invalid phases return 409. |
| #10 | Predictable /tmp download names | Fixed: ephemeral `/var/tmp/cpn-dl-*` paths. |
| #3 | RainLoop legacy default | Partial: marked legacy in UI and logged as inherited option; still installable. |
| #1 | Bind 0.0.0.0 / permanent firewall | Partial: removed permanent firewalld rule; cleanup on exit. Default still binds 0.0.0.0 for VPS UX; loopback-only needs coordinated release. |

## Deferred (need more time, signing infra, or production SMTP)

| ID | Why deferred |
|----|--------------|
| #2 | Download signature verification / no curl bash (needs trusted checksum catalog) |
| #4 | PHP EOL policy beyond current AL9/AL10 branch handling |
| #5 | Full Roundcube schema/extensions provisioning |
| #6 | Replace `php -S` webmail runtime |
| #7 | Broader CI matrix / dependency audit expansion |
| #9 | Real IMAP/SMTP mail stack checks |
| #13 | Preflight/rollback/idempotency framework |
| #14 | Prefer vendor `lsws` unit |
| #15 | OLS listener beyond :8088 |
| #16 | RPM signing and provenance |
| #18 | Child process cancel/timeouts |
| #21 | Service firewall openings beyond installer port |

## Open Dependabot PRs

Left alone (dependency bumps): #23-#44 except draft #22. Review/merge separately after CI green. Do not force-push.

Draft PR #22 (Cloudflare/domain) is out of scope for this install/account work.
