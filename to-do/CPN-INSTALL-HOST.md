# CPN install host (cpn.newstargeted.com)

## Status (10/09/2026)

- CyberPanel child domain `cpn.newstargeted.com` was created under `newstargeted.com`.
- Docroot: `/home/newstargeted.com/public_html/cpn.newstargeted.com/`
- `install.sh` and `.htaccess` are deployed on the VPS.
- LiteSpeed was restarted after deploy.
- Public DNS for `cpn.newstargeted.com` was **not** created: Cloudflare API credentials stored under `/home/cyberpanel/CloudFlare*` returned auth errors (9103 / invalid Authorization). CyberPanel local DNS create is unreliable while Cloudflare nameservers are authoritative.

## Ops to make the install one-liner live

1. In Cloudflare (zone `newstargeted.com`), add an **A** record:
   - Name: `cpn`
   - Content: `207.180.193.210`
   - Proxy: optional (orange cloud OK once SSL is issued)
2. Issue SSL for `cpn.newstargeted.com` (CyberPanel UI or `python3 /usr/local/CyberCP/cli/cyberPanel.py issueSSL --domainName cpn.newstargeted.com`).
3. Verify:

```bash
curl -fsSL https://cpn.newstargeted.com/install.sh | head
```

Expect a bash script starting with `#!/usr/bin/env bash`, not HTML.

4. Re-sync `install.sh` from the repo when bootstrap scripts change:

```bash
# from a machine with the repo + SSH to the VPS
scp scripts/install.sh NewsTargeted.com:/tmp/cpn-install.sh
ssh NewsTargeted.com 'install -m 644 -o newst3922 -g newst3922 /tmp/cpn-install.sh /home/newstargeted.com/public_html/cpn.newstargeted.com/install.sh'
```

## GitHub fallback (already works after `stable` push)

`https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh`
