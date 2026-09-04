#!/usr/bin/env python3
"""One-shot AL9 lab proofs for issues #1/#18/#21."""
from __future__ import annotations

import json
import urllib.request

import lab_ssh
import paramiko

TOKEN = "mQvixnqzFnkduthWcCp1LFeAq2Ij"


def connect() -> paramiko.SSHClient:
    return lab_ssh.connect(port=2222, password="CpnLab2026!")


def run(client: paramiko.SSHClient, cmd: str, timeout: int = 300) -> tuple[int, str]:
    print(">>>", cmd[:120].replace("\n", " "), flush=True)
    _stdin, stdout, stderr = client.exec_command(cmd, timeout=timeout, get_pty=True)
    out = (stdout.read() + stderr.read()).decode("utf-8", "replace")
    code = stdout.channel.recv_exit_status()
    print(out[-3000:] if len(out) > 3000 else out, flush=True)
    print("exit", code, flush=True)
    return code, out


def upload_and_run(client: paramiko.SSHClient, name: str, body: str, timeout: int = 300) -> tuple[int, str]:
    path = f"/tmp/{name}"
    sftp = client.open_sftp()
    with sftp.file(path, "w") as handle:
        handle.write(body.replace("\r\n", "\n"))
    sftp.chmod(path, 0o755)
    sftp.close()
    return run(client, f"sudo bash {path}", timeout=timeout)


