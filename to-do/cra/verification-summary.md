# CRA verification summary

Living roll-up for plan todo `cra-verification`. Updated: **08/11/2026**.

**Engineering foundations for Phases 0 to 4 are merged to `stable` via PRs #652, #653, and #654.** GitHub Dependabot + Code Scanning open counts are **0**. Live `/security` deploy and lab HTTP smoke are owned by sibling agents (not re-run here). Legal track remains **BLOCKED-ON-HUMAN**. Do not claim CE marking, signed DoC, or full CRA conformity from this folder.

Final engineering vs human split: [cra-verification-evidence.md](cra-verification-evidence.md).

## Merged PRs (status)

| PR | Branch | Title | State (08/11/2026) | Merge commit | Role |
|----|--------|-------|--------------------|--------------|------|
| [#652](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/652) | `fix/phase1b-github-security` | Clear Phase 1b Dependabot and CodeQL alerts | **MERGED** | `e3d7d8a6` | `phase1-github-security` / `verify-1b-github-security` |
| [#653](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/653) | `feat/cra-phase-2b-3-4` | Phase 0/1 CVD, Phase 2b/3 SBOM gate, Phase 2c disclosure site | **MERGED** | `bb27b13d` | Phase 0/1 paper, 2b SBOM/gate, 2c site, legal scaffold |
| [#654](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/654) | `feat/phase2a-full-panel-audit` | Phase 2a full panel audit trail and Settings UI | **MERGED** | `e62d706d` | `phase2-audit-trail` / `verify-2a-audit-trail` |

`stable` tip used for lab smoke: `5ccfa94c` (docs after Merge #654 `e62d706d`). Re-check:

```bash
gh pr view 652 --json state,mergedAt,mergeCommit
gh pr view 653 --json state,mergedAt,mergeCommit
gh pr view 654 --json state,mergedAt,mergeCommit
gh api "repos/Control-Panel-Network/CPN-Control-Panel-Network/dependabot/alerts?state=open&per_page=1"
gh api "repos/Control-Panel-Network/CPN-Control-Panel-Network/code-scanning/alerts?state=open&per_page=100"
```

## GitHub security alerts (stable)

| Source | Open count (08/11/2026) | Notes |
|--------|-------------------------|-------|
| Dependabot | **0** | `GET .../dependabot/alerts?state=open` returned empty `[]` |
| Code Scanning | **0** | `GET .../code-scanning/alerts?state=open` length **0** |

`verify-1b-github-security` engineering gate: **DONE** (open=0 after #652 merge). Ongoing release gates still apply via `scripts/cra-release-audit-gate.sh`.

## Verify gates

| Plan todo | Evidence | Status this pass |
|-----------|----------|------------------|
| `verify-1b-github-security` | `gh` Dependabot + Code Scanning open=0 on `stable` after #652 | **DONE** (engineering) |
| `verify-2a-audit-trail` | [verify-2a-evidence.md](verify-2a-evidence.md), [audit-coverage.md](audit-coverage.md), #654 merged | **BLOCKED after practical lab completion.** AL10 `2229` tip `5ccfa94` / 1.4.9: logout, CLI doctor heal, webhook delivery, auth-fail spike, policy lock, export, and safe real-route mutations exercised. Database, DNS, update-check, notification, and UI-preference routes are audited as `category=other`; real UI opt-out therefore fails. Code mapping fixes and a re-smoke are required. |
| `verify-2b-sbom-release` | [verify-2b-evidence.md](verify-2b-evidence.md), #653 merged | **WAITING sibling verify-2b** for live tagged release (or dry-run) with SBOM + `cpn-security-release.json` assets. Scripts `bash -n` OK. |
| `verify-2c-disclosure-site` | `site/security.html`, advisories JSON/MD on #653 | Engineering **DONE** in tree. Live https://cpn.newstargeted.com/security owned by sibling deploy agent (not re-checked here). |
| `cra-verification` (E2E) | This file + [cra-verification-evidence.md](cra-verification-evidence.md) | **PARTIAL.** Engineering merge + alerts=0 recorded. Blocked on sibling verify-2a/2b completion and human legal track. |

## Legal track (cannot be agent-completed)

All five items: **BLOCKED-ON-HUMAN**. Owners and deadlines: [legal-track.md](legal-track.md).

| # | Plan todo | Owner | Deadline |
|---|-----------|-------|----------|
| 1 | `legal-classification` | Counsel + manufacturer | ASAP (target memo **01/03/2027**) |
| 2 | `legal-notified-body` | Manufacturer + counsel | Before 11/12/2027 if Class I path needs NB |
| 3 | `legal-enisa-account` | Manufacturer ops / PSIRT | Before 11/09/2026 (overdue/urgent if not live) |
| 4 | `legal-doc-ce-signature` | Manufacturer signatory | Before 11/12/2027 covered placements |
| 5 | `legal-market-surveillance` | Manufacturer / counsel | Before first full-obligation Union/EEA placement |

## Remaining gates (ordered)

1. Fix **verify-2a** real-route category mappings for database/FTP, `/server/dns`, auto-upgrade/version, panel notifications, and UI preferences; then re-smoke the UI opt-out. Logout, CLI, webhook, spike alert, export, and policy-lock checks already pass.
2. Sibling **verify-2b**: tagged release or dry-run; attach SBOM + security-meta asset URLs in [verify-2b-evidence.md](verify-2b-evidence.md).
3. Sibling **deploy**: confirm live https://cpn.newstargeted.com/security (not owned by this pass).
4. Humans advance [legal-track.md](legal-track.md) (classification first; ENISA account urgent).
5. Mark plan `cra-verification` completed only when engineering verify siblings finish; keep legal-* open until humans close them.

## Related files

| File | Purpose |
|------|---------|
| [README.md](README.md) | CRA dashboard |
| [cra-verification-evidence.md](cra-verification-evidence.md) | Final DONE vs HUMAN-REQUIRED vs sibling-waiting note |
| [legal-track.md](legal-track.md) | Five legal items (BLOCKED-ON-HUMAN) |
| [verify-2a-evidence.md](verify-2a-evidence.md) | Audit trail verify evidence |
| [verify-2b-evidence.md](verify-2b-evidence.md) | SBOM / release verify evidence |
| [audit-coverage.md](audit-coverage.md) | Audit coverage matrix |
| [release-audit-gate.md](release-audit-gate.md) | Release CVE gate notes |

## Update log

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Initial verification summary for PRs 652/653/654 and remaining gates |
| 08/11/2026 | PRs 652/653/654 **MERGED** to `stable`; Dependabot open=0; Code Scanning open=0; sibling verify-2a/2b still waiting; link cra-verification-evidence |
| 08/11/2026 | verify-2a AL10 lab partial smoke recorded (auth/settings/firewall + export); remaining checklist still open |
| 08/11/2026 | verify-2a practical close-out run completed; logout/CLI/webhook/spike/policy checks passed; real-route category and UI opt-out defects now block completion |
