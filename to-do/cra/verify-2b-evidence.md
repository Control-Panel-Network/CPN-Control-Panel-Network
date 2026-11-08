# Verify 2b evidence (Phase 2b product / supply-chain)

Recorded: 08/11/2026 (engineering). Full release dry-run on GitHub Actions was **not** executed in this pass (skip full e2e of other phases). Local checks below.

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

## Auto-update metadata preference

| Piece | Status |
|-------|--------|
| Body marker `CPN-SECURITY-RELEASE:` | Parsed in `security_release_flag_from_body` |
| Asset `cpn-security-release.json` | Fetched in `resolve_security_release` |
| Marker asset `CPN-SECURITY-RELEASE` | Sync/async true |
| Heuristics | Fallback only |
| Consumer | `auto_upgrade_run.rs` uses `resolve_security_release` |

## Local validation run this pass

- `python` JSON load: `to-do/cra/audit-waivers.json` OK
- `PyYAML` safe_load: `.github/workflows/release.yml` and `ci.yml` OK
- No em dash / en dash in new CRA docs under `to-do/cra/` and `docs/USER-SECURITY.md`
- File presence verified for scripts, USER-SECURITY, technical-file drafts, operate-runbook

## Still required for marking `verify-2b-sbom-release` completed in the plan UI

1. Run release workflow dry-run (or tagged release) and confirm the three SBOM files plus `cpn-security-release.json` appear as artifacts / release assets.
2. Confirm the CRA audit gate step passes (or waivers are documented in risk-assessment).
3. Prefer committing promptly so OneDrive does not drop untracked script edits.
