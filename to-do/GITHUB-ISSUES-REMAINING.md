# Remaining GitHub issues (re-verified 04/09/2026)

Open after `gh issue list --state open`: **#1, #8, #16, #18**.

| Issue | Gap on main | Plan | Close? |
|-------|-------------|------|--------|
| #1 | Host/Origin allowlist trusted client `Host` | Server-configured `allowed_hosts`; reject attacker Host+Origin | Yes after CI + unit tests |
| #8 | No Actix HTTP tests for login/dashboard | `auth_http_tests.rs` covers valid login, bad password, unauth dashboard | Yes after CI |
| #18 | Cancel not observed while child runs; lab killed PGID only | `run_command` select on cancel; `request_cancel` kills PGIDs; unit + lab | Yes after CI + AL9 proof |
| #16 | No repo GPG secrets; no signed `v*` release | Keep checksums/docs; **leave open** until signed release | No (ops blocker) |

Branch: `fix/issues-1-8-18-acceptance`.
