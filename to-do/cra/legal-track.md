# Legal and manufacturer track (checklist scaffolding)

**Purpose:** Track the five manufacturer/legal items that engineering cannot complete with code alone.  
**Rule:** Drafts and checklists live here. **Binding signatures, contracts, account credentials, and insurance policies stay off-repo.** Record only status, owners, deadlines, and non-secret pointers.

**All five items are BLOCKED-ON-HUMAN** (counsel / manufacturer / ops). Engineering scaffolding does **not** complete them. Agents must not fake signatures, ENISA accounts, notified-body contracts, or insurance purchase.

Working classification assumption until item 1 completes: **Important Class I** ([classification.md](classification.md)).

## Summary (BLOCKED-ON-HUMAN)

| # | Plan todo | Owner | Deadline | Status |
|---|-----------|-------|----------|--------|
| 1 | `legal-classification` | Counsel + manufacturer (News Targeted / Control-Panel-Network) | ASAP (blocks notified-body spend); target counsel memo by **01/03/2027** | **BLOCKED-ON-HUMAN** |
| 2 | `legal-notified-body` | Manufacturer + counsel | Before **11/12/2027** Class I placements if standards path incomplete | **BLOCKED-ON-HUMAN** (blocked on item 1) |
| 3 | `legal-enisa-account` | Manufacturer ops / PSIRT | Hard: before **11/09/2026** Art. 14 start; as of **08/11/2026** treat as **overdue / urgent** if not already live | **BLOCKED-ON-HUMAN** |
| 4 | `legal-doc-ce-signature` | Legal manufacturer signatory | Before **11/12/2027** covered placements / substantial mods | **BLOCKED-ON-HUMAN** (blocked on tech file + conformity path) |
| 5 | `legal-market-surveillance` | Manufacturer / counsel | Before first CRA-covered Union/EEA placement under full obligations; plan review by **01/06/2027** | **BLOCKED-ON-HUMAN** |

Internal status values used in detail rows: `not_started` | `in_progress` | `blocked` | `done_off_repo` | `cancelled`.  
Until a human marks `done_off_repo` (with off-repo pointer), treat the public status as **BLOCKED-ON-HUMAN**.

**Engineering may only:** keep checklists current, assemble Annex VII drafts, and remind owners. **Engineering may never:** mark these done, invent NANDO ids, invent ENISA account ids, or commit signed DoC/CE scans.

---

## 1. Final legal classification (Class I vs default)

| Field | Value |
|-------|-------|
| Plan todo id | `legal-classification` |
| Status | **BLOCKED-ON-HUMAN** |
| Internal status | `in_progress` (engineering draft exists; counsel memo pending) |
| Owner | Counsel + manufacturer (News Targeted / Control-Panel-Network) |
| Deadline | ASAP (blocks notified-body spend). Suggested internal target: counsel memo filed by **01/03/2027**. |
| Depends on | Product description; Annex III / Implementing Regulation (EU) 2025/2392 mapping |
| Outcome required | Written counsel memo; update [classification.md](classification.md) to "confirmed" or "default" |
| Off-repo evidence pointer | _(fill: vault path or ticket id only)_ |
| Done when | Counsel memo filed; README dashboard + this row updated; notified-body spend decision recorded |
| Why agents cannot close | Binding legal classification requires counsel opinion; engineering Class I draft is assumption only |

### Actionable checklist (human)

- [ ] Manufacturer legal entity and EEA main establishment confirmed in writing
- [ ] Counsel maps CPN against Annex III / 2025/2392 categories
- [ ] Written memo states Class I **or** default (or other justified category)
- [ ] [classification.md](classification.md) updated to match memo (no conflicting "working assumption" language left)
- [ ] If default: item 2 cancelled or narrowed with written reason
- [ ] If Class I: item 2 remains required unless harmonised-standards path alone is accepted by counsel
- [ ] Off-repo pointer filled above (ticket / vault path only)

---

## 2. Notified body contract

| Field | Value |
|-------|-------|
| Plan todo id | `legal-notified-body` |
| Status | **BLOCKED-ON-HUMAN** |
| Internal status | `not_started` (scaffolding only; do not fake a contract) |
| Owner | Manufacturer + counsel |
| Deadline | Before **11/12/2027** Class I placements if standards path incomplete |
| Depends on | Item 1 = Class I (or explicit decision to use Module A with applicable harmonised standards) |
| Outcome required | Signed engagement; assessment schedule; NANDO body id |
| Off-repo evidence pointer | _(fill: contract vault ref / NANDO id only)_ |
| Done when | Signed contract on file off-repo; schedule exists; this checklist complete |
| Why agents cannot close | Contract signature and NANDO engagement are manufacturer/counsel acts; do not invent body ids |

### Actionable checklist (human)

- [ ] Classification memo confirms Class I (or counsel documents standards-only Module A path)
- [ ] Shortlist notified bodies from NANDO for CRA / relevant modules
- [ ] Request quotes / proposals from shortlisted bodies
- [ ] Engagement letter / contract signed by manufacturer (not stored in git)
- [ ] Assessment schedule recorded (milestones toward Dec 2027)
- [ ] NANDO body id recorded in this file (public id only; no credentials)
- [ ] If cancelled: written reason linked from item 1
- [ ] Off-repo pointer filled above

