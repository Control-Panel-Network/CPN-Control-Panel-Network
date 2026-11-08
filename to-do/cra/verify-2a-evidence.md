# Verify 2a evidence (Phase 2a full panel audit trail)

Recorded: **08/11/2026** (engineering). Plan todo: `verify-2a-audit-trail`.

**Verdict this pass:** unit/integration evidence from PR **#654** (`feat/phase2a-full-panel-audit`) is green for the `panel_audit*` filter. Lab category smoke is **not** completed here (manual checklist below). Do **not** mark the plan `verify-2a-audit-trail` todo completed until lab smoke + export/webhook checks pass on an install that includes Phase 2a.

Coverage matrix: [audit-coverage.md](audit-coverage.md) (authoritative on the audit PR; this branch may lag until merge).

## Unit / integration (in-tree)

Source branch / tip used for this evidence:

| Field | Value |
|-------|-------|
| PR | [#654](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/654) |
| Branch | `feat/phase2a-full-panel-audit` |
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

## Lab SSH status this pass

Repo helpers exist (`Test/lab_ssh_util.py`, `to-do/_lab_ssh_once.py`). AGENTS known NAT ports were probed only (no credential search):

| Host port | Probe 08/11/2026 | AGENTS lab (reference) |
|-----------|------------------|------------------------|
| 2226 | closed | AlmaLinux 9 clean2 |
| 2228 | closed | AlmaLinux 10 install |
| 2229 | open | AlmaLinux 10 prerelease |
| 2230 | open | Ubuntu 26.04 clean |

**Lab smoke not run:** Phase 2a audit binary is not assumed deployed on the open labs; credentialed SSH was not used in this pass. Prefer deploying PR #654 (or merged stable tip with Phase 2a) then running the checklist below.

Optional helper (operator-run; never print secrets):

```bash
python to-do/_lab_ssh_once.py --port 2229 'cpn --version'
```

## Manual lab smoke checklist (remaining)

Run on a panel that includes Phase 2a. Confirm each category yields a redacted row in `/var/lib/cpn/audit/audit.jsonl` (or Settings > Audit trail) with actor, IP when available, and outcome.

| # | Category | Suggested action | Pass? |
|---|----------|------------------|-------|
| 1 | Auth | Failed login + successful login + logout | [ ] |
| 2 | Identity | Password change or TOTP/passkey enroll rename | [ ] |
| 3 | Accounts / ACL | Create or deactivate a non-owner user | [ ] |
| 4 | API tokens | Issue then revoke a `cpn_` token | [ ] |
| 5 | Packages / plugins / themes | Install or activate a host/plugin action | [ ] |
| 6 | Hosting packages | Edit/assign package limits | [ ] |
| 7 | Websites / WP / apps | Create or suspend a site (lab-only) | [ ] |
| 8 | DB / FTP / SFTP | One mutating DB or FTP action | [ ] |
| 9 | Firewall / security | Start/stop firewall or Fail2Ban unban | [ ] |
| 10 | DNS / Cloudflare | Create or edit a DNS record (lab zone) | [ ] |
| 11 | Email | Create mailbox or change mailbox password | [ ] |
| 12 | Backup / restore | Trigger backup job if available | [ ] |
| 13 | Docker / PHP / services | Start/stop a container or service toggle | [ ] |
| 14 | Files | mkdir or upload in root File Manager | [ ] |
| 15 | Settings / repair / migrate | Change audit retention or heal | [ ] |
| 16 | Updates | Version check POST that mutates state, or note denylist if read-only | [ ] |
| 17 | CLI | `cpn mfa clear` dry path or `cpn password` on lab account (operator) | [ ] |
| 18 | Notifications | Remote notify if enabled | [ ] |
| 19 | UI prefs | Toggle a UI preference (opt-outable) | [ ] |

### Export / policy / alerts (lab)

- [ ] Settings > Audit trail: security-essential categories stay locked on
- [ ] Non-essential opt-out skips `ui` (matches unit test)
- [ ] Export JSONL and CSV download; grep confirms no passwords / TOTP / token values
- [ ] Optional webhook receives redacted JSON only (or document skipped)
- [ ] Auth fail spike notifies panel admins (or document skipped if inbox disabled)

## Still required to close `verify-2a-audit-trail`

1. Tick every category row above on a live lab install of Phase 2a.
2. Complete export / opt-out / alerts checks.
3. Keep [audit-coverage.md](audit-coverage.md) with no uncovered mutating routes outside the reviewed denylist (follow-up map polish may remain as `remaining` notes).

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
