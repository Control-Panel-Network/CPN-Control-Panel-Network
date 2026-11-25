# Annex I evidence matrix (draft)

Maps CRA Annex I essential cybersecurity requirements to CPN evidence and gaps.  
**Status:** Draft scaffold. Rows marked Open/Partial are **not** conformity claims.

Legend: **Done** (usable evidence exists) | **Partial** | **Open** | **N/A** | **Legal** (manufacturer/counsel).

## Part I: Product properties

| Req (summary) | Status | Evidence today | Gap / next action |
|---------------|--------|----------------|-------------------|
| Risk-based design | Partial | [risk-assessment.md](risk-assessment.md) started | Complete living assessment; annual review |
| No known exploitable vulns shipped | Partial | Dependabot, CodeQL, cargo audit | Clear open alerts (Phase 1b); release CVE gate (Phase 2b) |
| Secure by default + reset | Partial | Localhost bind; MFA enroll; password policy; reserved usernames | Written factory-reset / secure-delete UX docs (Phase 2b) |
| Security updates (free, automatic, postponable) | Partial | Auto-update default ON; Version UI | Security-vs-feature channel; postpone UX; free-update statement |
| Authz + report unauthorised access | Partial | Strong auth; partial action log | Full audit trail + alerts (Phase 2a) |
| Confidentiality / integrity | Partial | Reverse-proxy TLS; signed updates | Document threat model; TLS on listen port remains out of band |
| Data minimisation | Open | Stub inventory in risk assessment | Full data inventory / minimisation policy |
| Availability / DoS resilience | Partial | Rate limits on some auth paths; repair tooling | Document and test residual DoS cases |
| Attack surface / exploit mitigation | Partial | Ongoing hardening | Map each major surface in risk assessment |
| Security logging + opt-out for non-essential | Partial | Narrow panel action log | Full trail, retention, export, opt-out (Phase 2a) |
| Secure permanent data deletion | Partial | `cpn uninstall` / purge flags | Surface in UI/docs as user right (Phase 2b) |

## Part II: Vulnerability handling

| Req (summary) | Status | Evidence today | Gap / next action |
|---------------|--------|----------------|-------------------|
| Machine-readable SBOM | Partial | CycloneDX on tagged releases for installer | Expand to `installer-ui/` and `Panel/` npm trees |
| Remediate without delay; security updates | Partial | Release process; CVD draft SLAs | Enforce severity SLAs; prefer security-only releases when feasible |
| Regular security tests | Partial | CI CodeQL / audits | Formal schedule + reports in technical file |
| Public disclose fixed vulns | Open / Parallel | GHSA intake exists | `docs/SECURITY-ADVISORIES.md` + https://cpn.newstargeted.com/security (Phase 2c) |
| Coordinated disclosure policy | Partial | Root SECURITY.md upgraded for CRA CVD | Keep PSIRT mailbox live; publish advisories section |
| Secure auto distribution of updates | Partial | SHA256SUMS, GPG, attestations, HTTPS installers | Document as free security updates during support period |

## Process / manufacturer obligations (adjacent to Annex I)

| Obligation | Status | Pointer |
|------------|--------|---------|
| Art. 13 risk assessment | Partial | [risk-assessment.md](risk-assessment.md) |
| Support period with calendar end-date guidance | Partial | [support-period.md](support-period.md) |
| Art. 14 reporting | Partial (playbook) / Legal (ENISA account) | [reporting-playbook.md](reporting-playbook.md), [legal-track.md](legal-track.md) |
| Annex II user instructions | Open | Future `docs/USER-SECURITY.md` (Phase 2b) |
| Annex VII technical file | Open | Phase 3 `to-do/cra/technical-file/` |
| EU DoC + CE | Legal | [legal-track.md](legal-track.md) item 4 |
| Classification / notified body | Legal | [classification.md](classification.md), [legal-track.md](legal-track.md) |

## Critical open engineering themes

1. Full security audit trail (Annex I (2)(d) and (2)(l)): see [audit-coverage.md](audit-coverage.md).
2. Zero open Dependabot / Code Scanning on `stable` (Phase 1b).
3. Public advisory disclosure log (Phase 2c).
4. Expanded SBOM + release CVE gates (Phase 2b).

## Change log

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Initial Annex I evidence matrix draft |
