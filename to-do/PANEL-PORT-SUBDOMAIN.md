# Panel listen port and subdomain

Default CPN installer/panel listen port is **2087** (Cloudflare-friendly, WHM HTTPS family). Operators can change the port and optionally publish a hostname so login uses HTTPS without a port in the URL.

## Persistence (`/var/lib/cpn/`)

| File | Mode | Purpose |
|---|---|---|
| `listen_port` | `0600` | Preferred TCP listen port (plain text integer) |
| `panel_hostname` | `0600` | Optional DNS name (e.g. `panel.example.com`) |
| `port_migration` | `0600` | JSON: `old_port`, `new_port`, `mode`, `expires_at` |

Override the data root with `CPN_DATA_DIR` in labs.

`port_migration.mode` values:

- `redirect_1m`: dual-listen HTTP redirect helper on the old port for about 1 month
- `redirect_3m`: same for about 3 months
- `deny`: do not listen on the old port (no redirect)

After expiry, `cpn-installer` clears the migration record on startup.

## Subdomain without a port

1. Save hostname: UI **Save networking**, or `sudo cpn network set-hostname --hostname panel.example.com`
2. Point DNS A/AAAA for that name at the server
3. Terminate TLS on **443** (Nginx, Caddy, OpenLiteSpeed, or a CDN) and reverse-proxy to the CPN listen port (default `127.0.0.1:2087`)
4. Public login URL becomes `https://panel.example.com/login` (no `:2087` in the browser)

CPN itself still binds the configured listen port. Hostname alone does not open port 443.

## Changing the port

Installer UI (server selection) and `POST /api/listen-port` accept:

- `port` (required)
- `old_port_policy`: `redirect_1m` | `redirect_3m` | `deny` (required when the port changes)
- `panel_hostname` (optional; empty string clears)

Restart is required to bind a new port:

```bash
sudo cpn-installer --port 9443
# or systemd / CPN_LISTEN_PORT
```

When a redirect migration is active and the process is bound to the **new** port, a lightweight TCP helper also listens on the **old** port and answers with HTTP 302 to the new base URL (or `https://{panel_hostname}` when set).

## Operator CLI

```bash
sudo cpn network show
sudo cpn network set-port --port 9443 --old-port-policy redirect_1m
sudo cpn network set-hostname --hostname panel.example.com
sudo cpn network clear-hostname
sudo cpn network clear-migration
```

Installer flags:

```bash
sudo cpn-installer --port 9443 --old-port-policy redirect_3m --panel-hostname panel.example.com
```

## UI fields

On the installer server-selection screen:

- Listen port
- Old-port policy radios (shown when the draft port differs from the bound port)
- Panel hostname
- Save networking

Status JSON also exposes `panel_hostname`, `port_migration`, and `public_base_url`.
