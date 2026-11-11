# CPN security support period policy (draft)

**Status:** Engineering policy draft for CRA transparency.  
**Not a legal warranty statement.** Does not claim CRA conformity or a signed DoC.  
**Aligned surfaces:** [SECURITY.md](../../SECURITY.md), [docs/SUPPORT.md](../../docs/SUPPORT.md), Version Management UI ("latest two published releases").

## Policy summary

1. **Security support set:** Only the **latest two published GitHub Releases** with installable assets receive security fixes as a matter of product policy (current release + previous release).
2. **Outside that window:** Older tags may remain downloadable for labs and emergency rollback, but they are **outside security support**. Operators should upgrade.
3. **Calendar end-date guidance:** CRA expects clarity at acquisition. Until each release ships with a fixed `mm/yyyy` end stamp on download pages, use the rules below so operators can derive an end month without false precision.

## How to derive the support end month

For a release tag `vX.Y.Z` published on date `P` (display `dd/mm/yyyy`):

| Situation | Support end guidance |
|-----------|----------------------|
| Tag is still one of the latest two releases | Supported until it is displaced by two newer published releases with installable assets |
| Tag has just fallen out of the latest two | Support ends in the **calendar month** when the second newer release was published (record as `mm/yyyy`) |
| Manufacturer publishes an explicit end date for a line | That `mm/yyyy` wins over the rolling latest-two rule for that line |

### Snapshot example (as of 11/11/2026)

| Release | Published (approx.) | In latest two? | Support end guidance |
|---------|---------------------|----------------|----------------------|
| v1.4.10 | 11/11/2026 | Yes (current) | Rolling: until displaced by two newer releases |
| v1.4.9 | 07/11/2026 | Yes (previous) | Rolling: until displaced by two newer releases |
| v1.4.8 and older | earlier | No | Outside support; upgrade to a latest-two tag |

Refresh this snapshot when cutting releases. Do not leave stale "current" rows in public claims.

## Free security updates

During the support period for a supported release line, security remediation is intended to be available **without a separate security-fix fee** through normal upgrade channels (GitHub Releases, panel Version Management / auto-update, `cpn-installer --upgrade`). Feature work may ship in the same channels; prefer dedicated security releases when feasible (Phase 2b metadata).

## What this policy is not

- Not a promise that every CVE is fixed within a fixed hour count regardless of severity (see SECURITY.md SLAs for response targets).
- Not CE marking or CRA conformity.
- Not OS-distro support (see [docs/SUPPORT.md](../../docs/SUPPORT.md) for guest OS tiers).
- Not a commitment that `dev` branch commits are production-supported (production branch remains `stable` / published releases).

## Operator-facing wording (canonical)

Use this short form in SECURITY.md, SUPPORT.md, download blurbs, and Version UI (Version UI already states latest two):

> CPN provides security updates for the latest two published releases only (current release plus the previous release). When a release falls out of that set, treat its security support as ended in the calendar month it was displaced (`mm/yyyy`). Older tags may still install for labs but are outside support.

## Implementation backlog (other phases / UI)

- [ ] Show explicit `mm/yyyy` support-end on download pages when a tag leaves the latest two (or pre-declare per release).
- [ ] Keep Version Management warning for outside-support tags (already present).
- [ ] Remove any remaining "best effort" security-support wording from SECURITY.md (done in CRA Phase 1 doc pass).
- [ ] Link public advisory log when Phase 2c ships.

## Change log

| Date (dd/mm/yyyy) | Change |
|-------------------|--------|
| 08/11/2026 | Initial support-period policy; latest-two + calendar end-date guidance |
