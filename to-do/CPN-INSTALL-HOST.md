# CPN install host (cpn.newstargeted.com)

## Status (12/09/2026)

- Docroot: `/home/newstargeted.com/public_html/cpn.newstargeted.com/`
- `install.sh`, `upgrade.sh`, `cpn-bootstrap-lib.sh`, and `.htaccess` are deployed on the VPS.

## Public one-liners (host, then GitHub raw)

```bash
# Install
bash <(curl -fsSL https://cpn.newstargeted.com/install.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh)

# Upgrade
bash <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh)

# Pin ref / Release
bash <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh) -b 1.0.0-dev
```

See repo [docs/INSTALL.md](../docs/INSTALL.md).

## Re-sync from repo

```bash
scp scripts/install.sh NewsTargeted.com:/tmp/cpn-install.sh
scp scripts/upgrade.sh NewsTargeted.com:/tmp/cpn-upgrade.sh
scp scripts/cpn-bootstrap-lib.sh NewsTargeted.com:/tmp/cpn-bootstrap-lib.sh
scp to-do/deploy-cpn-install-host.sh NewsTargeted.com:/tmp/deploy-cpn-install-host.sh
ssh NewsTargeted.com 'bash /tmp/deploy-cpn-install-host.sh'
```
