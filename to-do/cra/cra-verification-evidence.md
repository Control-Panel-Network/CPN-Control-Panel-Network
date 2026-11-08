# CRA verification: engineering evidence note

Plan todo: `cra-verification`. Recorded: **08/11/2026**.  
Repo tip: `stable` @ `e62d706d` (includes merges of #652, #653, #654).

This note splits what engineering can close from what must stay open for humans or sibling verify agents. It does **not** claim CRA conformity, CE marking, or a signed EU Declaration of Conformity.

**Sibling updates (08/11/2026):** live https://cpn.newstargeted.com/security curls **200** (site sync done). Local CycloneDX + CRA audit gate recorded in [verify-2b-evidence.md](verify-2b-evidence.md). Lab HTTP smoke for remaining audit categories and a **tagged release after #653** remain open.

---

## DONE (engineering)

| Item | Evidence | Plan todo(s) |
|------|----------|--------------|
| Phase 1b Dependabot + CodeQL clear | PR [#652](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/652) MERGED (`e3d7d8a6`) | `phase1-github-security` |
| Dependabot open = 0 | `GET .../dependabot/alerts?state=open` empty `[]` on 08/11/2026 | `verify-1b-github-security` |
| Code Scanning open = 0 | `GET .../code-scanning/alerts?state=open` length 0 on 08/11/2026 | `verify-1b-github-security` |
| Phase 0/1 CVD, support period, risk/reporting scaffolds | PR [#653](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/653) MERGED (`bb27b13d`); `to-do/cra/*`, `SECURITY.md` | `phase0-classify`, `phase1-reporting` |
| Phase 2b SBOM scripts + release audit gate + USER-SECURITY | #653; `scripts/generate-release-sboms.sh`, `cra-release-audit-gate.sh`, `write-security-release-meta.sh`; `docs/USER-SECURITY.md` | `phase2-product` |
| Phase 2c disclosure site sources | #653; `site/security.html`, `docs/SECURITY-ADVISORIES.md`, `docs/security-advisories.json` | `phase2-cve-log` |
| Phase 2a full panel audit trail | PR [#654](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/654) MERGED (`e62d706d`); unit tests 11 `panel_audit` + 5 `panel_action_log` | `phase2-audit-trail` |
| Phase 3 Annex VII / DoC / CE **drafts** (unsigned) | `to-do/cra/technical-file/` | `phase3-conformity` (engineering draft only) |
| Phase 4 operate runbook | `to-do/cra/operate-runbook.md` | `phase4-operate` (engineering draft only) |
| Legal track actionable checklists | [legal-track.md](legal-track.md) with **BLOCKED-ON-HUMAN** per item | scaffolding for all `legal-*` |
| Verification dashboard updated | [verification-summary.md](verification-summary.md), [README.md](README.md) | `cra-verification` (this note) |

## WAITING (sibling verify agents)

Do **not** duplicate these in this pass.

| Gate | Owner | What is still needed | Plan todo |
|------|-------|----------------------|-----------|
| Lab audit category smoke + export/opt-out/alerts | Sibling **verify-2a** | Category mapping fixes + re-smoke; see [verify-2a-evidence.md](verify-2a-evidence.md) | `verify-2a-audit-trail` |
| Tagged release with SBOM + security-meta assets | Sibling **verify-2b** | Local CycloneDX + audit gate **DONE**; still need GitHub Release asset URLs after a post-#653 tag | `verify-2b-sbom-release` |
| Live `/security` HTTP confirm | Sibling **deploy / verify-2c** | **DONE** (200 + JSON 200 + empty state); see [verification-summary.md](verification-summary.md) | `verify-2c-disclosure-site` (live half) |

Until verify-2a finishes and a post-#653 tagged release closes verify-2b assets, plan `cra-verification` stays **partial**.

## HUMAN-REQUIRED (must stay open)

All five legal/manufacturer items. Status: **BLOCKED-ON-HUMAN**. Full checklists: [legal-track.md](legal-track.md).

| Plan todo | Owner | Deadline | Why blocked |
|-----------|-------|----------|-------------|
| `legal-classification` | Counsel + manufacturer | ASAP (target memo **01/03/2027**) | Needs written counsel Class I vs default memo |
| `legal-notified-body` | Manufacturer + counsel | Before **11/12/2027** if Class I needs NB | Needs signed NANDO engagement; blocked on classification |
| `legal-enisa-account` | Manufacturer ops / PSIRT | Hard **11/09/2026** (overdue/urgent if not live) | Needs real ENISA manufacturer account; no fake ids |
| `legal-doc-ce-signature` | Legal manufacturer signatory | Before **11/12/2027** covered placements | Needs binding DoC signature + CE authorization off-repo |
| `legal-market-surveillance` | Manufacturer / counsel | Before first full-obligation Union/EEA placement | Needs insurance + representation decisions |

Agents must not mark these completed, invent ENISA accounts, or commit signatures.

## Plan todo roll-up (this agent)

| Category | Todos | Action |
|----------|-------|--------|
| Engineering-complete (merged + evidence) | `phase0-classify`, `phase1-reporting`, `phase1-github-security`, `verify-1b-github-security`, `phase2-audit-trail`, `phase2-product`, `phase2-cve-log`, `phase3-conformity` (draft), `phase4-operate` (draft) | Keep completed; do not reopen |
| Stay open for sibling agents | `verify-2a-audit-trail`, residual `verify-2b-sbom-release` (tagged release assets), residual `cra-verification` | Wait for category re-smoke + release tag |
| Live site half closed | `verify-2c-disclosure-site` (live) | Curl 200 recorded 08/11/2026 |
| Stay open for human | `legal-classification`, `legal-notified-body`, `legal-enisa-account`, `legal-doc-ce-signature`, `legal-market-surveillance` | **BLOCKED-ON-HUMAN** |

## Commands used this pass

```bash
gh pr view 652 --json state,mergedAt,mergeCommit
gh pr view 653 --json state,mergedAt,mergeCommit
gh pr view 654 --json state,mergedAt,mergeCommit
gh api "repos/Control-Panel-Network/CPN-Control-Panel-Network/dependabot/alerts?state=open&per_page=1"
gh api "repos/Control-Panel-Network/CPN-Control-Panel-Network/code-scanning/alerts?state=open&per_page=100"
git rev-parse HEAD   # expected e62d706d on stable after #654
```

## Update log

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Final engineering evidence note after #652/#653/#654 merge; alerts open=0; legal BLOCKED-ON-HUMAN; sibling verify-2a/2b waiting |
| 08/11/2026 | Live `/security` DONE; local SBOM + audit gate recorded; tagged-release asset attachment still open for verify-2b |
