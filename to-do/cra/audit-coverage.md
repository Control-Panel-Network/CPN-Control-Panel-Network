# CPN panel audit coverage matrix (Phase 2a)

Living inventory for CRA Annex I (2)(d) / (2)(l). Store: `/var/lib/cpn/audit/audit.jsonl` (compat with legacy `panel-actions.jsonl`).

**Policy:** deny-by-default for authenticated mutating HTTP (POST/PUT/PATCH/DELETE) via `ActionLogGuard`, with an explicit denylist for noisy read-like POSTs. Auth failures and privileged CLI mutations record explicitly.

**Retention:** default **90 days** (`/var/lib/cpn/audit/policy.json`). No silent 1 MiB wipe of the compliance window.

**Opt-out:** non-essential only (`ui`, `themes`, `notifications`, `other`). Security-essential categories stay on.

**Export / alerts:** Settings > Audit trail (`/settings/audit`) JSONL/CSV export; optional HTTPS webhook; owner inbox alerts for key security events.

## Status legend

| Status | Meaning |
|--------|---------|
| covered | Stable action key + category; verified by unit test and/or middleware map |
| middleware | Deny-by-default middleware records (mapped or `http.<method>:<path>`) |
| explicit | Handler/CLI calls `record` / `audit_from_request` / `record_cli` |
| denylist | Intentionally not logged (reviewed) |
| remaining | Still needs a dedicated map entry or explicit call in a later pass |

## Denylist (noisy read-like)

Documented in `src/panel_audit_paths.rs::is_noisy_read_like`:

- `/settings/markdown-preview`, health/status polls, dashboard metrics/activity polls
- Store/list `/search` endpoints, site-preview POSTs
- Paths ending in `/preview` or containing `/poll`

## Coverage by category

| Category | Examples | Mechanism | Status |
|----------|----------|-----------|--------|
| Auth | `auth.login`, `auth.login.fail`, `auth.mfa.verify`, `auth.mfa.fail`, `auth.logout` | explicit (`auth_api`) | covered |
| Auth | forgot/reset password POSTs | middleware map + self-record skip for handler path | middleware / remaining explicit message polish |
| Identity | password change, TOTP/passkey enroll/disable/rename | path map + middleware | middleware |
| Accounts / ACL | create/delete/rename/deactivate/enable/modify, reseller | path map + middleware; CLI create/delete explicit | covered (core) / middleware |
| API tokens | `token.issue`, `token.revoke` | path map + middleware (+ alerts on issue) | covered |
| Packages / plugins / themes | install/activate/uninstall/settings; theme APIs | path map + middleware + existing handler records | covered |
| Hosting packages | create/edit/delete/assign | path map + middleware | middleware |
| Websites / WP / apps | create/delete/suspend/resume | path map + middleware | middleware |
| DB / FTP / SFTP | mutations under hub DB/FTP routes | deny-by-default middleware | middleware |
| Firewall / security | enable/start/stop/rules/bans/trusted; Fail2Ban unban | path map + middleware + explicit Fail2Ban | covered |
| DNS / Cloudflare | zones/records/nameservers/connect | path map + middleware | middleware |
| Email | mailbox create/delete/password/DKIM | path map + middleware | middleware |
| Backup / restore | run/restore/schedule | path map + middleware | middleware |
| Docker / PHP / services | container create/start/stop/remove; other POSTs | path map + middleware | middleware |
| Files | upload/delete/mkdir (root FM) | path map + middleware | middleware |
| Settings / repair / migrate | audit policy, log retention, port, messages, heal, migrate | path map + middleware | covered (audit settings) / middleware |
| Updates | upgrade POSTs | path map + middleware | middleware |
| CLI | `cli.mfa.clear`, `cli.password`, `cli.doctor.heal`, `cli.uninstall`, account create/delete | `panel_audit_cli` | covered |
| Notifications | remote broadcast | existing explicit record | covered |
| UI prefs | personal UI toggles | middleware (opt-outable) | middleware |

## Modules (implementation map)

| Module | Role |
|--------|------|
| `panel_audit_types.rs` | Schema: `ts, actor, action, target, ok, message, ip, ua, request_id, category, severity` |
| `panel_audit_paths.rs` | Action map, denylist, labels, category/severity |
| `panel_audit_policy.rs` | Retention + opt-out + webhook prefs |
| `panel_audit.rs` | Append store, retention prune, migrate legacy |
| `panel_audit_export.rs` | JSONL/CSV export + webhook forward |
| `panel_audit_alerts.rs` | Owner alerts + auth-fail spike |
| `panel_audit_http.rs` | Request IP/UA helpers |
| `panel_audit_cli.rs` | CLI actor mirror |
| `panel_action_log.rs` | Legacy facade |
| `panel_action_log_guard.rs` | Deny-by-default middleware |
| `panel_hub_routes_settings_audit.rs` | Settings UI + export routes |

## Verification checklist (verify-2a)

- [ ] `cargo test panel_audit --lib` (and related module tests) green
- [ ] `cargo check` green
- [ ] Lab smoke: one action per category above yields a redacted row with actor/IP/outcome
- [ ] Security-essential categories cannot be disabled in Settings > Audit trail
- [ ] Non-essential opt-out skips `ui` (unit covered)
- [ ] Export JSONL/CSV downloads without secrets
- [ ] Optional webhook receives redacted JSON only
- [ ] Auth fail spike notifies panel admins

## Remaining after this pass

Prioritize next:

1. Dedicated path map entries for every WordPress, site-apps, PHP default, SSL, SFTP, and email feature POST still synthesizing `http.post:...`
2. Explicit auth records for lockout and password-reset outcomes with richer messages
3. Integrity hash chain per rotated day file (optional plan item)
4. CI inventory script: fail if a new authenticated mutating route is neither mapped nor denylisted
5. Lab smoke evidence per category attached to PR / CRA checklist

Do not mark Phase 2a complete until the verification checklist above passes on a lab install.
