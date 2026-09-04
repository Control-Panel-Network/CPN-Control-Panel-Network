# CPN operator CLI (`cpn`)

SSH-friendly operator tool shipped next to `cpn-installer` in the RPM (`/usr/bin/cpn`).

## Entrypoint

- Source: `src/bin/cpn.rs`
- Shared library: `src/lib.rs` (`cpn_installer` crate)
- Account crypto / bootstrap: `src/account.rs`
- Account CRUD: `src/account_mgmt.rs`
- Site JSON records: `src/sites.rs`

## Data paths

Default data root: `/var/lib/cpn` (override with `CPN_DATA_DIR`).

| Path | Purpose |
|---|---|
| `/var/lib/cpn/panel-bootstrap.json` | First / panel account (mode `0600`) |
| `/var/lib/cpn/accounts/<user>.json` | Extra accounts after bootstrap exists |
| `/var/lib/cpn/sites/<domain>.json` | Website registry (mode `0600`); `docroot` points at files under `/home/...` |

Password hashes use the same format as the installer: `sha256(salt_hex + "|" + utf8_password)` as lowercase hex. Passwords are never written to logs; generated passwords print once as `generated_password=...` on stdout.

## Privileges

Account and site mutations require root (`geteuid == 0`). For lab or tests with a custom `CPN_DATA_DIR`, set `CPN_ALLOW_NONROOT=1`.

## Builtins

```bash
cpn --help
cpn version
cpn list
```

## Accounts

```bash
# Interactive password prompt (hidden input)
sudo cpn account create --username admin --email admin@example.com

# Scripted: password on stdin (not in argv)
printf '%s' 'YourPass1!' | sudo cpn account create --username admin --email admin@example.com --password-stdin

# Generate a policy-compliant password
sudo cpn account create --username admin --email admin@example.com --generate

sudo cpn account list
sudo cpn account passwd --username admin --generate
sudo cpn account delete --username ops --yes
```

## Sites (files under `/home`; JSON registry stubbed for vhosts)

Site commands create document roots under `/home/<domain>/public_html` (subdomains nest under the parent home). Registry JSON under `/var/lib/cpn/sites/` points at each `docroot`. They do **not** yet write Nginx / Caddy / OpenLiteSpeed vhost files (`vhost_wired=false`). See `to-do/SITE-DOCROOT.md`.

```bash
sudo cpn site create --domain example.com --owner admin
sudo cpn site create --domain blog.example.com --owner admin
sudo cpn site modify --domain example.com --owner ops --disable
sudo cpn site list
sudo cpn site delete --domain blog.example.com --yes
```

```bash
ssh -p 2222 root@127.0.0.1 'cpn site create --domain demo.example.com --owner admin'
```

## Example over SSH (AlmaLinux lab)

```bash
ssh -p 2222 root@127.0.0.1 'cpn version'
ssh -p 2222 root@127.0.0.1 'cpn account list'
ssh -p 2222 root@127.0.0.1 'cpn account passwd --username admin --generate'
ssh -p 2222 root@127.0.0.1 'cpn site create --domain demo.example.com --owner admin'
```

## Extending later

Add new top-level groups beside `account`, `site`, `network`, and `plugin` in `src/bin/cpn.rs` (for example `mail`, `ssl`, `service`). Keep mutation commands root-only, confirm deletes unless `--yes`, and avoid putting secrets in argv when a stdin flag can be used.

## Plugins

See `to-do/PLUGINS.md`. Install root: `/home/<domain>/plugins/`. Catalog: https://github.com/master3395/cyberpanel-plugins

```bash
sudo cpn plugin list
sudo cpn plugin list --domain example.com
sudo cpn plugin install --domain example.com --id examplePlugin
sudo cpn plugin enable --domain example.com --id examplePlugin
sudo cpn plugin disable --domain example.com --id examplePlugin
sudo cpn plugin remove --domain example.com --id examplePlugin --yes
sudo cpn plugin migrate --domain example.com
```

## Network (listen port / hostname)

See `to-do/PANEL-PORT-SUBDOMAIN.md`.

```bash
sudo cpn network show
sudo cpn network set-port --port 9443 --old-port-policy redirect_1m
sudo cpn network set-hostname --hostname panel.example.com
sudo cpn network clear-hostname
sudo cpn network clear-migration
```

Paths: `/var/lib/cpn/listen_port`, `panel_hostname`, `port_migration` (mode `0600`).