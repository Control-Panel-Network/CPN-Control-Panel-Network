# CPN CLI Reference

This document covers the operator CLI (`cpn`) and the installer runtime (`cpn-installer`).

> [!NOTE]
> Run `cpn --help`, `cpn <group> --help`, or `cpn-installer --help` on the installed version for the authoritative command syntax for that release.

## `cpn`

`cpn` is the post-install operator CLI. Read-only commands can run normally; commands that modify accounts, sites, apps, packages, plugins, or network settings require root privileges.

```bash
cpn --help
cpn list
cpn version
```

Top-level groups:

| Group | Purpose |
|---|---|
| `account` | Panel/operator account management |
| `site` | Website records |
| `network` | Listen port, hostname, and port migration |
| `plugin` | Per-site plugin management |
| `app` | Host applications and services |
| `package` | Hosting packages and account assignments |

## Accounts

```bash
cpn account list

sudo cpn account create \
  --username admin \
  --email admin@example.com \
  --language en \
  --generate

sudo cpn account passwd --username admin --generate
sudo cpn account delete --username admin --yes
```

Options used by account commands:

- `--username <NAME>` — account username.
- `--email <EMAIL>` — recovery/account email on create.
- `--language <LANG>` — account language; defaults to `en`.
- `--password-stdin` — read the password from stdin instead of argv.
- `--generate` — generate a password that satisfies the default password policy.
- `--yes` — skip destructive-operation confirmation where supported.

## Sites

```bash
cpn site list

sudo cpn site create \
  --domain example.com \
  --owner admin \
  --ssl-provider letsencrypt

sudo cpn site modify --domain example.com --disable
sudo cpn site modify --domain example.com --enable
sudo cpn site delete --domain example.com --yes
```

`site create` accepts:

- `--domain <DOMAIN>`
- `--owner <ACCOUNT>` — defaults to `admin`.
- `--docroot <PATH>`
- `--engine <ENGINE>`
- `--notes <TEXT>`
- `--ssl-provider <PROVIDER>` — `letsencrypt`, `zerossl`, `cloudflare_ca`, `custom`, or `none`.

`site modify` accepts the same mutable fields plus `--enable` or `--disable`. Do not pass both in the same command.

## Network

```bash
cpn network show

sudo cpn network set-port \
  --port 9443 \
  --old-port-policy redirect_1m

sudo cpn network set-hostname --hostname panel.example.com
sudo cpn network clear-hostname
sudo cpn network clear-migration
```

`network set-port` options:

- `--port <PORT>` — preferred listen port.
- `--old-port-policy <MODE>` — `redirect_1m`, `redirect_3m`, or `deny` when changing from the current port.
- `--from-port <PORT>` — optional previous port override used when recording the migration.

After changing the preferred port, restart the installer/panel process on the new port when instructed.

## Plugins

```bash
cpn plugin list
cpn plugin list --domain example.com

sudo cpn plugin install --domain example.com --id <plugin-id>
sudo cpn plugin enable --domain example.com --id <plugin-id>
sudo cpn plugin disable --domain example.com --id <plugin-id>
sudo cpn plugin remove --domain example.com --id <plugin-id> --yes
sudo cpn plugin migrate --domain example.com
```

## Apps

Supported app IDs currently include MariaDB, MySQL, PostgreSQL, phpMyAdmin, Email, and RabbitMQ.

```bash
cpn app list
sudo cpn app install --name postgresql
sudo cpn app start --name postgresql
sudo cpn app stop --name postgresql
sudo cpn app reinstall --name postgresql --yes
sudo cpn app uninstall --name postgresql --yes
```

Install, reinstall, and uninstall can optionally target a site:

```bash
sudo cpn app install --name phpmyadmin --domain example.com
sudo cpn app install --name phpmyadmin --subdomain db.example.com
```

Relevant options:

- `--name <APP>` — application ID.
- `--domain <DOMAIN>` — optional site scope.
- `--subdomain <HOST>` — optional subdomain scope.
- `--yes` — skip confirmation for reinstall/uninstall.

## Hosting packages

```bash
cpn package list

sudo cpn package create \
  --name starter \
  --disk-mb 1000 \
  --bandwidth-mb 1000 \
  --domains 20 \
  --emails 1000 \
  --databases 1000 \
  --ftp-accounts 1000 \
  --fqdn-enabled true

sudo cpn package assign --username admin --package starter
cpn package show --username admin
sudo cpn package delete --id <package-id> --yes
```

`package create` defaults to 1000 MB disk, 1000 MB bandwidth, 20 domains, 1000 email accounts, 1000 databases, 1000 FTP accounts, and FQDN enabled unless values are overridden.

`package update` requires the package ID plus the complete replacement values for its configurable limits.

## `cpn-installer`

The installer can run as a temporary **web UI** or as an interactive **SSH/CLI** wizard. Language defaults to **English** (independent of guest `LANG` / browser locale).

```bash
sudo cpn-installer                 # TTY: choose Web UI or SSH/CLI; non-TTY: Web UI
sudo cpn-installer --web          # Web UI explicitly
sudo cpn-installer --cli          # SSH/CLI wizard (interactive terminal required)
sudo cpn-installer --port 9443    # Web UI listen port (with --web or after choosing Web UI)
sudo cpn-installer --panel-hostname panel.example.com
```

Options:

| Option | Meaning |
|---|---|
| `--web` / `--ui` | Start the web installer UI |
| `--cli` / `--ssh` | Interactive SSH/CLI installer (no browser) |
| `--port <PORT>` | Web UI listen port; default is `2087` |
| `--panel-hostname <HOST>` | Persist the public panel hostname/subdomain |
| `--old-port-policy <MODE>` | Port-change behavior: `redirect_1m`, `redirect_3m`, or `deny` |
| `--allow-remote` | Bind `0.0.0.0` for the web UI; HTTP without TLS |
| `--listen-all` | Alias for `--allow-remote` |
| `-h`, `--help` | Show help |
| `-V`, `--version` | Show version |

Port resolution order is:

1. `--port`
2. `CPN_LISTEN_PORT`
3. saved CPN port preference
4. default `2087`

`CPN_ALLOW_REMOTE=1` is the environment-variable equivalent of `--allow-remote`.

For remote installation with the web UI, SSH forwarding is safer than exposing the temporary installer directly. See the root [README](../README.md) for installation and first-access steps.

The SSH/CLI path covers the main AlmaLinux install decisions (web engine, MariaDB/MySQL/none, phpMyAdmin, panel port, optional hostname, optional mail, first account). Advanced web-only UI options remain available via `--web`.