# Art. 14 reporting playbook (draft)

**Regulation:** Regulation (EU) 2024/2847 Article 14 (reporting obligations for manufacturers).  
**Channel:** ENISA Single Reporting Platform (manufacturer account).  
**Status:** Process draft for PSIRT / ops. **Does not** create an ENISA account by itself. Account setup is [legal-track.md](legal-track.md) item 3.

**Important timing:** Reporting obligations apply from **11 September 2026**. Scaffold date **08/11/2026** means this playbook must be treated as **live process**, not future-only preparation.

## What must be reported

Report through the ENISA platform when CPN (as manufacturer product) is affected by:

1. **Actively exploited vulnerability** in the product (or components under manufacturer control that affect CPN's essential cybersecurity properties), and/or
2. **Severe incident** impacting the security of the product (availability, integrity, or confidentiality at scale for users), per CRA Art. 14 and applicable Commission guidance.

Do **not** use this playbook for routine bug reports that are not actively exploited and are not severe incidents. Those follow [SECURITY.md](../../SECURITY.md) CVD.

When unsure, escalate to counsel / PSIRT lead within the early-warning window rather than waiting.

## Clock starts when

**Awareness** means the manufacturer (PSIRT, on-call, or equivalent) has credible information that an actively exploited vulnerability or severe incident affects CPN. Sources include:

- Confirmed exploitation in the wild (telemetry, customer report with evidence, trusted CERTs)
- Maintainer verification that a public exploit targets a shipped CPN version
- Severe production incident (for example mass auth bypass, signing-key compromise, update-pipeline compromise)

Record `aware_at` in UTC and local Europe/Oslo for the incident ticket (off-repo). Display dates to operators as `dd/mm/yyyy` and 24h time.

## Notification stages

| Stage | Deadline from awareness | Purpose | Minimum content |
|-------|-------------------------|---------|-----------------|
| **Early warning** | **24 hours** | Alert ENISA / CSIRT routing that something serious is ongoing | Product identity, nature (actively exploited vuln vs severe incident), first-seen time, rough severity, contact |
| **Main notification** | **72 hours** | Substantive update | Affected versions, impact, mitigation/workaround if any, remediation plan, whether users should take urgent action |
| **Final / complementary report** | Per platform guidance after handling | Close the loop | Root cause summary (no exploit PoC), fixed versions, advisory ids, lessons learned |

If information is incomplete at 24h or 72h, submit what is known and mark gaps explicitly. Missing fields are better than missing the deadline.

## Roles

| Role | Responsibility |
|------|----------------|
| PSIRT on-call | Own the clock; open ticket; draft notifications; coordinate fix |
| Engineering lead | Confirm affected versions; prepare patch; verify upgrades |
| Manufacturer ops | ENISA platform access; submit filings; store receipt ids off-repo |
| Counsel | Classification of "severe incident" / legal questions; external authority contact when needed |
| Communications | User notices after patch path exists; align with public advisory log |

Never commit ENISA passwords, API tokens, or personal phone numbers into git. Store them in the ops vault referenced from [legal-track.md](legal-track.md).

## Decision flow

1. Intake (GH private advisory, `security@newstargeted.com`, customer, CERT).
2. Triage: is this Art. 14 (active exploit / severe incident) or CVD-only?
3. If Art. 14: start 24h clock; page on-call; open off-repo incident ticket.
4. Contain: revoke keys, disable broken update channel, publish interim mitigation if safe.
5. File early warning on ENISA platform before 24h.
6. Deepen analysis; file main notification before 72h.
7. Ship fix on `stable`; cut security or patched release; verify signatures.
8. Publish GHSA / public advisory entry (Phase 2c surfaces) when users have a reasonable upgrade path.
9. File final / complementary report; archive evidence retention per counsel.

## Severity quick guide (operational)

Align product vuln severity with SECURITY.md. Art. 14 is about **exploitation / severe incident**, not only CVSS label.

| Signal | Likely Art. 14? |
|--------|-----------------|
| Confirmed in-the-wild exploit against shipped CPN | Yes (actively exploited vulnerability) |
| Stolen release signing key or update CDN compromise | Yes (severe incident and/or vuln handling emergency) |
| Single-tenant misconfiguration with no product defect | Usually no (customer ops); still help the operator |
| Unexploited high CVE with patch in progress | CVD path; watch for exploitation |

## Templates (copy into the ENISA form / ticket; redact secrets)

### Early warning (24h)

```text
Manufacturer: [legal entity]
Product: CPN Control Panel Network
Trade name / versions affected (best known): [...]
Type: actively exploited vulnerability | severe incident
Aware at (UTC): YYYY-MM-DDTHH:MM:SSZ
Summary (no exploit steps): [...]
Known impact: confidentiality | integrity | availability | unknown
Interim user action: [...]
Contact: security@newstargeted.com (PSIRT)
Incident internal id: [off-repo id]
```

### Main notification (72h)

```text
Internal id: [...]
ENISA early-warning reference: [...]
Confirmed affected versions: [...]
Fixed / expected fixed version: [...]
Technical impact (high level): [...]
Exploitation status: confirmed | suspected | unknown
Mitigations available before upgrade: [...]
Distribution channels for fix: GitHub Releases / panel auto-update / install scripts
Advisory plan: GHSA [...] / CPN-YYYY-NNN (when assigned)
Next update expected: [...]
```

### Final / complementary

```text
Internal id: [...]
Root cause category: [...]
Remediation shipped: version [...] on dd/mm/yyyy
Public advisory URLs: [...]
Residual risk / monitoring: [...]
Process improvements: [...]
```

## Coordinated public disclosure

- Prefer private handling until a signed upgrade path exists ([SECURITY.md](../../SECURITY.md)).
- After fix: publish via GitHub Security Advisories and the public log (`docs/SECURITY-ADVISORIES.md` / https://cpn.newstargeted.com/security when those surfaces exist).
- Do not paste exploit PoCs into public advisories or this repo.

## Tabletop dry-run (required before relying on the process)

| Step | Pass criteria |
|------|---------------|
| Locate ENISA account credentials in ops vault | Success without asking chat/git |
| Draft 24h early warning from template in under 30 minutes | Complete draft with redacted sample data |
| Identify who submits the filing | Named primary + backup |
| Exercise 72h main notification fields | Versions, impact, mitigation filled |
| Confirm SECURITY.md / support contacts still valid | Mailbox and GH private reporting work |

Log dry-run date in [legal-track.md](legal-track.md) item 3. Do not store sample PII.

## Retention

Keep Art. 14 filings, awareness timestamps, and authority correspondence in the manufacturer compliance vault for the period counsel specifies (align with technical file retention once Phase 3 starts). Git holds process only.

## Related documents

- [SECURITY.md](../../SECURITY.md): CVD and severity SLAs
- [legal-track.md](legal-track.md): ENISA account checklist
- [risk-assessment.md](risk-assessment.md): R-09 reporting readiness risk
- [support-period.md](support-period.md): which versions are in support when messaging users

## Change log

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Initial Art. 14 playbook draft (24h / 72h / final; templates; dry-run) |
