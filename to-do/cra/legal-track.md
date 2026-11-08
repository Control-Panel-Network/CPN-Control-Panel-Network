# Legal and manufacturer track (checklist scaffolding)

**Purpose:** Track the five manufacturer/legal items that engineering cannot complete with code alone.  
**Rule:** Drafts and checklists live here. **Binding signatures, contracts, account credentials, and insurance policies stay off-repo.** Record only status, owners, deadlines, and non-secret pointers.

Working classification assumption until item 1 completes: **Important Class I** ([classification.md](classification.md)).

## Summary

| # | Item | Owner | Target | Status |
|---|------|-------|--------|--------|
| 1 | Final CRA product classification | Counsel + manufacturer | ASAP (blocks notified-body spend) | Draft only (Class I working assumption) |
| 2 | Notified body engagement / contract | Manufacturer + counsel | Before Dec 2027 Class I placements if standards path incomplete | Checklist only |
| 3 | ENISA Single Reporting Platform manufacturer account | Manufacturer ops / PSIRT | Before **11/09/2026** (now overdue if not live) | Checklist only |
| 4 | Binding EU Declaration of Conformity + CE authorization | Legal manufacturer signatory | Before **11/12/2027** covered placements / substantial mods | Checklist only (no signature in git) |
| 5 | Insurance + market-surveillance representation | Manufacturer / counsel | Before first CRA-covered Union/EEA placement under full obligations | Checklist only |

Status values used below: `not_started` | `in_progress` | `blocked` | `done_off_repo` | `cancelled`.

---

## 1. Final legal classification (Class I vs default)

| Field | Value |
|-------|-------|
| Plan todo id | `legal-classification` |
| Status | `in_progress` (engineering draft exists; counsel memo pending) |
| Depends on | Product description; Annex III / Implementing Regulation (EU) 2025/2392 mapping |
| Outcome required | Written counsel memo; update [classification.md](classification.md) to "confirmed" or "default" |
| Off-repo evidence pointer | _(fill: vault path or ticket id only)_ |
| Done when | Counsel memo filed; README dashboard + this row updated; notified-body spend decision recorded |

### Checklist

- [ ] Manufacturer legal entity and EEA main establishment confirmed in writing
- [ ] Counsel maps CPN against Annex III / 2025/2392 categories
- [ ] Written memo states Class I **or** default (or other justified category)
- [ ] [classification.md](classification.md) updated to match memo (no conflicting "working assumption" language left)
- [ ] If default: item 2 cancelled or narrowed with written reason
- [ ] If Class I: item 2 remains required unless harmonised-standards path alone is accepted by counsel

---

## 2. Notified body contract

| Field | Value |
|-------|-------|
| Plan todo id | `legal-notified-body` |
| Status | `not_started` (scaffolding only; do not fake a contract) |
| Depends on | Item 1 = Class I (or explicit decision to use Module A with applicable harmonised standards) |
| Outcome required | Signed engagement; assessment schedule; NANDO body id |
| Off-repo evidence pointer | _(fill: contract vault ref / NANDO id only)_ |
| Done when | Signed contract on file off-repo; schedule exists; this checklist complete |

### Checklist

- [ ] Classification memo confirms Class I (or counsel documents standards-only Module A path)
- [ ] Shortlist notified bodies from NANDO for CRA / relevant modules
- [ ] Engagement letter / contract signed by manufacturer (not stored in git)
- [ ] Assessment schedule recorded (milestones toward Dec 2027)
- [ ] NANDO body id recorded in this file (public id only; no credentials)
- [ ] If cancelled: written reason linked from item 1

---

## 3. ENISA Single Reporting Platform manufacturer account

| Field | Value |
|-------|-------|
| Plan todo id | `legal-enisa-account` |
| Status | `not_started` (ops checklist; credentials never in git) |
| Depends on | Manufacturer entity; main-establishment CSIRT routing; live PSIRT process ([reporting-playbook.md](reporting-playbook.md)) |
| Hard deadline | **11 September 2026** (Art. 14 reporting start). As of **08/11/2026**, treat as **urgent if account is not already live**. |
| Outcome required | Live manufacturer account; credentials in secure ops store; dry-run logged |
| Off-repo evidence pointer | _(fill: password manager / ops vault entry name only)_ |
| Done when | Account live; dry-run logged; PSIRT knows how to access; playbook points to live account |

### Checklist

- [ ] Manufacturer entity and contact persons designated for ENISA reporting
- [ ] CSIRT routing / main establishment confirmed with counsel
- [ ] Manufacturer account created on ENISA Single Reporting Platform (when available)
- [ ] Credentials stored in secure ops vault (**not** git, **not** this file)
- [ ] Access limited to PSIRT / on-call with MFA
- [ ] Tabletop dry-run executed using [reporting-playbook.md](reporting-playbook.md); date logged below
- [ ] Dry-run date (dd/mm/yyyy): ____________
- [ ] Escalation contacts documented off-repo

---

## 4. Binding EU Declaration of Conformity + CE authorization

| Field | Value |
|-------|-------|
| Plan todo id | `legal-doc-ce-signature` |
| Status | `not_started` (engineering may draft text later under technical-file/; **no binding signature in git**) |
| Depends on | Technical file complete; conformity assessment passed (standards and/or notified body) |
| Hard deadline context | Before **11 December 2027** market placements / substantial modifications under full CRA product obligations |
| Outcome required | Signed DoC retained; CE affixed per guidance; signatory authorization on record |
| Off-repo evidence pointer | _(fill: signed PDF vault path only)_ |
| Done when | Signatory signed; CE process authorized; public claims match signed scope |

### Checklist

- [ ] Annex VII technical file assembled (Phase 3 engineering)
- [ ] Conformity assessment evidence complete for chosen path
- [ ] DoC draft reviewed by counsel (engineering draft is not enough)
- [ ] Authorized manufacturer signatory identified
- [ ] Binding signature executed (**off-repo**; do not commit scanned secrets or unnecessary PII)
- [ ] CE marking authorization recorded for covered distribution artifacts
- [ ] Public download / site claims reviewed so they do not overstate conformity before signature

**Cannot be completed by agents:** binding signature and CE authorization require the legal manufacturer.

---

## 5. Insurance + market-surveillance representation

| Field | Value |
|-------|-------|
| Plan todo id | `legal-market-surveillance` |
| Status | `not_started` (checklist scaffolding only) |
| Depends on | Manufacturer entity; Member State / EEA footprint |
| Outcome required | Insurance policy and authorized representative (if required) documented for relevant Member States / EEA |
| Off-repo evidence pointer | _(fill: broker / policy vault ref; representative entity name)_ |
| Done when | Policy and representation (if required) confirmed; contacts available to market-surveillance authorities |

### Checklist

- [ ] Map Union/EEA Member States where CPN is placed or targeted
- [ ] Confirm whether an authorized representative is required for the manufacturer footprint
- [ ] Obtain / renew product liability / cyber-relevant insurance per counsel advice (policy off-repo)
- [ ] Record representative name and address (public business details only) when required
- [ ] Market-surveillance contact path known to PSIRT / manufacturer ops
- [ ] Annual review date set (dd/mm/yyyy): ____________

---

## Agent vs manufacturer boundary

| Work | Who |
|------|-----|
| Checklists, drafts, reminders, evidence assembly | Engineering / agent (when executing) |
| Counsel memo, contracts, ENISA account creation, binding DoC/CE signature, insurance purchase, authorized representative appointment | Manufacturer, counsel, ops only |

## Update log

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Initial five-item legal track scaffold (no binding claims) |
