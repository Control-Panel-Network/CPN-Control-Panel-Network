# Phase 4 operate runbook (draft)

Operational reminders for running CPN under CRA expectations after engineering scaffolds exist. This is **not** a substitute for live ENISA account access, signed DoC, or insurance paperwork.

Related: [reporting-playbook.md](reporting-playbook.md), [support-period.md](support-period.md), [legal-track.md](legal-track.md), [technical-file/README.md](technical-file/README.md).

## 1. Support-period calendar

| Cadence | Action |
|---------|--------|
| Every GitHub Release | Confirm support end month/year guidance still matches [support-period.md](support-period.md) and [docs/SUPPORT.md](../../docs/SUPPORT.md) (latest-two policy) |
| Quarterly | Spot-check Version UI / download pages for the published end-date language |
| When dropping an old major line | Update SECURITY.md / SUPPORT.md and announce free security-update end dates clearly |

## 2. Vulnerability handling SLA reminders

Align severity response with [SECURITY.md](../../SECURITY.md) CVD SLAs (keep that file authoritative).

| Severity | Engineering target (reminder) |
|----------|-------------------------------|
| Critical | Patch or mitigating guidance without delay; prefer security release metadata (`CPN-SECURITY-RELEASE: true`) |
| High | Priority fix in the next security or patched release |
| Medium / Low | Schedule in normal releases; still disclose when fixed |

After a fix ships:

1. Publish/update GHSA when appropriate.
2. Append the public advisory log (Phase 2c JSON/MD + site `/security`).
3. Attach SBOMs and `cpn-security-release.json` on the release.
4. Run [release-audit-gate.md](release-audit-gate.md) clean (or documented waivers).

## 3. Art. 14 / ENISA reporting steps

Use the detailed process in **[reporting-playbook.md](reporting-playbook.md)**. Short checklist:

1. Confirm whether the event is an **actively exploited vulnerability** or a **severe incident** in scope of Art. 14.
2. Preserve logs; do not publish exploit PoCs.
3. Early warning target: **24h** via ENISA Single Reporting Platform (manufacturer account; credentials stay off-repo).
4. Main notification target: **72h**.
5. Final report when required by the playbook.
6. Coordinate user disclosure only after a reasonable upgrade path exists (see CVD policy).

If the ENISA account is not yet live, escalate the [legal-track.md](legal-track.md) ENISA row immediately; do not invent credentials in git.

## 4. Retain the technical file and signed materials

| Item | Where |
|------|-------|
| Living engineering evidence | `to-do/cra/` (this tree) |
| Signed DoC / CE authorisation | Manufacturer vault (pointer only in legal-track.md) |
| Notified-body certificates | Vault + NANDO ids in legal-track.md |
| Insurance / market-surveillance contacts | Vault + legal-track.md |

Retention duration: follow counsel; do not delete CRA evidence for versions still on the Union/EEA market.

## 5. Re-assess on substantial modifications

Treat the following as triggers to re-open risk assessment and, if needed, conformity assessment:

- New privileged remote-management surfaces
- Changes to update integrity / auto-update trust model
- MFA storage or authz model changes
- New host packages that expand network attack surface (mail, DB, reverse proxy)
- Classification or manufacturer entity changes

Update [risk-assessment.md](risk-assessment.md) and [annex-I-evidence-matrix.md](annex-I-evidence-matrix.md), then decide with counsel whether a new DoC / assessment is required.

## 6. Annual security review

Schedule a third-party or independent security review at least annually. Store the report reference (date `dd/mm/yyyy`, vendor, scope) in the technical file index without committing proprietary full reports if counsel forbids; keep a pointer in legal-track or an internal vault path.

## 7. Quick links

- User instructions: [docs/USER-SECURITY.md](../../docs/USER-SECURITY.md)
- Release SBOM / audit gate: [release-audit-gate.md](release-audit-gate.md)
- Reporting playbook: [reporting-playbook.md](reporting-playbook.md)
- Status dashboard: [README.md](README.md)
