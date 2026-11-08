# Verify 2b evidence (Phase 2b product / supply-chain)

Recorded: **08/11/2026** (engineering). Plan todo: `verify-2b-sbom-release`. PR: [#653](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/pull/653) (merged to `stable` 08/11/2026).

**Verdict this pass:** scripts produce valid CycloneDX JSON locally; CRA audit gate passes on `stable` tip. A **new tagged GitHub Release after #653** is still required before release-asset attachment is proven on GitHub. Cutting a tag was out of scope for this verify pass.

Stable tip checked: `e62d706d` (merge of #654; includes #652/#653/#654).

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

| Check | Where | Result this pass |
|-------|-------|------------------|
| cargo + npm critical/high fail unless waived | `scripts/cra-release-audit-gate.sh` | **PASSED** (exit 0) |
| Waiver file | `to-do/cra/audit-waivers.json` (empty) | 0 waivers loaded |
| Notes | `to-do/cra/release-audit-gate.md` | unchanged |
| CI syntax | `bash -n` on the three scripts | exit **0** |

## Docs

| Doc | Status |
|-----|--------|
| `docs/USER-SECURITY.md` | Present on `stable` |
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

Host: Windows + Git Bash. Repo at `stable` tip after merge of #652/#653/#654. Tooling: `cargo-cyclonedx 0.5.10`, Node `v23.8.0`, `@cyclonedx/cyclonedx-npm@4.0.0`, Python 3.12.7.

### Script syntax (`bash -n`)

```bash
bash -n scripts/generate-release-sboms.sh
bash -n scripts/cra-release-audit-gate.sh
bash -n scripts/write-security-release-meta.sh
```

**Result:** exit code **0**.

### CycloneDX SBOM generation (local dry-run)

Official `scripts/generate-release-sboms.sh` matches Linux CI. On this Windows host the embedded `python3` validator cannot open MSYS paths (`/c/...`), and Panel `npm ls` reports peer/extraneous `ELSPROBLEMS` against `node_modules`. Evidence was produced with the same tools the script wraps:

1. `cargo cyclonedx -f json --override-filename cpn-installer.cdx` then move to out dir
2. `npx @cyclonedx/cyclonedx-npm@4.0.0 --package-lock-only --ignore-npm-errors --spec-version 1.5` for `installer-ui/` and `Panel/`
3. `scripts/write-security-release-meta.sh` with `CPN_SECURITY_RELEASE=false` `CPN_RELEASE_TAG=verify-2b-dry-run`
4. `scripts/cra-release-audit-gate.sh` (exit 0)

**Artifact paths (operator workstation; not committed):**

| File | Path | bomFormat | components | bytes |
|------|------|-----------|------------|-------|
| Cargo SBOM | `C:\Users\kimsk\AppData\Local\Temp\cpn-sbom-out\cpn-installer.cdx.json` | CycloneDX | 240 | 290299 |
| installer-ui SBOM | `C:\Users\kimsk\AppData\Local\Temp\cpn-sbom-out\cpn-installer-ui.cdx.json` | CycloneDX | 118 | 170935 |
| Panel SBOM | `C:\Users\kimsk\AppData\Local\Temp\cpn-sbom-out\cpn-panel.cdx.json` | CycloneDX | 402 | 557572 |
| Security meta | `C:\Users\kimsk\AppData\Local\Temp\cpn-sbom-out\cpn-security-release.json` | n/a | n/a | 664 |

Local copy also mirrored under repo `dist-verify-2b/` for operator convenience (do not commit; large JSON).

JSON load check: all three `*.cdx.json` files parse with `bomFormat == "CycloneDX"`.

### CRA audit gate

```
== CRA release audit gate ==
Loaded 0 waiver id(s)
cargo critical/high unwaived: 0
npm/installer-ui critical/high unwaived: 0
npm/panel critical/high unwaived: 0
CRA release audit gate PASSED
CRA_AUDIT_GATE_EXIT=0
```

## GitHub Release dry-run / tagged release

**Blocker (documented):** no new GitHub Release tag was cut after #653 merged. Latest public release remains `v1.4.9` (07/11/2026), which predates the Phase 2b SBOM/security-meta workflow changes. Attaching `cpn-*.cdx.json` and `cpn-security-release.json` to a live Release therefore still requires an operator-owned tag (or a draft/pre-release) and a green `release.yml` run.

Local script output above is the substitute evidence until that tag exists.

## Still required for marking `verify-2b-sbom-release` completed

1. Cut or dry-run a **tagged release after #653** and confirm the three SBOM files plus `cpn-security-release.json` appear as GitHub Release assets (attach CI run URL + release URL here).
2. Keep CRA audit gate green (or document waivers in [risk-assessment.md](risk-assessment.md)).
3. Keep `docs/USER-SECURITY.md` and support-period docs linked (already on `stable`).

## Update log

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Initial verify-2b evidence scaffold |
| 08/11/2026 | Confirmed `bash -n` exit 0 on SBOM + audit-gate + security-meta scripts; live release still pending |
| 08/11/2026 | Post-merge on `stable`: local CycloneDX JSON for Cargo + installer-ui + Panel; security meta written; CRA audit gate PASSED; tagged release after #653 still blocked (no new tag cut) |
