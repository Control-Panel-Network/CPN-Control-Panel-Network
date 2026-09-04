# GitHub issues sweep (04/09/2026)

Repo: Control-Panel-Network/CPN-Control-Panel-Network

Branch: `fix/github-issues-sweep-zero`

## Closable after this PR

| ID | Fix |
|----|-----|
| #1 | Loopback default; remote mode token fingerprint + Bearer/cookie exchange; Origin check on mutating APIs |
| #4 | EL9 `php:8.2`; EL8 Remi `remi-8.2`; refuse PHP &lt; 8.2 after install |
| #7 | Real Vite/Panel builds; blocking `cargo audit` (+ documented h2 ignore); npm audit; privileged matrix stays manual |
| #8 | Panel POST login + HttpOnly session cookie; `/dashboard` requires session (preview optional) |
| #9 | Postfix/Dovecot + listener checks + ephemeral SMTP/IMAP roundtrip probe |
| #10 | Random `/var/tmp/cpn-dl-*` with `O_EXCL`/`0600` (already on main; kept) |
| #12 | Commit `Cargo.lock`; CI/release `--locked` only |
| #13 | Per-run journal `run_id`; scoped rollback; `WroteRepo` removable |
| #16 | Hard RPM + real SBOM + broader attestations; optional GPG/cosign when secrets exist |
| #17 | `check-version-sync.sh`; multi-arch RPM discovery |
| #18 | `kill_on_drop`, timeouts, process groups, curl `--max-time` |
| #20 | `force_reinstall` required to re-run server/mail when already ready |
| #21 | Firewall open must succeed before `external_ports_configured=true` |