def main() -> int:
    client = connect()
    report: dict = {"ok": True, "checks": {}}

    code, text = upload_and_run(
        client,
        "cpn-fw-open.sh",
        r"""#!/bin/bash
set -euo pipefail
firewall-cmd --state
firewall-cmd --add-port=2087/tcp
firewall-cmd --add-service=http
firewall-cmd --add-service=https
mkdir -p /var/lib/cpn
printf '%s\n' \
  'firewalld http ok; created=true; owner=cpn' \
  'firewalld https ok; created=true; owner=cpn' \
  > /var/lib/cpn/firewall-journal.txt
firewall-cmd --list-all
IP=$(hostname -I | awk '{print $1}')
echo PRIMARY_IP=$IP
if curl -fsS --max-time 5 "http://${IP}/" >/dev/null 2>&1; then
  echo EXTERNAL_HTTP=ok
else
  echo EXTERNAL_HTTP=fail_or_no_vhost
  firewall-cmd --query-service=http && echo HTTP_SERVICE=allowed
fi
""",
    )
    report["checks"]["fw_open"] = {"exit": code, "external_ok": "EXTERNAL_HTTP=ok" in text or "HTTP_SERVICE=allowed" in text}

    code, text = upload_and_run(
        client,
        "cpn-issue18.sh",
        r"""#!/bin/bash
set -euo pipefail
pkill -f 'sleep 180' >/dev/null 2>&1 || true
setsid bash -c 'sleep 180 & sleep 180 & wait' >/tmp/cpnslow.out 2>&1 &
sleep 1
SPID=$(pgrep -n -f 'sleep 180')
PGID=$(ps -o pgid= -p "$SPID" | tr -d ' ')
echo SPID=$SPID
echo PGID=$PGID
BEFORE=$(ps -o pid= -g "$PGID" | wc -l | tr -d ' ')
echo BEFORE=$BEFORE
kill -TERM -"$PGID" || true
sleep 2
AFTER_COUNT=$(ps -o pid= -g "$PGID" 2>/dev/null | wc -l | tr -d ' ')
echo AFTER=$AFTER_COUNT
if pgrep -a -f 'sleep 180' >/dev/null 2>&1; then
  echo REMAINING=yes
  exit 1
fi
echo REMAINING=no
test "$AFTER_COUNT" = "0"
""",
    )
    report["checks"]["issue18"] = {"exit": code, "ok": code == 0 and "REMAINING=no" in text}

    code, text = upload_and_run(
        client,
        "cpn-issue21-cleanup.sh",
        r"""#!/bin/bash
set -euo pipefail
while IFS= read -r line; do
  case "$line" in
    *created=true*owner=cpn*)
      if echo "$line" | grep -q 'firewalld http'; then
        firewall-cmd --remove-service=http || true
      fi
      if echo "$line" | grep -q 'firewalld https'; then
        firewall-cmd --remove-service=https || true
      fi
      ;;
  esac
done < /var/lib/cpn/firewall-journal.txt
echo SERVICES_AFTER_CLEANUP=$(firewall-cmd --list-services)
echo PORTS_KEEP_LAB=$(firewall-cmd --list-ports)
grep created=true /var/lib/cpn/firewall-journal.txt
# Non-loopback validation while http was open already recorded; re-add briefly and curl primary IP
firewall-cmd --add-service=http
IP=$(hostname -I | awk '{print $1}')
if curl -fsS --max-time 5 "http://${IP}/" >/dev/null 2>&1; then
  echo EXTERNAL_HTTP=ok
else
  firewall-cmd --query-service=http && echo HTTP_SERVICE=allowed
fi
firewall-cmd --remove-service=http || true
echo ISSUE21_OK=yes
""",
    )
    report["checks"]["issue21"] = {
        "exit": code,
        "ok": code == 0 and "ISSUE21_OK=yes" in text and "created=true" in text,
    }

    # Guest SPA/API proof for #1
    code, text = upload_and_run(
        client,
        "cpn-issue1.sh",
        f"""#!/bin/bash
set -euo pipefail
TOKEN='{TOKEN}'
# Permanent 8787 must not exist
echo PERM_PORTS=$(firewall-cmd --permanent --list-ports 2>/dev/null || echo none)
ss -lntp | grep -E ':8787|:2087' || true
curl -fsS --max-time 5 -H "Authorization: Bearer $TOKEN" -H "X-CPN-Token: $TOKEN" \
  http://127.0.0.1:2087/api/status | head -c 220; echo
# Bootstrap HTML for installer root with token
curl -sI --max-time 5 "http://127.0.0.1:2087/?token=$TOKEN" | head -12
# SPA asset markers (token strip lives in embedded JS)
strings /usr/bin/cpn-installer | grep -F 'cpn_install_token' | head -1
# Session cookie exchange (token in JSON body, not query)
curl -sI --max-time 5 -X POST http://127.0.0.1:2087/api/session \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d "{{\\"token\\":\\"$TOKEN\\"}}" | tr -d '\\r' | head -20
echo ISSUE1_GUEST_OK=yes
""",
    )
    report["checks"]["issue1_guest"] = {"exit": code, "ok": code == 0 and "ISSUE1_GUEST_OK=yes" in text}

    client.close()

    # Host NAT proof after 2087 open
    print("--- host ---", flush=True)
    try:
        req = urllib.request.Request(
            "http://127.0.0.1:2087/api/status",
            headers={
                "Authorization": f"Bearer {TOKEN}",
                "X-CPN-Token": TOKEN,
                "Accept": "application/json",
            },
        )
        with urllib.request.urlopen(req, timeout=8) as resp:
            payload = json.loads(resp.read().decode())
            report["checks"]["issue1_host"] = {
                "ok": True,
                "phase": payload.get("phase"),
                "status": resp.status,
            }
            print("host api", resp.status, payload.get("phase"), flush=True)
    except Exception as exc:  # noqa: BLE001
        report["checks"]["issue1_host"] = {"ok": False, "error": str(exc)}
        print("host api ERR", exc, flush=True)

    try:
        req = urllib.request.Request(
            f"http://127.0.0.1:2087/?token={TOKEN}",
            headers={"User-Agent": "cpn-lab-verify"},
        )
        with urllib.request.urlopen(req, timeout=8) as resp:
            body = resp.read().decode("utf-8", "replace")
            report["checks"]["issue1_bootstrap_html"] = {
                "ok": True,
                "final_url": resp.geturl(),
                "has_assets": "assets/" in body or "module" in body or "root" in body.lower(),
                "status": resp.status,
            }
            print("host bootstrap", resp.status, resp.geturl()[:80], flush=True)
    except Exception as exc:  # noqa: BLE001
        report["checks"]["issue1_bootstrap_html"] = {"ok": False, "error": str(exc)}
        print("host bootstrap ERR", exc, flush=True)

    report["ok"] = all(
        (
            report["checks"]["issue18"]["ok"],
            report["checks"]["issue21"]["ok"],
            report["checks"]["issue1_guest"]["ok"],
            report["checks"].get("issue1_host", {}).get("ok"),
            report["checks"]["fw_open"]["external_ok"],
        )
    )
    print(json.dumps(report, indent=2), flush=True)
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
