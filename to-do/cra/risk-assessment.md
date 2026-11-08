# Art. 13 cybersecurity risk assessment (living draft)

**Status:** Started (Phase 1). Not a completed Annex VII technical file.  
**Regulation hook:** Regulation (EU) 2024/2847 Art. 13 (cybersecurity risk assessment informing essential requirements).  
**Update cadence:** After material product changes, after significant incidents, and at least annually once the conformity file is active.

## Scope

| Item | Value |
|------|-------|
| Product | CPN Control Panel Network (installer + panel) |
| Assets in scope | Panel process, CLI/`cpn` privileged tools, update pipeline, MFA material under `/var/lib/cpn/mfa/`, site/plugin data under `/home/<domain>/`, mail/DB/Docker host packages when installed |
| Out of scope (for this draft) | Unmodified third-party upstream CVEs after install (tracked as supply-chain residual risk); customer website application code |

## Method (lightweight, living)

1. Identify assets, trust boundaries, and privileged actions.
2. Identify threats and misuse cases (including operator error and remote exposure).
3. Rate residual risk: **Critical / High / Medium / Low** after existing controls.
4. Record treatment: mitigate, accept (with owner), transfer, or avoid.
5. Map open treatments into [annex-I-evidence-matrix.md](annex-I-evidence-matrix.md) and engineering backlog.

Severity for product vulns also follows [SECURITY.md](../../SECURITY.md) SLAs. Do not invent exploit PoCs in this file.

## Trust boundaries

| Boundary | Notes |
|----------|-------|
| Operator host root | Installer and many panel actions assume root or equivalent; local root is trusted for install |
| Panel listen port | Default bind `127.0.0.1:2087`; `--allow-remote` / `CPN_ALLOW_REMOTE=1` expands to `0.0.0.0` without TLS on the listen port |
| Public HTTPS hostname | Reverse proxy + Let's Encrypt when configured; listen port remains plain HTTP |
| Update source | Official GitHub Releases by default; optional operator fork + token under `/var/lib/cpn/` (mode 600) |
| MFA storage | `/var/lib/cpn/mfa/` must survive upgrade/restore; wipe is a critical integrity/availability risk |
| Host packages | MariaDB, web server, mail, Docker/Podman expand attack surface when enabled |

## Priority risk register (initial)

| ID | Scenario | Impact | Likelihood (qual.) | Existing controls | Residual | Treatment |
|----|----------|--------|--------------------|-------------------|----------|-----------|
| R-01 | Privileged installer reachable remotely without TLS (`--allow-remote`) | Full host compromise if token leaked | Medium when opted in | Default localhost bind; token in console URL; docs warn | High | Document; prefer SSH forward; keep remote opt-in explicit |
| R-02 | Update pipeline installs tampered package | Remote code execution as root | Low if verification on | HTTPS assets, `SHA256SUMS` + GPG, attestations | Medium | Keep gates; expand SBOM; fail closed on verify |
| R-03 | MFA keys wiped or regenerated on upgrade | Account lockout / MFA bypass confusion | Medium historically as ops risk | Persist under `/var/lib/cpn/mfa/` mode 600; upgrade must not wipe | High if regression | Regression tests; risk entry for any CodeQL/ops exception |
| R-04 | Weak or default admin credentials | Account takeover | Medium without policy | Reserved username list, password policy, mandatory TOTP/passkey before dashboard | Medium | Keep enroll gate; lockout; CVD for auth bugs |
| R-05 | Missing audit trail for privileged mutations | Undetected abuse / CRA Annex I gap | High today (partial log only) | Partial `panel_action_log` | High | Phase 2a full audit ([audit-coverage.md](audit-coverage.md)) |
| R-06 | Mail / DB / Docker host packages misconfigured | Data exposure, spam relay, container escape residual | Medium | Domain jails, SELinux notes, uninstall confirmations | Medium | Hardening docs; risk reviews per package |
| R-07 | Known unfixed critical/high dependency CVE shipped | CRA Annex I (2)(a) gap | Medium while alerts open | Dependabot, CodeQL, cargo audit | High while open | Phase 1b clear alerts; release CVE gate Phase 2b |
| R-08 | Public disclosure without upgrade path | User harm / premature exploit | Low with process | Private reporting; advisory after fix | Medium | Art. 14 + CVD playbooks; public log Phase 2c |
| R-09 | Actively exploited vuln / severe incident not reported in Art. 14 windows | Regulatory failure from 11/09/2026 | Medium without ENISA account | Draft playbook; PSIRT intended | High until ops ready | [reporting-playbook.md](reporting-playbook.md) + legal ENISA account |
| R-10 | Support period unclear at download | Users run unsupported builds; CRA transparency gap | Medium | Version UI "latest two"; SECURITY historically "best effort" | Medium | [support-period.md](support-period.md); align SECURITY/SUPPORT |

## Accepted risks (must name an owner)

None accepted yet in this scaffold. Any CodeQL dismissals or deferred mitigations must be listed here with owner and review date before calling Annex I closed.

| ID | Acceptance text | Owner | Review by (dd/mm/yyyy) |
|----|-----------------|-------|------------------------|
| _(none)_ | | | |

## Data inventory / minimisation (stub)

| Data class | Purpose | Retention intent | Notes |
|------------|---------|------------------|-------|
| Account credentials (hashed) | Auth | Life of account | Never log plaintext passwords |
| MFA secrets / passkey material | 2FA | Life of enrollment | `/var/lib/cpn/mfa/`; never in git or audit payloads |
| Panel action / future audit logs | Security monitoring | Policy default 90 days (Phase 2a) | Redact secrets |
| Auto-upgrade / phone-home style signals | Ops (if any) | Per feature docs | Non-essential must be opt-outable under CRA logging rules |

Expand into a full inventory before Annex VII freeze.

## Links to evidence

- CVD: [SECURITY.md](../../SECURITY.md)
- Support period: [support-period.md](support-period.md)
- Annex I matrix: [annex-I-evidence-matrix.md](annex-I-evidence-matrix.md)
- Reporting: [reporting-playbook.md](reporting-playbook.md)

## Change log

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Initial living risk assessment scaffold with priority register R-01 to R-10 |
