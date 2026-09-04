# GitHub issues remaining acceptance (2026-09-04)

Branch: `fix/issues-remaining-acceptance`

## Verification run locally

- `cargo test --locked` → **71 passed**
- `node Panel/scripts/auth-selftest.mjs` → **ok**

## Per-issue status after this PR

| Issue | Code in PR | Close now? | Notes |
|------|------------|------------|-------|
| #1 | Session bootstrap cookie, strip `?token=`, Bearer headers, WS Origin | Leave open until CI + SPA smoke | Query bootstrap still allowed once |
| #4 | `php_lifecycle` EOL table + recipe PHP ranges + runtime assert | Close after CI green | Automated EOL gate added |
| #7 | Matrix failure artifacts + mail job | Leave open if mail not on every push | Artifacts on failure added |
| #8 | No hardcoded prod secret + auth-selftest | Close after CI green | Reads `panel-session.secret` |
| #9 | Mail E2E via `os-matrix` scope `mail`/`extended` | Leave open until mail job green on Actions | Roundcube matrix job added |
| #10 | RAII temp dir + symlink/clobber tests | Close after CI green | Drop cleans `/var/tmp/cpn-dl-*` |
| #12 | `build-rpm.sh --locked` + require Cargo.lock | Close after CI green | Clear acceptance met |
| #13 | Broader preflight + failure-injection retry test | Leave open | Soft network/port notes; not full stage-injection suite |
| #16 | Tag releases require GPG | Keep open | Needs org GPG secrets + rpmsign proof |
| #18 | Cancel flag + SIGINT/SIGTERM server stop | Leave open | Need lab proof child process group dies |
| #20 | `installer_transitions` unit tests | Close after CI green | All invalid transitions covered in unit tests |
| #21 | Journal `created=` + cleanup helper + non-loopback curl | Leave open | Need firewalld-active lab proof |

Policy: do not close until merge + CI evidence + acceptance met.
