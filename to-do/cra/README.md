# CPN CRA compliance status

Living dashboard for Regulation (EU) 2024/2847 (Cyber Resilience Act) engineering and legal scaffolding for **CPN Control Panel Network**.

**This folder is engineering evidence and checklists.** It does **not** claim that CPN is CRA-conformant, CE-marked, or that a signed EU Declaration of Conformity exists. Binding legal outcomes stay with the manufacturer signatory and counsel.

## Quick status (engineering scaffold)

| Area | Status | Notes |
|------|--------|-------|
| Phase 0 scaffold (`to-do/cra/`) | In progress | This folder |
| Classification (working) | Draft | Important Class I assumption; counsel confirmation pending ([classification.md](classification.md)) |
| Legal track (5 items) | Checklist only | [legal-track.md](legal-track.md); no binding signatures in git |
| Art. 13 risk assessment | Started | [risk-assessment.md](risk-assessment.md) |
| Annex I evidence matrix | Draft | [annex-I-evidence-matrix.md](annex-I-evidence-matrix.md) |
| Support period policy | Draft | [support-period.md](support-period.md); aligned with SECURITY.md / docs/SUPPORT.md |
| SECURITY.md CVD | Draft upgrade | Root [SECURITY.md](../../SECURITY.md) |
| Art. 14 reporting playbook | Draft | [reporting-playbook.md](reporting-playbook.md); ENISA account is legal/ops |
| Audit coverage matrix | In progress (Phase 2a) | [audit-coverage.md](audit-coverage.md); store `/var/lib/cpn/audit/audit.jsonl`, deny-by-default middleware, 90-day retention |
| Public advisory log / `/security` | Other workstream | Expected: `docs/SECURITY-ADVISORIES.md`, `docs/security-advisories.json`, https://cpn.newstargeted.com/security |
| Dependabot / Code Scanning clear | Other workstream | Phase 1b |
| Phase 2b SBOM + security metadata + audit gate + USER-SECURITY | Draft / wired | Release SBOMs + `cpn-security-release.json`; [release-audit-gate.md](release-audit-gate.md); [docs/USER-SECURITY.md](../../docs/USER-SECURITY.md) |
| Annex VII technical file / DoC / CE | Draft scaffolds only | [technical-file/](technical-file/README.md) (unsigned DoC; CE **not** affixed) |
| Phase 4 operate runbook | Draft | [operate-runbook.md](operate-runbook.md) |

## Commission timeline (reference)

| Date | Obligation |
|------|------------|
| **11 September 2026** | Art. 14 reporting (actively exploited vulnerabilities and severe incidents) via ENISA Single Reporting Platform |
| **11 December 2027** | Full essential requirements, vulnerability handling expectations, CE marking, EU Declaration of Conformity for covered placements / substantial modifications |

As of the scaffold date (**08/11/2026**), the Art. 14 reporting start date has already passed. Treat the playbook and ENISA account as **active obligations**, not future-only prep.

## Manufacturer (draft, confirm with counsel)

| Field | Working value |
|-------|----------------|
| Trade name | CPN (Control Panel Network) |
| Manufacturer (commercial) | News Targeted / Control-Panel-Network |
| Product family | Hosting control panel + installer (privileged server management) |
| Public product site | https://cpn.newstargeted.com |
| Source / releases | https://github.com/Control-Panel-Network/CPN-Control-Panel-Network |
| PSIRT contact (intended) | `security@newstargeted.com` (confirm mailbox live before advertising as sole channel) |
| GitHub private reporting | Repository Security Advisories / Private Vulnerability Reporting |

Do not put ENISA credentials, signing keys, insurance policy numbers, or personal contact details beyond public PSIRT mail in this repo.

## File index

| File | Purpose |
|------|---------|
| [classification.md](classification.md) | Class I working assumption and counsel questions |
| [legal-track.md](legal-track.md) | Checklist for the five legal/manufacturer items |
| [risk-assessment.md](risk-assessment.md) | Living Art. 13 cybersecurity risk assessment |
| [annex-I-evidence-matrix.md](annex-I-evidence-matrix.md) | Annex I requirements mapped to evidence / gaps |
| [support-period.md](support-period.md) | Latest-two releases + calendar end-date guidance |
| [reporting-playbook.md](reporting-playbook.md) | Art. 14 24h / 72h / final reporting process |
| [audit-coverage.md](audit-coverage.md) | Full audit trail coverage matrix (stub until Phase 2a) |
| [release-audit-gate.md](release-audit-gate.md) | Phase 2b cargo/npm critical/high release gate notes |
| [audit-waivers.json](audit-waivers.json) | Waiver ids for the release audit gate (no secrets) |
| [technical-file/](technical-file/README.md) | Annex VII outline, unsigned DoC draft, CE notes |
| [operate-runbook.md](operate-runbook.md) | Phase 4 SLA / ENISA / retention reminders |

## Related product docs

- [SECURITY.md](../../SECURITY.md): coordinated vulnerability disclosure (CVD)
- [docs/USER-SECURITY.md](../../docs/USER-SECURITY.md): Annex II style user security instructions (including secure deletion)
- [docs/SUPPORT.md](../../docs/SUPPORT.md): OS support tiers + security support period pointer
- Future: [docs/SECURITY-ADVISORIES.md](../../docs/SECURITY-ADVISORIES.md) and https://cpn.newstargeted.com/security (public fixed-vuln log)
- [docs/RELEASES.md](../../docs/RELEASES.md): signed release distribution, SBOMs, security metadata

## How to update this dashboard

1. After completing an engineering CRA deliverable, flip the matching row above and link evidence (PR, path, CI URL).
2. Legal rows stay **pending** until counsel/ops mark them done in [legal-track.md](legal-track.md) with off-repo evidence pointers (no secrets).
3. Never mark CRA-ready in public copy until Phase 3 legal outcomes and engineering verify gates agree.
