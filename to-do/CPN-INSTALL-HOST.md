# CPN install host (cpn.newstargeted.com)

## Status (11/09/2026)

- Docroot: `/home/newstargeted.com/public_html/cpn.newstargeted.com/`
- `install.sh`, `upgrade.sh`, and `.htaccess` are deployed on the VPS.

## Public one-liners (host, then GitHub raw)

```bash
# Install
sh <(curl -fsSL https://cpn.newstargeted.com/install.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh || wget -O - https://cpn.newstargeted.com/install.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh)

# Upgrade
sh <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh || wget -O - https://cpn.newstargeted.com/upgrade.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh)
```

`curl -fsSL` is required so a down or non-200 host fails and the next URL runs.

## Re-sync from repo

```bash
scp scripts/install.sh NewsTargeted.com:/tmp/cpn-install.sh
scp scripts/upgrade.sh NewsTargeted.com:/tmp/cpn-upgrade.sh
scp to-do/deploy-cpn-install-host.sh NewsTargeted.com:/tmp/deploy-cpn-install-host.sh
ssh NewsTargeted.com 'bash /tmp/deploy-cpn-install-host.sh'
```
