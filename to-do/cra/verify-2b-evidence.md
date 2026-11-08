# Verify 2b evidence (Phase 2b product / supply-chain)

Recorded: **08/11/2026** (engineering). Plan todo: `verify-2b-sbom-release`. PR: [#653](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/653) (`feat/cra-phase-2b-3-4`).

**Verdict this pass:** scripts and docs are present and syntax-checked. A **live tagged GitHub Release** (or full release-workflow dry-run that uploads assets) is still required before marking `verify-2b-sbom-release` completed.

## Expected release artifacts (from `.github/workflows/release.yml`)

| Artifact | Source |
|----------|--------|
| `cpn-installer.cdx.json` | Cargo via `scripts/generate-release-sboms.sh` |
| `cpn-installer-ui.cdx.json` | installer-ui npm CycloneDX |
| `cpn-panel.cdx.json` | Panel npm CycloneDX |
| `cpn-security-release.json` | `scripts/write-security-release-meta.sh` |
| `CPN-SECURITY-RELEASE` | Marker asset only when `security_release=true` |
| Attached on GitHub Release | `softprops/action-gh-release` `files: dist/**` |

## Audit gates

| Check | Where |
|-------|-------|
| cargo + npm critical/high fail unless waived | `scripts/cra-release-audit-gate.sh` on release job |
| Waiver file | `to-do/cra/audit-waivers.json` (empty by default) |
| Notes | `to-do/cra/release-audit-gate.md` |
| CI syntax | `bash -n` includes the three new scripts in `.github/workflows/ci.yml` |

## Docs

| Doc | Status |
|-----|--------|
| `docs/USER-SECURITY.md` | Present; Annex II style; secure deletion section 7 |
| README Docs table | Links USER-SECURITY |
| `docs/RELEASES.md` / `docs/SUPPORT.md` / `docs/CLI.md` | Cross-links |
| `docs/SUPPORT.md` + support-period | Present; calendar end-date guidance in `to-do/cra/support-period.md` |

## Auto-update metadata preference

| Piece | Status |
|-------|--------|
| Body marker `CPN-SECURITY-RELEASE:` | Parsed in `security_release_flag_from_body` |
| Asset `cpn-security-release.json` | Fetched in `resolve_security_release` |
| Marker asset `CPN-SECURITY-RELEASE` | Sync/async true |
| Heuristics | Fallback only |
| Consumer | `auto_upgrade_run.rs` uses `resolve_security_release` |

## Local validation run this pass (08/11/2026)

### Script syntax (`bash -n`)

Ran from repo root under WSL/bash:

```bash
bash -n scripts/generate-release-sboms.sh
bash -n scripts/cra-release-audit-gate.sh
bash -n scripts/write-security-release-meta.sh
```

**Result:** exit code **0** (all three clean). No live SBOM files were generated in this pass (that still needs the release job or a controlled dry-run with Node/Cargo toolchains matching CI).

### Other local checks (prior / same day)

- `python` JSON load: `to-do/cra/audit-waivers.json` OK
- `PyYAML` safe_load: `.github/workflows/release.yml` and `ci.yml` OK
- No em dash / en dash in new CRA docs under `to-do/cra/` and `docs/USER-SECURITY.md`
- File presence verified for scripts, USER-SECURITY, technical-file drafts, operate-runbook

## Dry-run path (operator; not executed here)

Preferred evidence for closing the verify todo:

1. Trigger `.github/workflows/release.yml` on a pre-release / draft tag **or** a workflow_dispatch dry-run if available.
2. Confirm release assets include:
   - `cpn-installer.cdx.json`
   - `cpn-installer-ui.cdx.json`
   - `cpn-panel.cdx.json`
   - `cpn-security-release.json`
3. Confirm CRA audit gate step passes (or waivers documented in [risk-assessment.md](risk-assessment.md)).
4. Attach CI run URL and release URL under Update log.

**Blocked this pass:** no new tagged release was cut (out of scope for scaffolding). Live tagged release still needed.

## Still required for marking `verify-2b-sbom-release` completed

1. Run release workflow dry-run (or tagged release) and confirm the three SBOM files plus `cpn-security-release.json` appear as artifacts / release assets.
2. Confirm the CRA audit gate step passes (or waivers are documented in risk-assessment).
3. Keep `docs/USER-SECURITY.md` and support-period docs linked from product docs (already present on this PR).

## Update log

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Initial verify-2b evidence scaffold |
| 08/11/2026 | Confirmed `bash -n` exit 0 on SBOM + audit-gate + security-meta scripts; live release still pending |
