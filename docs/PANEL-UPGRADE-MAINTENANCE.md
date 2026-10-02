# Panel upgrade maintenance page

When an owner upgrades or repairs CPN (Version Management UI or `cpn-installer --upgrade` / `--repair` / `--downgrade`), CPN writes a shared flag so visitors see a dark branded maintenance card instead of blank 408 / connection reset pages.

## Flag file

| Path | Purpose |
|------|---------|
| `/var/lib/cpn/maintenance.json` | Active flag (mode `600`) |
| `/var/lib/cpn/maintenance-page.html` | Static HTML mirror (mode `644`) for reverse-proxy fallback |

Override the data root with `CPN_DATA_DIR` (labs/tests).

### JSON fields

- `active`, `phase`, `progress` (0-100), `message`, `title`, `target`
- `source`: `ui` or `cli`
- `started_at_unix`, `updated_at_unix`, `expires_at_unix` (default TTL 45 minutes; soft-extends while progress updates)
- `bypass_token`: owner/staff bypass cookie/query value

## Lifecycle

1. **Begin** before package download/replace (`panel_maintenance_mode::begin`)
2. **Update** on every installer progress tick
3. **UI + systemd**: `mark_restarting` then detached panel reload; new process calls `heal_on_startup` and clears `restarting` / `completed` / expired flags
4. **CLI / non-systemd**: clear on success immediately
5. **Failure**: always clear (no stuck maintenance)
6. **Timeout**: expired flags auto-clear on next read
7. **Heal**: `sudo cpn doctor --heal` clears a stuck flag

## Visitor UX

- Middleware serves the fancy HTML (503 + Retry-After) for normal pages while the flag is active
- Public poll: `GET /api/panel-maintenance` (JSON, no auth)
- Page: `GET /maintenance`
- Auto-retry / Try again until the panel answers again
- **Bypass**: signed-in panel admin session, or `?cpn_maint_bypass=<token>` (sets HttpOnly cookie). Staff link on the page uses the token.

## When the process is fully down

Actix middleware cannot answer while `cpn-installer` is stopped. The static file `maintenance-page.html` is rewritten whenever the flag updates so a reverse proxy can serve it on upstream failure.

### Optional OpenLiteSpeed / LiteSpeed ErrorDocument

If the panel sits behind OLS/LSWS and you want a page during the brief restart window, point a 502/503 ErrorDocument at the static mirror (example; adjust paths to your vhost):

```apache
ErrorDocument 502 /var/lib/cpn/maintenance-page.html
ErrorDocument 503 /var/lib/cpn/maintenance-page.html
```

Direct binds on `:2087` (default lab) still show connection refused until the unit is back; browsers that already loaded `/maintenance` keep auto-retrying.

## Coverage

| Path | Sets flag | Clears |
|------|-----------|--------|
| `/settings/version` upgrade/repair/downgrade (`POST /api/maintenance`) | yes | restart heal / failure / TTL |
| `cpn-installer --upgrade` / `--repair` / `--downgrade` | yes | success (CLI) / failure / TTL |
| `cpn doctor --heal` | no | clears stuck flag |

No CyberPanel strings in the UI page or this doc's product copy.
