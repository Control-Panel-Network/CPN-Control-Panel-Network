# Verify 2a evidence (Phase 2a full panel audit trail)

Recorded: **08/11/2026** (engineering). Plan todo: `verify-2a-audit-trail`.

**Verdict this pass:** `verify-2a-audit-trail` is **PASS** and can be marked completed. Commit `64f98992` maps the real database, DNS, updates, notifications, and UI preference mutation routes to stable actions and categories while retaining deny-by-default coverage. AlmaLinux 10 prerelease (SSH `2229`) was upgraded to `stable @ 64f98992` / CPN `1.4.9`, then the close-out and targeted opt-out smokes passed.

Real database, DNS, update-check, notification, and UI-preference routes now produce `database`, `dns`, `updates`, `notifications`, and `ui` rows. With `disable_ui` selected, a successful no-op POST to `/api/panel/color-mode` produced zero audit rows, while security-essential audit settings mutations remained recorded. The optional webhook passed and was restored to disabled after the smoke.

Coverage matrix: [audit-coverage.md](audit-coverage.md).

## Unit / integration (in-tree)

Source branch / tip used for unit evidence:

| Field | Value |
|-------|-------|
| PR | [#654](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/654) plus direct `stable` category fix |
| Branch | `stable` |
| Commit | `64f98992` |
| Host | WSL (OpenSSL available); Windows host `cargo test` blocked on missing OpenSSL / `openssl-sys` |

### Command

```bash
cd /path/to/feat/phase2a-full-panel-audit
cargo test panel_audit --lib -- --nocapture
```

### Result (08/11/2026)

```
running 13 tests
test panel_audit_types::tests::essential_categories_locked ... ok
test panel_audit_policy::tests::default_retention_is_ninety ... ok
test panel_audit_paths::tests::category_inference ... ok
test panel_audit_paths::tests::maps_plugin_and_auth_paths ... ok
test panel_audit_paths::tests::maps_phase_2a_real_mutation_routes ... ok
test panel_audit_paths::tests::phase_2a_prefix_fallbacks_preserve_categories ... ok
test panel_audit_types::tests::legacy_json_deserializes ... ok
test panel_audit::tests::retention_drops_old_rows ... ok
test panel_audit::tests::ui_opt_out_skips_non_essential ... ok
test panel_audit_export::tests::export_includes_header_and_row ... ok
test panel_audit_alerts::tests::auth_fail_spike_resets_window ... ok
test panel_audit_policy::tests::cannot_disable_auth ... ok
test panel_audit::tests::record_round_trips_and_redacts ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 1211 filtered out
```

Also run (legacy facade still covered on the same tip):

```bash
cargo test panel_action_log --lib -- --nocapture
```

**Result:** `ok. 5 passed; 0 failed` (`percent_decode_handles_truncated_escapes`, `maps_plugin_and_host_paths`, `decodes_location_notice_and_domain`, `error_wins_and_flags_failure`, `record_round_trips_newest_first_and_redacts`).

### What the unit suite already proves

| Gate from audit-coverage checklist | Covered by unit test(s) |
|------------------------------------|-------------------------|
| Round-trip audit row + secret redaction | `panel_audit::tests::record_round_trips_and_redacts` |
| Non-essential `ui` opt-out skips | `panel_audit::tests::ui_opt_out_skips_non_essential` |
| Retention drops old rows (policy days) | `panel_audit::tests::retention_drops_old_rows` |
| Default retention 90 days | `panel_audit_policy::tests::default_retention_is_ninety` |
| Security-essential `auth` cannot be disabled | `panel_audit_policy::tests::cannot_disable_auth` + `essential_categories_locked` |
| Path map / denylist samples | `panel_audit_paths::tests::*` |
| Export JSONL/CSV shape (no secrets in fixture) | `panel_audit_export::tests::export_includes_header_and_row` |
| Auth-fail spike window helper | `panel_audit_alerts::tests::auth_fail_spike_resets_window` |

## Lab HTTP / CLI smoke (08/11/2026, AL10 2229)

### Lab and deploy

| Field | Value |
|-------|-------|
| VM | `CPN-AlmaLinux-10-prerelease` |
| SSH | `127.0.0.1:2229` (helper: `to-do/_lab_ssh_once.py`) |
| Panel | `http://127.0.0.1:2087` (`panel_public_url` same) |
| Pre-upgrade | `cpn 1.4.7` (no Phase 2a markers; no `/var/lib/cpn/audit/`; legacy `panel-actions.jsonl` only) |
| Upgrade | `sudo cpn-installer --upgrade --to stable` (detached; log `/var/log/cpn/verify2a-upgrade.log`) |
| Post-upgrade | `cpn 1.4.9`; `CPN_BUILD_SHA=64f9899294364f9c95ce344d0afe06992635c200` (category fix on `stable`) |
| Ubuntu 2230 | Reachable; still `cpn 1.4.7` (not upgraded this pass; AL10 preferred) |

Session cookie after tip upgrade is port-scoped: `cpn_panel_session_2087` (not bare `cpn_panel_session`).

### Actions exercised

| # | Category | Action | HTTP | Audit row |
|---|----------|--------|------|-----------|
| 1 | Auth | Wrong-password POST `/login` | 401 | `action=auth.login.fail` `category=auth` `ok=false` `ip=10.0.2.2` |
| 1 | Auth | Correct POST `/login` | 200 `/dashboard` + session | `action=auth.login` `category=auth` `ok=true` |
| 1 | Auth | POST `/logout` | 303 `/login` | `action=auth.logout` `category=auth` `actor=cpnowner` `ok=true` |
| 15 | Settings | GET+POST `/settings/audit` (retention + empty webhook) | 200 | `action=settings.audit` `category=settings` `message=Audit policy saved` |
| 9 | Firewall | GET `/security/firewall` + POST `/security/firewall/reload` | 200 | `action=firewall.reload` `category=firewall` (reload failed lab zone parse; **row still written**) |
| 2 | Identity | POST `/account/users/password` with invalid empty input | 303 | `identity.password.change`, `category=identity`, expected failure audited |
| 3 | Accounts | POST `/account/users/create` with invalid empty input | 303 | `account.create`, `category=accounts`, expected failure audited |
| 4 | API tokens | POST `/account/api-access/revoke` with empty id | 303 | `token.revoke`, `category=api_tokens`, expected failure audited |
| 5 | Packages | POST `/plugins/disable-cpn` with empty id | 303 | `plugin.deactivate-cpn`, `category=packages`, expected failure audited |
| 6 | Hosting packages | POST `/packages/create` with invalid empty input | 303 | `hosting_package.create`, `category=hosting_packages`, expected failure audited |
| 7 | Websites | POST `/websites/create` with invalid empty input | 303 | `website.create`, `category=websites`, expected failure audited |
| 8 | Database | POST `/databases/create` with empty name | 303 | `action=db.create`, `category=database`, expected validation failure audited |
| 10 | DNS | POST `/server/dns/zones/create` without CSRF | 303 | `action=dns.zone.create`, `category=dns`, expected CSRF failure audited |
| 11 | Email | POST `/email/accounts/create` with invalid empty input | 303 | synthesized action, correctly inferred `category=email` |
| 12 | Backup | POST `/backups/schedule/save` with empty/default form | 303 | synthesized action, `category=backup`, `ok=true` |
| 13 | Docker | POST `/docker/create` with invalid empty input | 400 | `docker.create`, `category=docker`, expected failure audited |
| 14 | Files | POST `/server/files/op` without CSRF | 303 | synthesized action, `category=files`, expected failure audited |
| 16 | Updates | POST `/api/auto-upgrade/check` with `apply=false` | 200 | `action=updates.auto-upgrade.check`, `category=updates` |
| 18 | Notifications | POST `/api/panel/notifications/mark-read` with invalid empty JSON | 400 | `action=panel.notifications.mark-read`, `category=notifications`, expected validation failure audited |
| 19 | UI prefs | no-op POST `/api/panel/color-mode` with current mode | 200 | `action=ui.color-mode`, `category=ui`; suppressed when `disable_ui=1` |

### Store confirmation

```text
/var/lib/cpn/audit/audit.jsonl   mode 600; ~19 lines after smoke
/var/lib/cpn/panel-actions.jsonl legacy still present (older host/plugin rows)
```

The fixed close-out run added 27 rows. Category counts were `auth=9`, `identity=1`, `accounts=1`, `api_tokens=2`, `packages=1`, `hosting_packages=1`, `websites=1`, `database=1`, `dns=1`, `email=1`, `backup=1`, `docker=1`, `files=1`, `notifications=1`, `ui=1`, `settings=2`, and `cli=1`. There were zero `other` rows. The targeted follow-up added `updates=1` and proved the real UI route was suppressed under `disable_ui`.

Sample redacted shapes (no secrets in messages):

```json
{"ts":1794173700,"actor":"cpnowner","action":"auth.login.fail","ok":false,"message":"login failed","ip":"10.0.2.2","category":"auth","severity":"warning"}
{"ts":1794173701,"actor":"cpnowner","action":"auth.login","ok":true,"message":"login ok","ip":"10.0.2.2","category":"auth","severity":"notice"}
{"ts":1794173716,"actor":"cpnowner","action":"settings.audit","ok":true,"message":"Audit policy saved","ip":"10.0.2.2","category":"settings","severity":"info"}
{"ts":1794173719,"actor":"cpnowner","action":"firewall.reload","ok":false,"message":"firewall-cmd --reload: Error: INVALID_ZONE: ...","ip":"10.0.2.2","category":"firewall","severity":"warning"}
```

### Export smoke

Authenticated GET:

| Format | Status | Content-Type | Secret-smoke (lab passwords / wrong-password string) |
|--------|--------|--------------|------------------------------------------------------|
| JSONL | 200 | `application/x-ndjson; charset=utf-8` | no match |
| CSV | 200 | `text/csv; charset=utf-8` | no match |

### CLI mutation

There is no standalone `cpn audit` subcommand on this tip. A safe, bounded privileged mutation was exercised:

```text
sudo cpn doctor heal --id panel.service --json
exit=0
```

Resulting row:

```json
{"actor":"cli:cpn","action":"cli.doctor.heal","target":"panel.service","ok":true,"message":"heal completed","category":"cli","severity":"notice"}
```

No MFA or password state was changed.

### Webhook delivery

A one-request HTTP listener was bound on guest loopback `127.0.0.1:18765`. The Settings UI accepted that permitted local-SIEM URL, and the listener received a redacted `settings.audit` JSON event with these keys only:

```text
action, actor, category, ip, message, ok, request_id, severity, target, ts, ua
```

Sensitive-key smoke (`password`, `secret`, token value, private key) was clean. The policy was restored afterward to `webhook_url=""`.

### Auth-failure spike notification

The rolling counter file was reset without changing account state, then eight wrong-password logins were sent. `/var/lib/cpn/notifications/cpnowner.json` gained one `Audit: repeated login failures` notification. The eight `auth.login.fail` rows were also present with `category=auth`, `severity=warning`, and no password content.

### Policy lock and UI opt-out

The live `/settings/audit` page returned 200 and showed:

- `Security-essential categories cannot be disabled`
- `Locked on:` category list
- a `disable_ui` checkbox
- no `disable_auth` checkbox

Saving `disable_ui=1` succeeded. A no-op real UI mutation at `/api/panel/color-mode` returned HTTP 200 and added zero audit rows. The same route records `action=ui.color-mode`, `category=ui` when UI auditing is enabled. The audit policy was restored to no disabled categories.

Final policy after all checks:

```json
{"retention_days":90,"webhook_url":"","disabled_categories":[],"alerts_enabled":true}
```

Both `/var/lib/cpn/audit/audit.jsonl` and `policy.json` remained mode `600`.

### Helpers used (repo only)

- `to-do/_lab_ssh_once.py`
- `to-do/_verify2a_upgrade_probe.py` (probe / start-upgrade / upgrade-status)
- `to-do/_verify2a_post_upgrade_smoke.py` (auth / settings / firewall + store inspect)
- `to-do/_verify2a_close_smoke.py` (full safe close-out category smoke)
- `to-do/_verify2a_followup.py` (updates category + real UI opt-out proof)

## Manual lab smoke checklist

Run on a panel that includes Phase 2a. Confirm each category yields a redacted row in `/var/lib/cpn/audit/audit.jsonl` (or Settings > Audit trail) with actor, IP when available, and outcome.

| # | Category | Suggested action | Pass? |
|---|----------|------------------|-------|
| 1 | Auth | Failed login + successful login + logout | [x] |
| 2 | Identity | Password change or TOTP/passkey enroll rename | [x] expected validation failure audited |
| 3 | Accounts / ACL | Create or deactivate a non-owner user | [x] expected validation failure audited |
| 4 | API tokens | Issue then revoke a `cpn_` token | [x] revoke validation failure audited |
| 5 | Packages / plugins / themes | Install or activate a host/plugin action | [x] plugin deactivate validation failure audited |
| 6 | Hosting packages | Edit/assign package limits | [x] create validation failure audited |
| 7 | Websites / WP / apps | Create or suspend a site (lab-only) | [x] create validation failure audited |
| 8 | DB / FTP / SFTP | One mutating DB or FTP action | [x] `db.create`, `category=database` |
| 9 | Firewall / security | Start/stop firewall or Fail2Ban unban | [x] reload exercised (lab zone error; audited) |
| 10 | DNS / Cloudflare | Create or edit a DNS record (lab zone) | [x] `dns.zone.create`, `category=dns` |
| 11 | Email | Create mailbox or change mailbox password | [x] expected validation failure audited |
| 12 | Backup / restore | Trigger backup job if available | [x] schedule mutation audited |
| 13 | Docker / PHP / services | Start/stop a container or service toggle | [x] Docker create validation failure audited |
| 14 | Files | mkdir or upload in root File Manager | [x] file operation CSRF failure audited |
| 15 | Settings / repair / migrate | Change audit retention or heal | [x] audit policy POST |
| 16 | Updates | Version check POST that mutates state, or note denylist if read-only | [x] `updates.auto-upgrade.check`, `category=updates` |
| 17 | CLI | privileged mutation audit | [x] `cpn doctor heal --id panel.service --json` |
| 18 | Notifications | Remote notify if enabled | [x] `panel.notifications.mark-read`, `category=notifications`; local panel notification mutation exercised |
| 19 | UI prefs | Toggle a UI preference (opt-outable) | [x] `ui.color-mode`, `category=ui`; zero rows while `disable_ui=1` |

### Export / policy / alerts (lab)

- [x] Settings > Audit trail: security-essential categories shown locked on; no `disable_auth` control
- [x] Non-essential opt-out skips `ui`: real color-mode POST succeeded and added zero rows
- [x] Export JSONL and CSV download; simple secret-smoke found no lab passwords / wrong-password string
- [x] Optional local webhook received redacted JSON only; policy restored disabled
- [x] Eight-event auth fail spike added one panel-admin inbox notification

## Closure disposition

The practical lab work is complete. `verify-2a-audit-trail` **can be marked completed**. The category mapping defect is fixed, all 19 checklist categories have lab evidence, deny-by-default auditing remains active, and real-route UI opt-out behavior passed.

### Re-run confirmation (this agent, WSL)

Host: WSL `/tmp/cpn-pr654-verify2a` worktree at `57c6494179eef65e24c1128824b691a9289ddea5` (`feat/phase2a-full-panel-audit`). Earlier same-day run also green at `45a93052`.

```bash
export PATH="$HOME/.cargo/bin:$PATH"   # or /home/Master3395/.cargo/bin
cargo test panel_audit --lib -- --nocapture
cargo test panel_action_log --lib -- --nocapture
```

`panel_audit` result:

```
running 11 tests
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 1210 filtered out; finished in 0.01s
```

`panel_action_log` result:

```
running 5 tests
test panel_action_log::tests::decodes_location_notice_and_domain ... ok
test panel_action_log::tests::error_wins_and_flags_failure ... ok
test panel_action_log::tests::percent_decode_handles_truncated_escapes ... ok
test panel_action_log::tests::maps_plugin_and_host_paths ... ok
test panel_action_log::tests::record_round_trips_newest_first_and_redacts ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 1216 filtered out; finished in 0.00s
```

**Pass counts:** `panel_audit` **11/11**; `panel_action_log` **5/5** (16 total unit tests green under those filters).

## Update log

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Unit evidence: 11/11 `panel_audit*` and 5/5 `panel_action_log*` tests OK on WSL at `45a93052`; lab smoke deferred; port probe only |
| 08/11/2026 | WSL re-run confirmed: 11/11 `panel_audit*` + 5/5 `panel_action_log*` at `57c64941` (also green at `45a93052`) |
| 08/11/2026 | Rebased onto `stable` after #653; resolved add/add on this file; Clippy collapsible_if / too_many_arguments fixed |
| 08/11/2026 | #654 merged; AL10 2229 tip-upgraded to `5ccfa94` / 1.4.9; lab smoke auth+settings+firewall; audit.jsonl + export secret-smoke documented; remaining checklist kept open |
| 08/11/2026 | Close-out smoke: logout, CLI doctor heal, loopback webhook, auth-fail spike, policy lock, and safe real-route mutations exercised; database/DNS/updates/notifications/UI category defects documented as completion blockers |
| 08/11/2026 | Commit `64f98992` deployed to AL10 2229; fixed category close-out produced zero `other` rows; targeted update and UI opt-out proof passed; verify-2a marked PASS |
