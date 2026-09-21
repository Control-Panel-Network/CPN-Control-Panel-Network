# CPN host site (`cpn.newstargeted.com`)

Public install host for CPN bootstrap scripts and product landing page.

## Layout in this repo

| Path | Purpose |
| --- | --- |
| `site/` | Landing page (`index.html`), assets, `.htaccess` |
| `scripts/install.sh` | Mirrored to `/install.sh` |
| `scripts/upgrade.sh` | Mirrored to `/upgrade.sh` |
| `scripts/cpn-bootstrap-lib.sh` | Mirrored to `/cpn-bootstrap-lib.sh` |
| `scripts/sync-cpn-host-site.sh` | Builds a deployable tree |

Homepage `/` is HTML. Script URLs must keep returning shell source (not HTML) so one-liners work.

## Build a dist tree

```bash
./scripts/sync-cpn-host-site.sh /tmp/cpn-host-dist
```

## Live docroot

CyberPanel / LiteSpeed:

`/home/newstargeted.com/public_html/cpn.newstargeted.com`

Ownership: files `newst3922:newst3922`, docroot folder may stay `newst3922:nobody`. Public files mode `644`.

After `.htaccess` changes, restart LSWS (`systemctl restart lsws`).

## Verify

```bash
curl -sI https://cpn.newstargeted.com/ | head
curl -sI https://cpn.newstargeted.com/install.sh | head
curl -fsSL https://cpn.newstargeted.com/install.sh | head -5
```

Expect `/` as `text/html` and `/install.sh` as script text (`text/plain` or `application/x-sh`) starting with `#!/usr/bin/env bash`.
