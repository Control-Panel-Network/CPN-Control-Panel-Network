#!/usr/bin/env bash
set -euo pipefail
DOCROOT="/home/newstargeted.com/public_html/cpn.newstargeted.com"
install -d -m 755 -o newst3922 -g nobody "$DOCROOT"
install -m 644 -o newst3922 -g newst3922 /tmp/cpn-install.sh "$DOCROOT/install.sh"

cat > "$DOCROOT/.htaccess" <<'EOF'
Options -Indexes
<IfModule mod_headers.c>
  <FilesMatch "^(install\.sh|preUpgrade\.sh)$">
    Header set Content-Type "text/plain; charset=utf-8"
    Header set Content-Disposition "inline"
    Header set X-Content-Type-Options nosniff
    Header set Cache-Control "no-cache, max-age=300"
  </FilesMatch>
</IfModule>
EOF
chown newst3922:newst3922 "$DOCROOT/.htaccess"
chmod 644 "$DOCROOT/.htaccess"

cat > "$DOCROOT/index.html" <<'EOF'
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>CPN Install</title>
</head>
<body>
  <h1>CPN Control Panel Network</h1>
  <p>Bootstrap install script: <a href="/install.sh">/install.sh</a></p>
  <p>Project: <a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network">GitHub</a></p>
</body>
</html>
EOF
chown newst3922:newst3922 "$DOCROOT/index.html"
chmod 644 "$DOCROOT/index.html"

# Local CyberPanel DNS (may be unused when Cloudflare is authoritative)
python3 /usr/local/CyberCP/cli/cyberPanel.py createDNSRecord \
  --domainName newstargeted.com --name cpn --recordType A \
  --value 207.180.193.210 --ttl 3600 2>/dev/null || true

# Cloudflare DNS via stored CyberPanel credentials (no secrets printed)
/usr/local/CyberCP/bin/python - <<'PY' || true
import sys
sys.path.insert(0, "/usr/local/CyberCP")
try:
    from plogical.cloudflareClient import get_cloudflare_client
except Exception as exc:
    print("cf_import_fail", type(exc).__name__)
    raise SystemExit(0)
try:
    cf = get_cloudflare_client()
    zones = cf.zones.get(params={"name": "newstargeted.com"})
    if not zones:
        print("cf_zone_missing")
        raise SystemExit(0)
    zid = zones[0]["id"]
    existing = cf.zones.dns_records.get(zid, params={"name": "cpn.newstargeted.com"})
    if existing:
        print("cf_dns_exists", existing[0].get("type"), existing[0].get("proxied"))
    else:
        rec = cf.zones.dns_records.post(zid, data={
            "type": "A",
            "name": "cpn",
            "content": "207.180.193.210",
            "ttl": 1,
            "proxied": True,
        })
        print("cf_dns_created", rec.get("name"), rec.get("proxied"))
except Exception as exc:
    print("cf_dns_fail", type(exc).__name__)
PY

if command -v systemctl >/dev/null 2>&1; then
  systemctl restart lsws || /usr/local/lsws/bin/lswsctrl restart || true
else
  /usr/local/lsws/bin/lswsctrl restart || true
fi

ls -la "$DOCROOT"
echo DEPLOY_OK
