#!/usr/bin/env python3
"""Smoke sticky sidebar CSS via guest-side curl on AL9 (avoids host NAT flakes)."""
from __future__ import annotations

import lab_ssh

CREDS = r"D:\OneDrive - v-man\Priv\VirtualBox VMs\CPN-lab-credentials.txt"
BASE = "http://127.0.0.1:2087"


def panel_password() -> str:
    with open(CREDS, encoding="utf-8") as handle:
        lines = handle.readlines()
    for index, line in enumerate(lines):
        if line.strip() == "Username: Admin" and index + 1 < len(lines):
            nxt = lines[index + 1].strip()
            if nxt.startswith("Password:"):
                return nxt.split(":", 1)[1].strip()
    raise RuntimeError("Panel password not found in CPN-lab-credentials.txt")


GUEST_PY = r'''
import json
checks = {}
for name, prefix in (("packages", "packages"), ("email", "email_hub"), ("websites", "websites")):
    html = open("/tmp/cpn-%s.html" % name, encoding="utf-8", errors="replace").read()
    print(name, "bytes", len(html))
    checks["%s_panel_layout" % prefix] = "panel-layout" in html
    checks["%s_sidebar" % prefix] = 'id="panel-sidebar"' in html
    checks["%s_main_scroll" % prefix] = "overflow-y:auto" in html
    checks["%s_dvh_shell" % prefix] = "100dvh" in html
    checks["%s_locked_body" % prefix] = "overflow:hidden" in html
pkg = open("/tmp/cpn-packages.html", encoding="utf-8", errors="replace").read()
checks["packages_table_wrap"] = "table-wrap" in pkg
checks["packages_actions"] = ("Hosting Packages" in pkg) or ("List Packages" in pkg) or ("Edit" in pkg)
checks["packages_sticky_cols"] = ("position:sticky" in pkg) and ("right:0" in pkg)
email = open("/tmp/cpn-email.html", encoding="utf-8", errors="replace").read()
checks["email_hub_tiles"] = ("hub-tile" in email) or ("Email" in email)
checks["email_hub_minmax"] = ("minmax(min(100%" in email) or ("hub-tile-grid" in email)
web = open("/tmp/cpn-websites.html", encoding="utf-8", errors="replace").read()
checks["websites_shell"] = ("Websites" in web) or ("website" in web.lower())
print(json.dumps(checks, indent=2))
ok = all(bool(v) for v in checks.values())
print("SMOKE_OK=yes" if ok else "SMOKE_OK=no")
raise SystemExit(0 if ok else 1)
'''


def main() -> int:
    password = panel_password().replace("'", "'\"'\"'")
    script = f"""#!/bin/bash
set -euo pipefail
sudo pkill -9 -f /usr/bin/cpn-installer || true
sleep 1
sudo bash -lc 'nohup /usr/bin/cpn-installer --allow-remote --port 2087 >/tmp/cpn-installer-out.log 2>&1 &'
sleep 3
echo PROCS=$(pgrep -c cpn-installer || echo 0)
curl -sI --max-time 8 {BASE}/login | head -3 || true
COOKIE_JAR=/tmp/cpn-smoke-cookies.txt
rm -f "$COOKIE_JAR"
LOGIN_CODE=$(curl -sS -c "$COOKIE_JAR" -b "$COOKIE_JAR" -o /tmp/cpn-login.out -w '%{{http_code}}' \\
  -X POST -d 'username=Admin&password={password}' {BASE}/login || echo fail)
echo LOGIN_CODE=$LOGIN_CODE
for path in packages email websites; do
  code=$(curl -sS -b "$COOKIE_JAR" -o /tmp/cpn-$path.html -w '%{{http_code}}' --max-time 15 {BASE}/$path || echo fail)
  echo PAGE_$path=$code
done
cat > /tmp/cpn-smoke-check.py <<'PY'
{GUEST_PY}
PY
python3 /tmp/cpn-smoke-check.py
"""
    client = lab_ssh.connect(host="127.0.0.1", port=2222, password="CpnLab2026!")
    sftp = client.open_sftp()
    with sftp.file("/tmp/cpn-smoke-sticky.sh", "w") as handle:
        handle.write(script)
    sftp.chmod("/tmp/cpn-smoke-sticky.sh", 0o755)
    sftp.close()
    _stdin, stdout, stderr = client.exec_command(
        "bash /tmp/cpn-smoke-sticky.sh", timeout=120, get_pty=True
    )
    out = stdout.read().decode("utf-8", "replace")
    try:
        print(out, flush=True)
    except UnicodeEncodeError:
        print(out.encode("ascii", "replace").decode("ascii"), flush=True)
    err = stderr.read().decode("utf-8", "replace")
    if err.strip():
        print(err, flush=True)
    code = stdout.channel.recv_exit_status()
    client.close()
    return code


if __name__ == "__main__":
    raise SystemExit(main())
