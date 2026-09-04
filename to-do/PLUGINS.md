# CPN Plugins

CPN Panel can install optional plugins from the News Targeted / community plugin catalog.

## Install path

Plugins are **per site**:

- `/home/<domain>/plugins/<plugin-id>/`
- Subdomains use the nested home: `/home/<parent>/<sub.fqdn>/plugins/<plugin-id>/`

Each installed plugin gets a CPN manifest at `cpn-plugin.json` next to the plugin files. Legacy catalog packages may ship `meta.xml`; CPN maps those fields into `cpn-plugin.json` and rewrites user-facing names/descriptions so the Panel never shows competing product branding.

### Legacy migration

Older installs placed plugins under `/var/lib/cpn/plugins/`. Selecting a site in the Panel or running:

```bash
sudo cpn plugin migrate --domain example.com
```

moves those folders into `/home/example.com/plugins/` when the target id is not already present.

## Catalog

Catalog URL: https://github.com/master3395/cyberpanel-plugins

CPN downloads the catalog as a GitHub tarball (`codeload.github.com/.../tar.gz/refs/heads/main`), extracts plugin folders that contain `meta.xml`, and caches the result for one hour in `/var/lib/cpn/plugin-catalog-cache.json` (catalog cache only; not plugin files).

## Panel UI

Session-gated **Plugins** nav item (same auth as Dashboard / Websites):

- **Site picker**: required; installs target the selected domain
- **Installed**: grid or table, activate/deactivate, settings/help/about stubs, uninstall
- **Plugin Store**: search, categories, install from catalog into the selected site

Routes (served by `cpn-installer`):

- `GET /plugins`
- `POST /plugins/install` (needs `domain` + `id`)
- `POST /plugins/uninstall`
- `POST /plugins/enable`
- `POST /plugins/disable`

## CLI

```bash
sudo cpn plugin list
sudo cpn plugin list --domain example.com
sudo cpn plugin install --domain example.com --id examplePlugin
sudo cpn plugin enable --domain example.com --id examplePlugin
sudo cpn plugin disable --domain example.com --id examplePlugin
sudo cpn plugin remove --domain example.com --id examplePlugin --yes
sudo cpn plugin migrate --domain example.com
```

## Notes

- Plugin runtime hooks (settings pages, service wiring) are stubs in this release; install/state management is live.
- Treat third-party plugins as untrusted until reviewed. Use at your own risk on production hosts.
