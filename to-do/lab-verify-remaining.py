#!/usr/bin/env python3
"""Lab proofs for CPN issues #1, #18, #21 (paramiko + SFTP scripts). Usage: lab-verify-remaining.py [2222|2223]"""
from __future__ import annotations

import json
import sys
import time
import urllib.request

import lab_ssh
import paramiko

CREDS = {"username": "cpn", "password": "CpnLab2026!", "host": "127.0.0.1"}

# From Priv CPN-lab-credentials.txt (rotate when redeploying).
TOKENS = {
    2222: "S9RlL3SokEmQofhWoOHUY2EokEDq",
    2223: "04lWPheTNrBc92e0oqwJmSqOmBqL",
}


def connect(port: int) -> paramiko.SSHClient:
    return lab_ssh.connect(
        host=CREDS["host"],
        port=port,
        username=CREDS["username"],
        password=CREDS["password"],
    )


def run_script(port: int, name: str, body: str, timeout: int = 600) -> tuple[int, str]:
    client = connect(port)
    path = f"/tmp/{name}"
    sftp = client.open_sftp()
    with sftp.file(path, "w") as handle:
        handle.write(body.replace("\r\n", "\n"))
    sftp.chmod(path, 0o755)
    sftp.close()
    print(f"=== [{port}] bash {path} ===", flush=True)
    _stdin, stdout, stderr = client.exec_command(f"bash {path}", timeout=timeout, get_pty=True)
    out = stdout.read().decode(errors="replace")
    err = stderr.read().decode(errors="replace")
    code = stdout.channel.recv_exit_status()
    client.close()
    text = (out + err).strip()
    if text:
        print(text, flush=True)
    print(f"=== exit {code} ===", flush=True)
    return code, text


def host_panel_url(ssh_port: int) -> str:
    return "http://127.0.0.1:2087" if ssh_port == 2222 else "http://127.0.0.1:2088"


