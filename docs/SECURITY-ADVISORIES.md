# CPN security advisories

Public disclosure log for **fixed** vulnerabilities in CPN Control Panel Network.

This file mirrors [`security-advisories.json`](security-advisories.json). The live site page is [https://cpn.newstargeted.com/security](https://cpn.newstargeted.com/security). GitHub Security Advisories remain the private intake and official advisory objects: [Security advisories](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/security/advisories).

## How to report

**Do not** open a public GitHub issue for unfixed, security-sensitive findings.

1. Prefer [GitHub Private Vulnerability Reporting](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/security/advisories/new) (Security → Advisories → Report a vulnerability).
2. Or contact a maintainer listed in [SECURITY.md](../SECURITY.md).

See [SECURITY.md](../SECURITY.md) for scope, response expectations, and operator guidance.

## Published advisories

No published advisories yet.

When a fix ships, maintainers append an entry to `security-advisories.json` (and this file), publish or update the matching GHSA, and refresh the site `/security` page. Entry fields: `id`, `cve`, `ghsa`, `title`, `severity`, `affected_versions`, `fixed_version`, `published`, `summary`, `impact`, `remediation`, `references`.

### Entry template (for maintainers)

```markdown
### CPN-YYYY-NNN: Title

| Field | Value |
| --- | --- |
| CVE | CVE-YYYY-NNNNN or none |
| GHSA | GHSA-xxxx-xxxx-xxxx or none |
| Severity | critical / high / medium / low |
| Affected | e.g. `< 1.4.10` |
| Fixed in | e.g. `1.4.10` |
| Published | dd/mm/yyyy |

**Summary:** …

**Impact:** …

**Remediation:** Upgrade to … (interim mitigation if any).

**References:** …
```