---

## 3. ENISA Single Reporting Platform manufacturer account

| Field | Value |
|-------|-------|
| Plan todo id | `legal-enisa-account` |
| Status | **BLOCKED-ON-HUMAN** |
| Internal status | `not_started` (ops checklist; credentials never in git) |
| Owner | Manufacturer ops / PSIRT |
| Hard deadline | **11 September 2026** (Art. 14 reporting start). As of **08/11/2026**, if the account is not live this item is **overdue / urgent**. |
| Depends on | Manufacturer entity; main-establishment CSIRT routing; live PSIRT process ([reporting-playbook.md](reporting-playbook.md)) |
| Outcome required | Live manufacturer account; credentials in secure ops store; dry-run logged |
| Off-repo evidence pointer | _(fill: password manager / ops vault entry name only)_ |
| Done when | Account live; dry-run logged; PSIRT knows how to access; playbook points to live account |
| Why agents cannot close | Creating the ENISA manufacturer account and storing credentials require real human identity verification; never fake an account id in git |

### Actionable checklist (human)

- [ ] Manufacturer entity and contact persons designated for ENISA reporting
- [ ] CSIRT routing / main establishment confirmed with counsel
- [ ] Confirm ENISA Single Reporting Platform portal availability for manufacturers
- [ ] Manufacturer account created on ENISA Single Reporting Platform (when available)
- [ ] Credentials stored in secure ops vault (**not** git, **not** this file)
- [ ] Access limited to PSIRT / on-call with MFA
- [ ] Tabletop dry-run executed using [reporting-playbook.md](reporting-playbook.md); date logged below
- [ ] Dry-run date (dd/mm/yyyy): ____________
- [ ] Escalation contacts documented off-repo
- [ ] Off-repo pointer filled above

---

## 4. Binding EU Declaration of Conformity + CE authorization

| Field | Value |
|-------|-------|
| Plan todo id | `legal-doc-ce-signature` |
| Status | **BLOCKED-ON-HUMAN** |
| Internal status | `not_started` (engineering may draft text under technical-file/; **no binding signature in git**) |
| Owner | Legal manufacturer signatory |
| Hard deadline context | Before **11 December 2027** market placements / substantial modifications under full CRA product obligations |
| Depends on | Technical file complete; conformity assessment passed (standards and/or notified body) |
| Outcome required | Signed DoC retained; CE affixed per guidance; signatory authorization on record |
| Off-repo evidence pointer | _(fill: signed PDF vault path only)_ |
| Done when | Signatory signed; CE process authorized; public claims match signed scope |
| Why agents cannot close | Binding signature and CE authorization require the legal manufacturer; engineering drafts under `technical-file/` are **not** signed DoCs |

### Actionable checklist (human)

- [ ] Annex VII technical file assembled (Phase 3 engineering drafts exist under [technical-file/](technical-file/README.md))
- [ ] Conformity assessment evidence complete for chosen path
- [ ] DoC draft reviewed by counsel (engineering draft is not enough)
- [ ] Authorized manufacturer signatory identified
- [ ] Binding signature executed (**off-repo**; do not commit scanned secrets or unnecessary PII)
- [ ] CE marking authorization recorded for covered distribution artifacts
- [ ] Public download / site claims reviewed so they do not overstate conformity before signature
- [ ] Off-repo pointer filled above

**Cannot be completed by agents:** binding signature and CE authorization require the legal manufacturer. Do not paste a fake signature block into git.

---

## 5. Insurance + market-surveillance representation

| Field | Value |
|-------|-------|
| Plan todo id | `legal-market-surveillance` |
| Status | **BLOCKED-ON-HUMAN** |
| Internal status | `not_started` (checklist scaffolding only) |
| Owner | Manufacturer / counsel |
| Deadline | Before first CRA-covered Union/EEA placement under full obligations. Suggested internal target: broker / representation plan reviewed by **01/06/2027**. |
| Depends on | Manufacturer entity; Member State / EEA footprint |
| Outcome required | Insurance policy and authorized representative (if required) documented for relevant Member States / EEA |
| Off-repo evidence pointer | _(fill: broker / policy vault ref; representative entity name)_ |
| Done when | Policy and representation (if required) confirmed; contacts available to market-surveillance authorities |
| Why agents cannot close | Insurance purchase and authorized-representative appointment are manufacturer/counsel commercial acts |

### Actionable checklist (human)

- [ ] Map Union/EEA Member States where CPN is placed or targeted
- [ ] Confirm whether an authorized representative is required for the manufacturer footprint
- [ ] Obtain / renew product liability / cyber-relevant insurance per counsel advice (policy off-repo)
- [ ] Record representative name and address (public business details only) when required
- [ ] Market-surveillance contact path known to PSIRT / manufacturer ops
- [ ] Annual review date set (dd/mm/yyyy): ____________
- [ ] Off-repo pointer filled above

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
| 08/11/2026 | Explicit **PENDING HUMAN** markers, owners, and deadlines for all five items |
| 08/11/2026 | Status wording standardized to **BLOCKED-ON-HUMAN**; actionable human checklists; suggested internal targets; explicit "why agents cannot close" on each item |
