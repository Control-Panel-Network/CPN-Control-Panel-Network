# CPN Plugins

CPN Panel can install optional plugins from the News Targeted / community plugin catalog.

## Install path

Plugins are stored under:

- `/var/lib/cpn/plugins/<plugin-id>/` (default Unix data root)
- Override root with `CPN_DATA_DIR` (labs and custom installs)

Each installed plugin gets a CPN manifest at `cpn-plugin.json` next to the plugin files. Legacy catalog packages may ship `meta.xml`; CPN maps those fields into `cpn-plugin.json` and rewrites user-facing names/descriptions so the Panel never shows competing product branding.

## Catalog

Catalog URL: https://github.com/master3395/cyberpanel-plugins

CPN downloads the catalog as a GitHub tarball (`codeload.github.com/.../tar.gz/refs/heads/main`), extracts plugin folders that contain `meta.xml`, and caches the result for one hour in `/var/lib/cpn/plugin-catalog-cache.json`.

## Panel UI

Session-gated **Plugins** nav item (same auth as Dashboard / Websites):

- **Installed**: grid or table, activate/deactivate, settings/help/about stubs, uninstall
- **Plugin Store**: search, categories, install from catalog

Routes (served by `cpn-installer`):

- `GET /plugins`
- `POST /plugins/install`
- `POST /plugins/uninstall`
- `POST /plugins/enable`
- `POST /plugins/disable`

## CLI

```bash
sudo cpn plugin list
sudo cpn plugin install --id examplePlugin
sudo cpn plugin enable --id examplePlugin
sudo cpn plugin disable --id examplePlugin
sudo cpn plugin remove --id examplePlugin --yes
```

## Notes

- Plugin runtime hooks (settings pages, service wiring) are stubs in this release; install/state management is live.
- Treat third-party plugins as untrusted until reviewed. Use at your own risk on production hosts.
