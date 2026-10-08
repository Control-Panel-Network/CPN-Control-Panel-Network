# Plugin install scopes: Host | CPN only | Site

Date: 08/10/2026

## Paths

| Target | Who | Path |
|--------|-----|------|
| Host | Owner/admin only | `$CPN_DATA_DIR/host-plugins/<id>/` (usually `/var/lib/cpn/host-plugins/`) |
| CPN only | Signed-in CPN user | `$CPN_DATA_DIR/user-plugins/<username>/<id>/` |
| Site | Site ACL (Install) | `/home/<domain>/plugins/<id>/` (nested for subdomains) |

CPN-only is **not** a public site app. Files stay under the panel data dir (not `public_html`). Each install root also gets `.htaccess` deny + `CPN-ONLY.txt` as defense in depth.

Settings URL domain token: `_cpn:<username>` (example `_cpn:cpnowner`).

## Catalog scope

In plugin `meta.xml`:

```xml
<scope>host+cpn</scope>
```

Also accepted: `install_scope`. Values: `host`, `cpn` / `cpn_only`, `site`, `host+site` / `dual`, `host+cpn`, `cpn+site`, `host+cpn+site` / `all`.

Panel allowlist for CPN utilities (even if meta is stale): `autoBanSecurityAlerts`, `autoBan`.

## ACL matrix

| Action | Owner/admin | Normal panel user |
|--------|-------------|-------------------|
| See Host install target | Yes | No |
| Install Host | Yes | No (Activate only after Host install) |
| See / install CPN only | Yes | Yes (own account) |
| Uninstall CPN only | Own (or admin viewing own token) | Own account only |
| Install Site | Site Install ACL | Site Install ACL |
| Uninstall Host package | Yes | No (Deactivate / Manage / Open only) |

## Non-owner Store UX

- Install target shows **CPN only** and **Site** (no Host).
- Helper text under CPN only explains account-scoped panel tools vs Site public plugins.
- Site picker stays hidden until Site is selected.

## Routes

- `POST /plugins/install-cpn`
- `POST /plugins/uninstall-cpn`
- `POST /plugins/enable-cpn`
- `POST /plugins/disable-cpn`

## Example plugin

**Auto Ban Security Alerts** declares `host+cpn`: owners may still Install on Host for everyone; any user may Install for CPN only under their account.
