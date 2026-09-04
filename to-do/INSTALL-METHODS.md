# CPN install and management methods

Operators can install and manage CPN three ways. All three share the same data under `/var/lib/cpn` (override with `CPN_DATA_DIR`) and the same website files under `/home/<domain>/`.

## 1) GUI (installer web UI + Panel)

1. Install the package (RPM / deb / Docker) so `cpn-installer` is on the host.
2. Start the installer:

```bash
sudo cpn-installer
# Optional listen port (labs often use 2087 or 2088)
sudo cpn-installer --port 2087
```

3. Open the printed URL (`http://127.0.0.1:2087/?token=...` or the lab host port). Complete server, mail, and first-account stages in the browser.
4. After install, open the Panel login (same host/port, `/login` or `/`). Use Websites, Email, Databases, Apps, Backups, and Plugins from the sidebar.

Panel website create uses the same `/home/<domain>/public_html` layout as the CLI.

## 2) CLI (`cpn` and `cpn-installer` flags)

```bash
sudo cpn-installer --help
sudo cpn --help
sudo cpn version
sudo cpn list

# Accounts
sudo cpn account create --username admin --email admin@example.com --generate
sudo cpn account list

# Sites (files under /home)
sudo cpn site create --domain example.com --owner admin
sudo cpn site list

# Plugins (per domain)
sudo cpn plugin install --domain example.com --id examplePlugin
sudo cpn plugin list --domain example.com

# Network
sudo cpn network show
```

Useful installer flags: `--port`, `--allow-remote`, maintenance/upgrade flags documented in `to-do/UPGRADE-REPAIR.md`.

## 3) SSH (operator commands on the host)

From a workstation into a lab VM (example AlmaLinux 9 on host port 2222):

```bash
ssh -p 2222 root@127.0.0.1 'cpn version'
ssh -p 2222 root@127.0.0.1 'cpn site create --domain demo.example.com --owner admin'
ssh -p 2222 root@127.0.0.1 'cpn site list'
ssh -p 2222 root@127.0.0.1 'cpn plugin list'
```

AlmaLinux 10 lab often uses SSH port `2223` and Panel UI port `2088`. Credentials live in the lab credential file next to the VirtualBox VMs (not in this repo).

## Path cheat sheet

| What | Path |
|---|---|
| Panel registry / bootstrap | `/var/lib/cpn/` (internal; override `CPN_DATA_DIR`) |
| Website files | `/home/<domain>/public_html` |
| Subdomain files | `/home/<parent>/<sub.fqdn>/public_html` |
| Site backups | `/home/<domain>/backups/` (subdomains nested under parent) |
| Panel backups | `/home/cpn-panel/backups/` |
| Site plugins | `/home/<domain>/plugins/<plugin-id>/` |

See also: `to-do/CLI.md`, `to-do/SITE-DOCROOT.md`, `to-do/PLUGINS.md`, `README.md`.
