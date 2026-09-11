#!/usr/bin/env bash
set -euo pipefail
DOCROOT="/home/newstargeted.com/public_html/cpn.newstargeted.com"
install -d -m 755 -o newst3922 -g nobody "$DOCROOT"
install -m 644 -o newst3922 -g newst3922 /tmp/cpn-install.sh "$DOCROOT/install.sh"
install -m 644 -o newst3922 -g newst3922 /tmp/cpn-upgrade.sh "$DOCROOT/upgrade.sh"

cat > "$DOCROOT/.htaccess" <<'EOF'
Options -Indexes
<IfModule mod_headers.c>
  <FilesMatch "^(install\.sh|upgrade\.sh|preUpgrade\.sh)$">
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
  <p>Install (host, then GitHub fallback):</p>
  <pre>sh &lt;(curl -fsSL https://cpn.newstargeted.com/install.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh || wget -O - https://cpn.newstargeted.com/install.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh)</pre>
  <p>Upgrade (host, then GitHub fallback):</p>
  <pre>sh &lt;(curl -fsSL https://cpn.newstargeted.com/upgrade.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh || wget -O - https://cpn.newstargeted.com/upgrade.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh)</pre>
  <p>Scripts: <a href="/install.sh">/install.sh</a> · <a href="/upgrade.sh">/upgrade.sh</a></p>
  <p>Project: <a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network">GitHub</a></p>
</body>
</html>
EOF
chown newst3922:newst3922 "$DOCROOT/index.html"
chmod 644 "$DOCROOT/index.html"

if command -v systemctl >/dev/null 2>&1; then
  systemctl restart lsws || /usr/local/lsws/bin/lswsctrl restart || true
else
  /usr/local/lsws/bin/lswsctrl restart || true
fi

ls -la "$DOCROOT"
echo DEPLOY_OK
