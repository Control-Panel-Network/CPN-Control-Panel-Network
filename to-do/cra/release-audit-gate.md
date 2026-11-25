# CRA release audit gate notes (Phase 2b)

## Purpose

Fail tagged / release workflow builds when **unwaived critical or high** findings appear in:

- `cargo audit` (Rust / installer crate graph)
- `npm audit --omit=dev` for `installer-ui/` and `Panel/`

Script: [`scripts/cra-release-audit-gate.sh`](../../scripts/cra-release-audit-gate.sh)  
Wired from [`.github/workflows/release.yml`](../../.github/workflows/release.yml).

CI (`.github/workflows/ci.yml`) still runs `cargo audit` strictly and npm audit with `continue-on-error` for transient registry noise. The **release** gate is the hard CRA supply-chain check.

## How to run locally

```bash
chmod +x scripts/cra-release-audit-gate.sh
./scripts/cra-release-audit-gate.sh
```

Optional: `CPN_AUDIT_GATE_SKIP_NPM=1` for cargo-only debugging. Do not skip npm for official releases.

## Waivers

File: [`audit-waivers.json`](audit-waivers.json)

Add an entry only when:

1. The finding is accepted with a written line in [`risk-assessment.md`](risk-assessment.md) (or an explicit CRA risk note), and
2. There is no practical fixed dependency yet, or the advisory does not affect the shipped CPN attack surface.

Example shape (do not invent live advisories here):

```json
{
  "schema_version": 1,
  "waivers": [
    {
      "id": "RUSTSEC-YYYY-NNNN",
      "reason": "Short non-secret justification",
      "risk_assessment_ref": "risk-assessment.md#example-anchor",
      "expires": "2026-12-31"
    }
  ]
}
```

Ids are matched case-insensitively (`RUSTSEC-…`, `GHSA-…`, `CVE-…`). Empty `waivers` means zero exceptions.

## Related artifacts

- CycloneDX SBOMs on each release: `cpn-installer.cdx.json`, `cpn-installer-ui.cdx.json`, `cpn-panel.cdx.json` ([`scripts/generate-release-sboms.sh`](../../scripts/generate-release-sboms.sh))
- Security release metadata: `cpn-security-release.json` ([`scripts/write-security-release-meta.sh`](../../scripts/write-security-release-meta.sh))
