# CPN user security instructions (Annex II style)

This document is the operator-facing security information and instructions pack for **CPN Control Panel Network**. It is written to align with CRA Annex II style guidance for users of a product with digital elements. It is **not** an EU Declaration of Conformity, CE marking statement, or legal advice.

Manufacturer working identity, support period, and disclosure channels are summarised in [SECURITY.md](../SECURITY.md), [SUPPORT.md](SUPPORT.md), and [to-do/cra/](../to-do/cra/README.md).

## 1. Product identification

| Field | Value |
|-------|-------|
| Trade name | CPN (Control Panel Network) |
| What it is | Hosting control panel and installer for privileged server management |
| Public site | https://cpn.newstargeted.com |
| Source and releases | https://github.com/Control-Panel-Network/CPN-Control-Panel-Network |
| Vulnerability reporting | [SECURITY.md](../SECURITY.md) and GitHub Private Vulnerability Reporting |
| Published fixed advisories | [SECURITY-ADVISORIES.md](SECURITY-ADVISORIES.md) (when present) and https://cpn.newstargeted.com/security |

## 2. Intended use and environment

CPN is intended for administrators who install and operate hosting control-panel software on supported Linux guests (and Windows Server Phase A where documented). It manages privileged configuration (accounts, sites, firewall, mail, databases, updates).

- Install only on systems you are authorised to administer.
- Prefer a disposable lab host for first installs and major upgrades.
- Keep host OS patches current; CPN does not replace OS vendor security updates.
- Expose the panel on a public hostname only behind TLS (reverse proxy) with MFA enrolled for administrators.

## 3. Secure-by-default expectations

On a normal install, CPN aims to:

- Bind the panel listen address to `127.0.0.1` unless you explicitly allow remote bind.
- Require a strong password policy and reserved-username checks for admin accounts.
- Require administrator MFA (TOTP and/or passkey) before normal dashboard use after first login.
- Keep automatic updates **enabled** by default (Owner may turn them off under Version Management).
- Distribute releases with `SHA256SUMS`, GPG signatures, and GitHub attestations ([RELEASES.md](RELEASES.md)).

Factory-like reset of panel bootstrap data uses installer repair with `--reset-data` (destructive). Full product removal uses uninstall (section 7).

## 4. Security updates and support period

- **Free security updates** are provided during the published support period for the **latest two** releases (see [SUPPORT.md](SUPPORT.md) and [to-do/cra/support-period.md](../to-do/cra/support-period.md)).
- Prefer official GitHub Release packages (RPM/DEB/Windows ZIP). Verify checksums and signatures before installing.
- Automatic updates default **On** with strategy **All newer releases**. Security-only mode prefers machine-readable release metadata (`CPN-SECURITY-RELEASE` body marker and `cpn-security-release.json`) over release title keywords.
- Postpone or disable automatic updates only from Version Management if your change window requires it; you remain responsible for applying security fixes before support end.

## 5. Authentication, authorisation, and audit

- Enrol MFA or a passkey for every administrator account.
- Do not share Owner credentials; use scoped staff accounts where available.
- Review Server logs and Activity Board for unexpected privilege actions.
- A full CRA-aligned audit trail (retention, export, security-essential categories that cannot be disabled, and non-essential opt-out) is tracked under Phase 2a (`to-do/cra/audit-coverage.md`). Until that work lands, treat the existing panel action log and OS logs as partial evidence only.

## 6. Confidentiality and integrity of updates

- Always verify `SHA256SUMS` (and GPG when enabled) for release artifacts.
- Do not run unsigned or checksum-mismatched installer binaries as root.
- Release CycloneDX SBOMs (`cpn-installer.cdx.json`, `cpn-installer-ui.cdx.json`, `cpn-panel.cdx.json`) are attached to GitHub Releases for supply-chain review.

## 7. Secure permanent data deletion

Operators may permanently remove CPN-held data using documented uninstall and purge paths. **System Repair** (`cpn doctor` / Server > System Repair) diagnoses and heals services; it does **not** wipe MFA keys or perform full product deletion.

### Product uninstall (leave CPN)

```bash
sudo cpn-installer --uninstall        # interactive confirm
sudo cpn uninstall --yes              # same engine, non-interactive
sudo cpn-installer --uninstall --yes --dry-run
```

Default uninstall removes the panel package and CPN state under `/var/lib/cpn` and `/etc/cpn` (unless `--keep-data`), and **keeps** customer website homes and host MariaDB / OpenLiteSpeed packages.

| Flag | Effect |
|------|--------|
| `--keep-data` | Leave `/var/lib/cpn` and `/etc/cpn` |
| `--purge-all` | Also remove CPN Docker volumes and webmail trees under `/var/lib/cpn-webmail` and `/opt/cpn-webmail` |
| `--purge-sites` | Delete registered website homes (second confirmation; never silent) |
| `--purge-stack` | Attempt to remove common MariaDB / OpenLiteSpeed packages |
| `--dry-run` | Print steps only |

Destructive panel bootstrap wipe without full uninstall:

```bash
sudo cpn-installer --repair --reset-data
```

Details: [CLI.md](CLI.md), [INSTALL.md](INSTALL.md). After uninstall, verify that `/var/lib/cpn` is gone (unless `--keep-data`) and rotate any credentials that were shared outside CPN.

## 8. Reporting vulnerabilities and severe incidents

- Coordinated disclosure: [SECURITY.md](../SECURITY.md).
- Manufacturer Art. 14 reporting playbook (ENISA Single Reporting Platform): [to-do/cra/reporting-playbook.md](../to-do/cra/reporting-playbook.md).
- Operators who believe a vulnerability is being actively exploited should contact the PSIRT channel immediately and preserve logs without circulating exploit details publicly.

## 9. Related documents

| Doc | Topic |
|-----|-------|
| [SECURITY.md](../SECURITY.md) | CVD policy |
| [SUPPORT.md](SUPPORT.md) | OS tiers and support period |
| [RELEASES.md](RELEASES.md) | Packages, SBOM, signatures |
| [CLI.md](CLI.md) | Uninstall, doctor, repair |
| [to-do/cra/README.md](../to-do/cra/README.md) | CRA engineering status |

Last reviewed: 08/11/2026 (engineering draft).
