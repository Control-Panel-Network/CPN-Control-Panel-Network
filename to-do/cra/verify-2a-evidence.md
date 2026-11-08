# Verify 2a evidence (Phase 2a full panel audit trail)

Recorded: **08/11/2026** (engineering). Plan todo: `verify-2a-audit-trail`.

**Verdict this pass:** unit/integration evidence from PR **#654** remains green. **Partial lab HTTP smoke completed** on AlmaLinux 10 prerelease (SSH `2229`) after tip-upgrade to `stable @ 5ccfa94` (includes merged #654). High-value categories exercised: **auth** (fail + ok), **settings** (audit policy save), **firewall** (reload). Store confirmed at `/var/lib/cpn/audit/audit.jsonl` (legacy `panel-actions.jsonl` still present; migrated rows lack `category`). Export JSONL/CSV downloaded with no password leak in a simple secret-smoke. **Not closed:** remaining checklist categories, logout, UI opt-out UI proof, webhook, auth-fail spike notify, and dedicated CLI mutation audit.

Coverage matrix: [audit-coverage.md](audit-coverage.md).

## Unit / integration (in-tree)

Source branch / tip used for unit evidence:

| Field | Value |
|-------|-------|
| PR | [#654](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/654) |
| Branch | `feat/phase2a-full-panel-audit` (merged to `stable`) |
| Commit | clippy-fix tip after rebase onto `stable` (prior evidence `57c64941`, `45a93052`) |
| Host | WSL (OpenSSL available); Windows host `cargo test` blocked on missing OpenSSL / `openssl-sys` |

### Command

```bash
cd /path/to/feat/phase2a-full-panel-audit
cargo test panel_audit --lib -- --nocapture
```

### Result (08/11/2026)

```
running 11 tests
test panel_audit_types::tests::essential_categories_locked ... ok
test panel_audit_policy::tests::default_retention_is_ninety ... ok
test panel_audit_paths::tests::category_inference ... ok
test panel_audit_paths::tests::maps_plugin_and_auth_paths ... ok
test panel_audit_types::tests::legacy_json_deserializes ... ok
test panel_audit::tests::retention_drops_old_rows ... ok
test panel_audit::tests::ui_opt_out_skips_non_essential ... ok
test panel_audit_export::tests::export_includes_header_and_row ... ok
test panel_audit_alerts::tests::auth_fail_spike_resets_window ... ok
test panel_audit_policy::tests::cannot_disable_auth ... ok
test panel_audit::tests::record_round_trips_and_redacts ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 1210 filtered out
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
| Post-upgrade | `cpn 1.4.9`; `CPN_BUILD_SHA=5ccfa94caf0da1a3201989a87e9a8995a4eb89ce` (`origin/stable` tip after #654) |
| Ubuntu 2230 | Reachable; still `cpn 1.4.7` (not upgraded this pass; AL10 preferred) |

Session cookie after tip upgrade is port-scoped: `cpn_panel_session_2087` (not bare `cpn_panel_session`).

### Actions exercised

| # | Category | Action | HTTP | Audit row |
|---|----------|--------|------|-----------|
| 1 | Auth | Wrong-password POST `/login` | 401 | `action=auth.login.fail` `category=auth` `ok=false` `ip=10.0.2.2` |
| 1 | Auth | Correct POST `/login` | 200 `/dashboard` + session | `action=auth.login` `category=auth` `ok=true` |
| 1 | Auth | Logout | not run | remaining |
| 15 | Settings | GET+POST `/settings/audit` (retention + empty webhook) | 200 | `action=settings.audit` `category=settings` `message=Audit policy saved` |
| 9 | Firewall | GET `/security/firewall` + POST `/security/firewall/reload` | 200 | `action=firewall.reload` `category=firewall` (reload failed lab zone parse; **row still written**) |

### Store confirmation

```text
/var/lib/cpn/audit/audit.jsonl   mode 600; ~19 lines after smoke
/var/lib/cpn/panel-actions.jsonl legacy still present (older host/plugin rows)
```

Category counts after smoke (JSON parse on guest): `auth=5`, `settings=1`, `firewall=1`, plus legacy migrated rows without `category`.

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

### CLI note

There is **no** standalone `cpn audit` subcommand on this tip (`cpn --help` has no `audit`). Privileged CLI actions are expected to call `panel_audit_cli::record_cli` (for example MFA/password paths). Dedicated CLI mutation was **not** run this pass (avoid resetting lab MFA/password). Checklist item 17 remains open.

### Helpers used (repo only)

- `to-do/_lab_ssh_once.py`
- `to-do/_verify2a_upgrade_probe.py` (probe / start-upgrade / upgrade-status)
- `to-do/_verify2a_post_upgrade_smoke.py` (auth / settings / firewall + store inspect)

## Manual lab smoke checklist

Run on a panel that includes Phase 2a. Confirm each category yields a redacted row in `/var/lib/cpn/audit/audit.jsonl` (or Settings > Audit trail) with actor, IP when available, and outcome.

| # | Category | Suggested action | Pass? |
|---|----------|------------------|-------|
| 1 | Auth | Failed login + successful login + logout | [x] fail+ok (logout remaining) |
| 2 | Identity | Password change or TOTP/passkey enroll rename | [ ] |
| 3 | Accounts / ACL | Create or deactivate a non-owner user | [ ] |
| 4 | API tokens | Issue then revoke a `cpn_` token | [ ] |
| 5 | Packages / plugins / themes | Install or activate a host/plugin action | [ ] |
| 6 | Hosting packages | Edit/assign package limits | [ ] |
| 7 | Websites / WP / apps | Create or suspend a site (lab-only) | [ ] |
| 8 | DB / FTP / SFTP | One mutating DB or FTP action | [ ] |
| 9 | Firewall / security | Start/stop firewall or Fail2Ban unban | [x] reload exercised (lab zone error; audited) |
| 10 | DNS / Cloudflare | Create or edit a DNS record (lab zone) | [ ] |
| 11 | Email | Create mailbox or change mailbox password | [ ] |
| 12 | Backup / restore | Trigger backup job if available | [ ] |
| 13 | Docker / PHP / services | Start/stop a container or service toggle | [ ] |
| 14 | Files | mkdir or upload in root File Manager | [ ] |
| 15 | Settings / repair / migrate | Change audit retention or heal | [x] audit policy POST |
| 16 | Updates | Version check POST that mutates state, or note denylist if read-only | [ ] |
| 17 | CLI | `cpn mfa clear` dry path or `cpn password` on lab account (operator) | [ ] |
| 18 | Notifications | Remote notify if enabled | [ ] |
| 19 | UI prefs | Toggle a UI preference (opt-outable) | [ ] |

### Export / policy / alerts (lab)

- [ ] Settings > Audit trail: security-essential categories stay locked on (page loads; locked UI not screenshot-verified)
- [ ] Non-essential opt-out skips `ui` (matches unit test; lab UI toggle not exercised)
- [x] Export JSONL and CSV download; simple secret-smoke found no lab passwords / wrong-password string
- [ ] Optional webhook receives redacted JSON only (skipped)
- [ ] Auth fail spike notifies panel admins (skipped; inbox/notify not verified)

## Still required to close `verify-2a-audit-trail`

1. Finish remaining category rows (identity through UI prefs), including logout for auth.
2. Complete policy lock / UI opt-out UI checks; webhook + auth-fail spike notify (or document permanent skip).
3. Exercise one privileged CLI path that calls `panel_audit_cli::record_cli` without destroying lab MFA.
4. Keep [audit-coverage.md](audit-coverage.md) with no uncovered mutating routes outside the reviewed denylist.

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
