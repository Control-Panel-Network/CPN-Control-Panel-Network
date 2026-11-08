# CRA verification summary

Living roll-up for plan todo `cra-verification`. Recorded: **08/11/2026**.

**Engineering is not CRA-complete.** Open PRs carry Phase 1b / 2a / 2b-3-4 work. Legal track remains **PENDING HUMAN**. Do not claim CE marking, signed DoC, or full CRA conformity from this folder.

## Open PRs (status)

| PR | Branch | Title | State (08/11/2026) | Role |
|----|--------|-------|--------------------|------|
| [#652](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/652) | `fix/phase1b-github-security` | Clear Phase 1b Dependabot and CodeQL alerts | OPEN | `phase1-github-security` / `verify-1b-github-security` |
| [#653](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/653) | `feat/cra-phase-2b-3-4` | Phase 0/1 CVD, Phase 2b/3 SBOM gate, Phase 2c disclosure site | OPEN | Phase 0/1 paper, 2b SBOM/gate, 2c site, legal scaffold |
| [#654](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/654) | `feat/phase2a-full-panel-audit` | Phase 2a full panel audit trail and Settings UI | OPEN | `phase2-audit-trail` / `verify-2a-audit-trail` |

Re-check with:

```bash
gh pr view 652 --json state,url,statusCheckRollup
gh pr view 653 --json state,url,statusCheckRollup
gh pr view 654 --json state,url,statusCheckRollup
```

## Verify gates

| Plan todo | Evidence | Status this pass |
|-----------|----------|------------------|
| `verify-1b-github-security` | `gh` Dependabot + Code Scanning open counts on `stable`; PR #652 | **Open.** Snapshot 08/11/2026: Dependabot open **1**, Code Scanning open **5** (API). Must reach open=0 on `stable` after #652 merges and re-scan. |
| `verify-2a-audit-trail` | [verify-2a-evidence.md](verify-2a-evidence.md), [audit-coverage.md](audit-coverage.md), PR #654 | **Partial.** `cargo test panel_audit --lib`: **11 passed**; `panel_action_log`: **5 passed** on WSL at `45a93052`. Lab category smoke + export/webhook still pending. |
| `verify-2b-sbom-release` | [verify-2b-evidence.md](verify-2b-evidence.md), PR #653 | **Partial.** Scripts present; `bash -n` exit **0** on SBOM / audit-gate / security-meta scripts. **Live tagged release** (or full release dry-run with assets) still required. |
| `verify-2c-disclosure-site` | `site/security.html`, `docs/SECURITY-ADVISORIES.md`, `docs/security-advisories.json` on #653 | Engineering present on #653; confirm live https://cpn.newstargeted.com/security after deploy/sync. |
| `cra-verification` (E2E) | This file + legal track | **Blocked** until verify-1b/2a/2b (and live 2c) pass. Legal-* may stay pending human. |

## Legal track (cannot be agent-completed)

All five items: **PENDING HUMAN**. Owners and deadlines: [legal-track.md](legal-track.md).

| # | Plan todo | Owner | Deadline |
|---|-----------|-------|----------|
| 1 | `legal-classification` | Counsel + manufacturer | ASAP |
| 2 | `legal-notified-body` | Manufacturer + counsel | Before 11/12/2027 if Class I path needs NB |
| 3 | `legal-enisa-account` | Manufacturer ops / PSIRT | Before 11/09/2026 (urgent if not live) |
| 4 | `legal-doc-ce-signature` | Manufacturer signatory | Before 11/12/2027 covered placements |
| 5 | `legal-market-surveillance` | Manufacturer / counsel | Before first full-obligation Union/EEA placement |

## Remaining gates (ordered)

1. Merge or finish **#652**; confirm Dependabot + Code Scanning open=0 on `stable`.
2. Merge **#654** after CI green; run lab smoke from [verify-2a-evidence.md](verify-2a-evidence.md); attach evidence.
3. Keep **#653**; cut or dry-run a **tagged release** and attach SBOM + `cpn-security-release.json` asset URLs in [verify-2b-evidence.md](verify-2b-evidence.md).
4. Deploy / sync disclosure site; curl https://cpn.newstargeted.com/security (200 + advisories load).
5. Humans advance [legal-track.md](legal-track.md) (classification first).
6. Only then mark plan `cra-verification` completed (legal signatures may remain open under legal-* todos).

## Related files

| File | Purpose |
|------|---------|
| [README.md](README.md) | CRA dashboard |
| [legal-track.md](legal-track.md) | Five legal items |
| [verify-2a-evidence.md](verify-2a-evidence.md) | Audit trail verify evidence |
| [verify-2b-evidence.md](verify-2b-evidence.md) | SBOM / release verify evidence |
| [audit-coverage.md](audit-coverage.md) | Audit coverage matrix |
| [release-audit-gate.md](release-audit-gate.md) | Release CVE gate notes |

## Update log

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Initial verification summary for PRs 652/653/654 and remaining gates |