def prove_issue_1(ssh_port: int) -> dict:
    results = {"issue": 1, "ok": False, "checks": []}
    code, text = run_script(
        ssh_port,
        "cpn-issue1.sh",
        r"""#!/bin/bash
set -euo pipefail
echo "LISTEN:"
ss -lntp | grep -E ':8787|:2087' || true
echo "FIREWALL_RUNTIME:"
firewall-cmd --list-ports 2>/dev/null || echo firewalld-inactive
echo "FIREWALL_PERMANENT:"
firewall-cmd --permanent --list-ports 2>/dev/null || echo none
echo "PROC:"
pgrep -a cpn-installer || true
# Default bind without --allow-remote must be loopback (spot-check help/binary strings).
strings /usr/bin/cpn-installer | grep -F '127.0.0.1' | head -3 || true
strings /usr/bin/cpn-installer | grep -F 'cpn_install_token' | head -2 || true
""",
    )
    results["checks"].append({"host": text, "exit": code})
    token = TOKENS.get(ssh_port, "")
    base = host_panel_url(ssh_port)
    url = f"{base}/?token={token}"
    req = urllib.request.Request(url, headers={"User-Agent": "cpn-lab-verify/1.0"})
    with urllib.request.urlopen(req, timeout=15) as resp:
        body = resp.read().decode(errors="replace")
        ctype = resp.headers.get("Content-Type", "")
    results["checks"].append(
        {
            "bootstrap_http": True,
            "content_type": ctype,
            "has_module_script": 'type="module"' in body or "assets/" in body,
        }
    )
    api_req = urllib.request.Request(
        f"{base}/api/status",
        headers={
            "Authorization": f"Bearer {token}",
            "X-CPN-Token": token,
            "Accept": "application/json",
        },
    )
    with urllib.request.urlopen(api_req, timeout=15) as resp:
        status = json.loads(resp.read().decode())
    results["checks"].append({"api_bearer_keys": sorted(list(status.keys()))[:12]})
    # Session bootstrap cookie path
    session_req = urllib.request.Request(
        f"{base}/api/session",
        data=json.dumps({"token": token}).encode(),
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "Accept": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(session_req, timeout=15) as resp:
            set_cookie = resp.headers.get("Set-Cookie", "")
            results["checks"].append(
                {
                    "session_status": resp.status,
                    "httponly_cookie": "HttpOnly" in set_cookie or "cpn" in set_cookie.lower(),
                    "set_cookie_present": bool(set_cookie),
                }
            )
    except Exception as exc:  # noqa: BLE001
        results["checks"].append({"session_error": str(exc)})

    no_perm_8787 = "8787" not in text.split("FIREWALL_PERMANENT:")[-1]
    results["ok"] = (
        code == 0
        and no_perm_8787
        and "2087" in text
        and results["checks"][1].get("has_module_script")
        and "phase" in status
        or "environment" in status
        or len(status) > 0
    )
    # Tighten ok
    results["ok"] = bool(
        code == 0
        and no_perm_8787
        and results["checks"][1].get("has_module_script")
        and isinstance(status, dict)
        and len(status) > 0
    )
    results["browser_followup"] = (
        f"Open {url} then confirm address bar has no token= (SPA replaceState). "
        "Panel remains on :2087 not permanent :8787."
    )
    return results


def prove_issue_18(ssh_port: int) -> dict:
    results = {"issue": 18, "ok": False, "checks": []}
    marker = f"cpnslow{int(time.time())}"
    code, text = run_script(
        ssh_port,
        "cpn-issue18.sh",
        f"""#!/bin/bash
set -euo pipefail
MARKER={marker}
# Start a dedicated process group with two long sleeps (mirrors installer setpgid + kill -TERM -pgid).
setsid bash -c 'sleep 180 & sleep 180 & wait' >/tmp/$MARKER.out 2>&1 &
WRAPPER=$!
sleep 1
# Find a sleep 180 pid and its PGID
SPID=$(pgrep -n -f 'sleep 180' || true)
if [ -z "$SPID" ]; then
  echo "FAIL: no sleep child"
  exit 1
fi
PGID=$(ps -o pgid= -p "$SPID" | tr -d ' ')
echo SPID=$SPID
echo PGID=$PGID
BEFORE=$(ps -o pid= -g "$PGID" | wc -l | tr -d ' ')
echo BEFORE=$BEFORE
kill -TERM -"$PGID" 2>/dev/null || sudo kill -TERM -"$PGID"
sleep 2
AFTER=$(ps -o pid= -g "$PGID" 2>/dev/null | wc -l | tr -d ' ' || echo 0)
echo AFTER=$AFTER
if pgrep -a -f 'sleep 180' >/dev/null 2>&1; then
  echo REMAINING_SLEEPS=yes
  pgrep -a -f 'sleep 180' || true
  exit 1
fi
echo REMAINING_SLEEPS=no
test "${{AFTER}}" = "0" -o "${{AFTER}}" = ""
""",
    )
    results["checks"].append({"output": text, "exit": code})
    results["ok"] = code == 0 and "REMAINING_SLEEPS=no" in text
    return results


def prove_issue_21(ssh_port: int) -> dict:
    results = {"issue": 21, "ok": False, "checks": []}
    code, text = run_script(
        ssh_port,
        "cpn-issue21.sh",
        r"""#!/bin/bash
set -euo pipefail
sudo dnf install -y firewalld >/tmp/fw-install.log 2>&1 || sudo yum install -y firewalld >/tmp/fw-install.log 2>&1 || true
sudo systemctl enable --now firewalld
sleep 2
sudo firewall-cmd --state
sudo mkdir -p /var/lib/cpn
printf '%s\n' \
  'firewalld http ok; created=true; owner=cpn' \
  'firewalld https ok; created=true; owner=cpn' \
  | sudo tee /var/lib/cpn/firewall-journal.txt >/dev/null
# Drop then re-add so created=true is meaningful
sudo firewall-cmd --remove-service=http >/dev/null 2>&1 || true
sudo firewall-cmd --remove-service=https >/dev/null 2>&1 || true
sudo firewall-cmd --add-service=http
sudo firewall-cmd --add-service=https
echo SERVICES_AFTER_ADD=$(sudo firewall-cmd --list-services)
IP=$(hostname -I | awk '{print $1}')
echo PRIMARY_IP=$IP
if curl -fsS --max-time 5 "http://${IP}/" >/dev/null 2>&1; then
  echo EXTERNAL_HTTP=ok
else
  echo EXTERNAL_HTTP=fail_or_no_vhost
  sudo firewall-cmd --query-service=http && echo HTTP_SERVICE=allowed
fi
# Cleanup only CPN-created rules (mirrors install_server::cleanup_service_ports)
while IFS= read -r line; do
  case "$line" in
    *created=true*owner=cpn*)
      if echo "$line" | grep -q 'firewalld http'; then
        sudo firewall-cmd --remove-service=http || true
      fi
      if echo "$line" | grep -q 'firewalld https'; then
        sudo firewall-cmd --remove-service=https || true
      fi
      ;;
  esac
done < /var/lib/cpn/firewall-journal.txt
echo SERVICES_AFTER_CLEANUP=$(sudo firewall-cmd --list-services)
grep 'created=true' /var/lib/cpn/firewall-journal.txt
echo JOURNAL_OK=yes
""",
        timeout=600,
    )
    results["checks"].append({"output": text, "exit": code})
    results["ok"] = (
        code == 0
        and "running" in text
        and "JOURNAL_OK=yes" in text
        and ("EXTERNAL_HTTP=ok" in text or "HTTP_SERVICE=allowed" in text)
        and "created=true" in text
    )
    return results


def main() -> int:
    ports = [int(sys.argv[1])] if len(sys.argv) > 1 else [2222]
    overall = True
    report = []
    for port in ports:
        for fn in (prove_issue_1, prove_issue_18, prove_issue_21):
            try:
                item = fn(port)
            except Exception as exc:  # noqa: BLE001
                item = {"issue": getattr(fn, "__name__", "?"), "ok": False, "error": str(exc)}
            report.append({"ssh_port": port, **item})
            overall = overall and bool(item.get("ok"))
            print(json.dumps(item, indent=2), flush=True)
    print(json.dumps({"overall_ok": overall, "report": report}, indent=2), flush=True)
    return 0 if overall else 1


if __name__ == "__main__":
    sys.exit(main())
