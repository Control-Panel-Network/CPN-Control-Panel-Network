# CPN Plugins

CPN Panel installs optional plugins from the community plugin catalog into **site homes**.

## Install path (domain-keyed)

Plugins are **per site FQDN**, not per panel username:

- `/home/<domain>/plugins/<plugin-id>/`
- Subdomains: `/home/<parent>/<sub.fqdn>/plugins/<plugin-id>/`

ACL (who may install/enable/uninstall) is per panel account/team via site ownership plus optional grants in `$CPN_DATA_DIR/site-acl.json`. Files always stay under the domain/subdomain home.

Each installed plugin gets `cpn-plugin.json` next to the plugin files. Legacy catalog `meta.xml` is mapped into that manifest with CPN-only user-facing names.

### Legacy migration

Older installs under `/var/lib/cpn/plugins/` migrate with:

```bash
sudo cpn plugin migrate --domain example.com
```

## Catalog

Catalog URL: https://github.com/master3395/cyberpanel-plugins

Cache: `$CPN_DATA_DIR/plugin-catalog-cache.json` (one hour).

## Panel UI

Session-gated **Plugins** nav:

- Site picker: only domains/subdomains the session user may manage
- Installed / Plugin Store views
- Not a global install for every account on the host

Routes: `GET /plugins`, `POST /plugins/install|uninstall|enable|disable` (each checks site ACL).

## CLI

```bash
sudo cpn plugin list
sudo cpn plugin list --domain example.com
sudo cpn plugin install --domain example.com --id examplePlugin
sudo cpn plugin enable --domain blog.example.com --id examplePlugin
sudo cpn plugin remove --domain example.com --id examplePlugin --yes
sudo cpn plugin migrate --domain example.com
```

## Notes

- Plugin runtime hooks remain stubs in this release; install/state management is live.
- Treat third-party plugins as untrusted until reviewed.
