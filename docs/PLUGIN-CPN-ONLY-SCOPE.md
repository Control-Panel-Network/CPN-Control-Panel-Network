# Plugin install scopes: Host | User Panel | Site

Date: 15/10/2026

## Paths

| Target (UI label) | Who | Path |
|--------|-----|------|
| Host | Owner/admin only | `$CPN_DATA_DIR/host-plugins/<id>/` (usually `/var/lib/cpn/host-plugins/`) |
| User Panel | Signed-in CPN user | `$CPN_DATA_DIR/user-plugins/<username>/<id>/` |
| Site | Site ACL (Install) | `/home/<domain>/plugins/<id>/` (nested for subdomains) |

**User Panel** (internal id `cpn` / `cpn-only` / `target=cpn`) is **not** a public site app. Files stay under the panel data dir (not `public_html`). Each install root also gets `.htaccess` deny + `CPN-ONLY.txt` as defense in depth.

Settings URL domain token: `_cpn:<username>` (example `_cpn:cpnowner`). Store and Installed URLs keep `target=cpn` and category `CPN` for compatibility; the operator-facing label is **User Panel**.

## Catalog scope

In plugin `meta.xml`:

```xml
<scope>host+cpn</scope>
```

Also accepted: `install_scope`. Values: `host`, `cpn` / `cpn_only`, `site`, `host+site` / `dual`, `host+cpn`, `cpn+site`, `host+cpn+site` / `all`.

Panel allowlist for User Panel utilities (even if meta is stale): `autoBanSecurityAlerts`, `autoBan`.

## ACL matrix

| Action | Owner/admin | Normal panel user |
|--------|-------------|-------------------|
| See Host install target | Yes | No |
| Install Host | Yes | No (Activate only after Host install) |
| See / install User Panel | Yes | Yes (own account) |
| Uninstall User Panel | Own (or admin viewing own token) | Own account only |
| Install Site | Site Install ACL | Site Install ACL |
| Uninstall Host package | Yes | No (Deactivate / Manage / Open only) |

## Non-owner Store UX

- Install target shows **User Panel** and **Site** (no Host).
- Helper text under User Panel explains account-scoped panel tools vs Site public plugins.
- Site picker stays hidden until Site is selected.

## Routes

- `POST /plugins/install-cpn`
- `POST /plugins/uninstall-cpn`
- `POST /plugins/enable-cpn`
- `POST /plugins/disable-cpn`

## Host install trust (scripts and catalog digest)

Host installs copy the plugin tree out of the catalog tarball and, for a few packages (for example `protonMail`), run a bundled `install-host.sh` / `uninstall-host.sh` as root. Before anything runs, `plugin_script_trust::script_in_host_plugin_dir` enforces a jail: only those two script names are allowed, the plugin dir and the script must not be symlinks, the script must be a regular file that is not world-writable and is owned by root or the panel user, and the canonical path must stay under `$CPN_DATA_DIR/host-plugins/`. The extracted archive is also checked (no `..` escapes, no symlinks) before it is copied.

Optional catalog digest gate (default OFF so existing installs keep working): set `CPN_REQUIRE_PLUGIN_CATALOG_SIG=1` (or `true` / `yes`) in the panel environment, or create the empty file `$CPN_DATA_DIR/plugin-catalog-require-sig`. When enabled, Host install downloads `<catalog tarball URL>.sha256`, reads the first 64-char hex token (plain `sha256sum` output works), and refuses the install when the sidecar is missing or the SHA-256 of the downloaded tarball does not match.

## Example plugin

**Auto Ban Security Alerts** declares `host+cpn`: owners may still Install on Host for everyone; any user may Install for User Panel under their account.
