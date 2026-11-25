# CPN CRA product classification (draft)

**Status:** Engineering working assumption. **Not** a final legal classification memo.  
**Owner for final decision:** Manufacturer + counsel.  
**Related checklist:** [legal-track.md](legal-track.md) item 1.

## Working assumption

Treat CPN as an **Important product with digital elements, Class I**, under Regulation (EU) 2024/2847 and Implementing Regulation (EU) 2025/2392, pending written counsel confirmation.

### Why Class I is the working assumption

1. **Network management systems (Annex III / 2025/2392 mapping):** CPN monitors and controls connected network elements (servers and hosting services) by applying configuration, service control, firewall, DNS, SSL, updates, and related privileged operations.
2. **Identity / privileged access management (adjacent):** CPN authenticates operators, enforces MFA/passkeys, issues API tokens, and gates privileged hosting actions.

Until counsel confirms otherwise, conformity planning **must not** assume Module A (internal production control) alone is enough. Class I typically requires applicable **harmonised standards** and/or a **notified body** conformity assessment path.

## Facts already treated as settled for engineering

| Question | Engineering answer |
|----------|-------------------|
| Product with digital elements? | Yes (installer + control panel software) |
| Placed on the Union/EEA market commercially as manufacturer? | Yes for News Targeted / Control-Panel-Network commercial distribution (open-source licence does not remove manufacturer duties when monetised or commercially placed) |
| Open-source steward light-touch (Art. 24) applicable? | **No** under the working commercial-manufacturer posture |
| Trade name | CPN (Control Panel Network) |
| Version scheme | Semver tags on GitHub Releases (for example `v1.4.9`); panel also tracks `stable` / `dev` commits |

## Classification decision tree (for counsel)

1. Confirm product with digital elements: **yes**.
2. Confirm commercial placement as manufacturer in the Union/EEA: **yes** (confirm entity, address, and EEA main establishment for CSIRT routing).
3. Map against Annex III categories / Implementing Regulation (EU) 2025/2392.
4. If **Important Class I:** plan harmonised standards and/or notified body; keep building the Annex VII technical file either way.
5. If counsel confirms **default category only:** cancel or narrow notified-body spend; keep Module A + technical file.
6. Either path still requires Art. 14 reporting readiness, support-period transparency, CVD, and (from Dec 2027) essential requirements for covered placements / substantial modifications.

## Outcomes depending on counsel memo

| Counsel result | Engineering / legal follow-up |
|----------------|-------------------------------|
| Confirms Important Class I | Keep Class I track; engage notified body if standards path incomplete ([legal-track.md](legal-track.md) item 2) |
| Confirms default category | Update this file and the README dashboard; cancel or narrow notified-body todo; keep Module A evidence |
| Ambiguous / mixed components | Split product variants only if counsel recommends; do not invent a split in engineering copy |

## Questions for counsel (do not answer in git as legal fact)

- Exact manufacturer legal entity name, registered address, and EEA main establishment for CSIRT / Art. 14 routing.
- Whether installer-only artifacts, Docker images, and panel packages are one product or multiple for DoC purposes.
- Whether free GitHub distribution plus paid hosting/services still counts as placing the product on the market as manufacturer for every channel.
- Preferred conformity route: harmonised standards (when published) vs notified body vs both.
- Language and retention for the final classification memo (off-repo vault path to record in legal-track.md only as a pointer).

## Evidence to attach to the counsel memo (engineering can supply)

- Product description: README, https://cpn.newstargeted.com, privileged capabilities list from risk assessment.
- Distribution channels: GitHub Releases, `cpn.newstargeted.com` install/upgrade scripts, Docker Hub tags for `master3395/cpn-installer` when used commercially.
- Security posture summary: [SECURITY.md](../../SECURITY.md), signed releases ([docs/RELEASES.md](../../docs/RELEASES.md)), MFA-by-default admin path.
- This working assumption document and [annex-I-evidence-matrix.md](annex-I-evidence-matrix.md).

## Change log (scaffold)

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Initial Class I working assumption draft for CRA scaffold |
