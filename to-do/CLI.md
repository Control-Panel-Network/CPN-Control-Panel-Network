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
| `/var/lib/cpn/sites/<domain>.json` | Website records (mode `0600`) |

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

## Sites (structured JSON; vhost wiring stubbed)

Site commands create and update records under `/var/lib/cpn/sites/`. They do **not** yet write Nginx / Caddy / OpenLiteSpeed vhost files (`vhost_wired=false`). When panel recipes own vhost generation, wire those writers into the same CLI commands.

```bash
sudo cpn site create --domain app.example.com --owner admin
sudo cpn site modify --domain app.example.com --owner ops --disable
sudo cpn site list
sudo cpn site delete --domain app.example.com --yes
```

## Example over SSH (AlmaLinux lab)

```bash
ssh -p 2222 root@127.0.0.1 'cpn version'
ssh -p 2222 root@127.0.0.1 'cpn account list'
ssh -p 2222 root@127.0.0.1 'cpn account passwd --username admin --generate'
ssh -p 2222 root@127.0.0.1 'cpn site create --domain demo.example.com --owner admin'
```

## Extending later

Add new top-level groups beside `account` and `site` in `src/bin/cpn.rs` (for example `mail`, `ssl`, `service`). Keep mutation commands root-only, confirm deletes unless `--yes`, and avoid putting secrets in argv when a stdin flag can be used.