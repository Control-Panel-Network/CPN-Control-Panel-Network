# Annex VII technical documentation outline (draft)

Mapped for Regulation (EU) 2024/2847 Annex VII style structure. Fill evidence links as engineering and legal tracks progress. Dates use `dd/mm/yyyy`.

## 1. General description of the product

- Trade name: CPN (Control Panel Network)
- Versions: GitHub Release semver tags (`vMAJOR.MINOR.PATCH`[+suffix])
- Intended purpose: privileged hosting control panel and installer
- Classification working assumption: Important Class I ([../classification.md](../classification.md))
- Manufacturer identity: see [../README.md](../README.md) manufacturer table (confirm with counsel)

## 2. Design and development documentation

| Topic | Evidence pointer |
|-------|------------------|
| Architecture / threat model | [../risk-assessment.md](../risk-assessment.md) |
| Secure-by-default claims | [../../docs/USER-SECURITY.md](../../docs/USER-SECURITY.md), installer defaults |
| Source control | GitHub `stable` / `dev` branches |
| Change control | PRs, [../../docs/CHANGELOG.md](../../docs/CHANGELOG.md) |

## 3. Risk assessment (Art. 13)

- Living file: [../risk-assessment.md](../risk-assessment.md)
- Annex I mapping: [../annex-I-evidence-matrix.md](../annex-I-evidence-matrix.md)

## 4. Essential cybersecurity requirements (Annex I)

Track coverage and residual gaps in the evidence matrix. Do not claim full conformity until gaps are closed and legal assessment agrees.

## 5. Vulnerability handling

| Topic | Evidence pointer |
|-------|------------------|
| CVD policy | [../../SECURITY.md](../../SECURITY.md) |
| Art. 14 playbook | [../reporting-playbook.md](../reporting-playbook.md) |
| Public fixed-vuln log | `docs/security-advisories.json` / site `/security` (Phase 2c) |
| Support period | [../support-period.md](../support-period.md), [../../docs/SUPPORT.md](../../docs/SUPPORT.md) |

## 6. SBOM and supply chain

| Artifact | How produced |
|----------|----------------|
| Cargo SBOM | `cpn-installer.cdx.json` via `scripts/generate-release-sboms.sh` |
| installer-ui npm SBOM | `cpn-installer-ui.cdx.json` |
| Panel npm SBOM | `cpn-panel.cdx.json` |
| Release audit gate | [../release-audit-gate.md](../release-audit-gate.md) |
| Signed distribution | [../../docs/RELEASES.md](../../docs/RELEASES.md) |

## 7. Testing and verification

- CI: `.github/workflows/ci.yml`, CodeQL, Dependabot
- Release provenance: attestations + GPG
- Audit trail verification: Phase 2a ([../audit-coverage.md](../audit-coverage.md))
- CRA verify gates: plan todos `verify-1b`, `verify-2a`, `verify-2b`, `verify-2c`

## 8. Information and instructions to users

- [../../docs/USER-SECURITY.md](../../docs/USER-SECURITY.md)
- Secure deletion / uninstall: USER-SECURITY section 7 + [../../docs/CLI.md](../../docs/CLI.md)

## 9. Conformity assessment

- Harmonised standards tracker: [harmonised-standards.md](harmonised-standards.md)
- Notified body: legal track ([../legal-track.md](../legal-track.md))
- Draft DoC: [declaration-of-conformity-draft.md](declaration-of-conformity-draft.md) (**unsigned**)
- CE notes: [ce-marking-notes.md](ce-marking-notes.md) (**not affixed by this scaffold**)

## 10. Retention

Retain the completed technical file, signed DoC (when executed), and market-surveillance contacts for the period required by the CRA and counsel guidance. Operational retention reminders: [../operate-runbook.md](../operate-runbook.md).
