# Security Policy

Coordinated vulnerability disclosure (CVD) for **CPN Control Panel Network**. This policy supports Cyber Resilience Act (CRA) vulnerability-handling expectations. It is **not** a CE mark, EU Declaration of Conformity, or claim that CPN is fully CRA-conformant.

CRA engineering scaffolds and legal checklists: [to-do/cra/README.md](to-do/cra/README.md).

## PSIRT contact

**Product Security Incident Response (PSIRT)** for CPN:

| Channel | Use |
|---------|-----|
| **Email** | `security@newstargeted.com` (preferred PSIRT mailbox; confirm delivery if you do not receive an acknowledgement) |
| **GitHub Private Vulnerability Reporting** | Repository **Security → Advisories → Report a vulnerability** (preferred for structured private reports when enabled) |
| **Maintainer escalation** (if mailbox/GH path fails) | [@master3395](https://github.com/master3395), [@KraoESPfan1n](https://github.com/KraoESPfan1n) |

**Do not** open a public GitHub issue or public pull request for security-sensitive findings.

Please include:

- Affected version, commit, or package.
- How CPN was run (RPM, DEB, raw binary, container, or Windows Phase A).
- Steps to reproduce (no public exploit dump required for triage).
- Impact and required attacker access.
- Whether you believe the issue is **actively exploited** (triggers CRA Art. 14 process for the manufacturer; see [to-do/cra/reporting-playbook.md](to-do/cra/reporting-playbook.md)).

## Severity and response SLAs

Severities use impact on confidentiality, integrity, and availability for a typical privileged CPN install. Timelines are **targets** from PSIRT acknowledgement, not contractual guarantees.

| Severity | Examples (illustrative) | Initial response target | Remediation target |
|----------|-------------------------|-------------------------|--------------------|
| **Critical** | Remote unauthenticated RCE; auth bypass to full panel as admin; release signing-key compromise | **1 business day** | Fix or mitigating guidance as soon as practical; prefer emergency/security release |
| **High** | Authenticated privilege escalation; stored XSS in admin with realistic exploit path; secret disclosure of MFA/API material | **2 business days** | Security fix in a supported release without undue delay |
| **Medium** | Limited auth weaknesses; CSRF on sensitive state change; dependency CVE with constrained attack path | **5 business days** | Fix in normal release cadence for supported versions |
| **Low** | Defense-in-depth gaps; low-impact info disclosure; hardening suggestions | **7 calendar days** | Backlog; may ship with feature releases |

If a report describes **active exploitation** or a **severe security incident**, manufacturer ops follow the Art. 14 playbook (24h early warning / 72h main notification via ENISA) in parallel with CVD. That process is ops/legal; reporters still use the PSIRT channels above.

## Supported versions (security support period)

CPN provides security updates for the **latest two published releases** only (current GitHub Release plus the previous release with installable assets). Production guidance remains the `stable` line / latest release.

| Version set | Security support |
|-------------|------------------|
| Latest two published releases | **Yes** (security fixes applied and released when practical) |
| Older releases | **Outside support** (may still download for labs/rollback; upgrade required for security fixes) |
| `dev` branch commits | Not a security-supported production channel |

**Calendar end-date guidance:** When a release falls out of the latest-two set, treat its security support as ended in the **calendar month** (`mm/yyyy`) it was displaced by the second newer release, unless a more specific end date is published for that line. Full policy: [to-do/cra/support-period.md](to-do/cra/support-period.md). OS guest tiers (Supported / Partial / Refused) are separate: [docs/SUPPORT.md](docs/SUPPORT.md).

Prefer a test host for first installs. Review [docs/SUPPORT.md](docs/SUPPORT.md) before important deployments.

## Published advisories

Public disclosure log for **fixed** vulnerabilities (affected versions, severity, remediation):

- Site: [https://cpn.newstargeted.com/security](https://cpn.newstargeted.com/security)
- Machine-readable: [docs/security-advisories.json](docs/security-advisories.json)
- Human-readable: [docs/SECURITY-ADVISORIES.md](docs/SECURITY-ADVISORIES.md)
- GitHub Security Advisories: [security/advisories](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/security/advisories)

No inventing of CVE or GHSA entries. Publish only after a fix ships and operators have a reasonable upgrade path. An empty advisories list means no published CPN advisory entries yet; report new issues privately via PSIRT. Do not treat changelog blurbs alone as the durable CVE/advisory log.

## Threat model (summary)

CPN is a privileged web installer that can install and configure system packages and services. Linux package paths currently cover supported/partial Enterprise Linux 8 to 10 targets, Ubuntu 22.04/24.04/26.04, and Debian 12/13; the exact support tiers are maintained in [README.md](README.md) and [docs/SUPPORT.md](docs/SUPPORT.md). Windows Server 2016+ is a limited Phase A path and does not have Linux package parity.

A single Rust process can run either the web installer UI (progress over WebSockets) or an interactive SSH/CLI wizard (`--cli`). Both paths perform privileged installation actions. The web UI defaults to `127.0.0.1:2087` and prints a temporary access token in the console URL. Use SSH port forwarding for remote browser access. `--allow-remote` / `CPN_ALLOW_REMOTE=1` binds to `0.0.0.0` and is an explicit operator opt-in to HTTP exposure without TLS. Prefer `--cli` over SSH when you want a terminal-only install without opening the temporary HTTP UI.

### Trust assumptions

- The operator already has root or equivalent administrative access on the install host.
- Prefer a dedicated test machine for first installs and major upgrades.
- Anyone who can reach the installer and obtain a valid temporary token can drive privileged install actions.

### In scope

- Leakage or hard-coding of installer tokens, credentials, signing material, or other secrets.
- Missing/weak authentication on installer actions that should require the temporary token.
- Unsafe download, package, repository, signature, or shell handling that expands remote-code-execution risk beyond the operator's intent.
- Package/update behavior that silently replaces operator-owned configuration or security controls.
- Misleading firewall/network exposure of the installer or managed services.
- Panel authentication, MFA/passkey handling, API tokens, and privileged hosting mutations.

### Out of scope (typical)

- Treating pre-existing local root access as a remote exploit by itself.
- A vulnerability entirely inside an unmodified third-party package after installation.
- Production deployment without reviewing support tiers and taking backups.

## Operator guidance

- Download end-user packages from [GitHub Releases](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases), not from untrusted mirrors.
- Do **not** publish the temporary token shown in the installer URL.
- Prefer SSH port forwarding to `--allow-remote`.
- Stop the installer when the session is finished and restrict port `2087`.
- Review existing web/database/mail configuration before using CPN on a non-empty host.
- Verify `SHA256SUMS` and its signature before installing a release artifact as root.
- Stay on a **latest-two** release for security support; enable automatic updates when appropriate.

Official release fingerprint:

```text
FE70B9718F63B10BB70A6F70BECBB7488AE5C3E5
```

The matching public key is stored at `packaging/RPM-GPG-KEY-CPN` and included in tagged release artifacts. See also [docs/RELEASES.md](docs/RELEASES.md).

## Incident response

1. **Triage:** confirm severity and affected versions; keep sensitive details private; start Art. 14 clocks if actively exploited or severe incident.
2. **Mitigate:** revoke/rotate leaked credentials or tokens and shut down exposed installer instances.
3. **Fix:** prepare and review the smallest safe patch on `stable`.
4. **Release:** publish a fixed version with verification artifacts for supported releases.
5. **Disclose:** publish/update the advisory (GHSA + public advisory log when available) after users have a reasonable upgrade path.
6. **Authority reporting:** manufacturer ops complete ENISA filings per [to-do/cra/reporting-playbook.md](to-do/cra/reporting-playbook.md) when Art. 14 applies.

## Maintainer notes

- Require MFA on GitHub accounts with write access.
- Keep protected branches and required checks enabled when collaborators are added.
- Never commit installer tokens, private keys, production credentials, ENISA credentials, or signing secrets.
- Prefer HTTPS repositories and signed native packages. The official `scripts/install.sh` / `scripts/upgrade.sh` one-liners download only HTTPS release assets and refuse install when `SHA256SUMS` (and GPG when `SHA256SUMS.asc` is present) do not verify. A future yum/apt repository remains the long-term goal.
- Keep CRA scaffolds under `to-do/cra/` current when security process changes.
