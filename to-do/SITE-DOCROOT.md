# Website document roots

## Convention (Unix)

| Kind | Domain example | Home | Document root |
|---|---|---|---|
| Primary | `example.com` | `/home/example.com/` | `/home/example.com/public_html/` |
| Subdomain | `blog.example.com` | `/home/example.com/blog.example.com/` | `/home/example.com/blog.example.com/public_html/` |

Subdomain create requires a parent site record (longest matching registered parent first). Accept a full FQDN; CPN nests under the parent home.

## Registry

JSON records remain at `/var/lib/cpn/sites/<domain>.json` (or `$CPN_DATA_DIR/sites/`) with a `docroot` field. That path is **internal panel metadata** only. Website files live under `/home/...`. The Websites UI can hide or collapse document root display; operators should think in terms of `/home/<domain>/`.

Related backups:

- Site: `/home/<domain>/backups/` (subdomain: `/home/<parent>/<sub.fqdn>/backups/`)
- Panel-wide: `/home/cpn-panel/backups/`

Related plugins:

- `/home/<domain>/plugins/<plugin-id>/`

Override the hosting home root with `CPN_SITES_HOME` (labs and unit tests).

## Create / delete behaviour

- Create: mkdir home + `public_html` (mode `0755`, ownership root when possible), optional `index.html` placeholder.
- Delete: removes the registry JSON only. Files under `/home/...` are kept.
- Legacy: older records (for example `/var/www/...`) still list and show their existing `docroot`. New creates always default to `/home/.../public_html`.

## CLI

```bash
sudo cpn site create --domain example.com --owner admin
sudo cpn site create --domain blog.example.com --owner admin
sudo cpn site list
```

See also `to-do/CLI.md`.
