#!/usr/bin/env python3
"""Rebuild cpn-installer on AL9 from origin/main and restart with allow-remote."""
from __future__ import annotations

import paramiko

HOST = "127.0.0.1"
PORT = 2222


def main() -> int:
    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(
        HOST,
        port=PORT,
        username="cpn",
        password="CpnLab2026!",
        timeout=20,
        allow_agent=False,
        look_for_keys=False,
    )
    script = r"""#!/bin/bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /home/cpn/CPN-Control-Panel-Network
git fetch origin
git checkout -f main
git reset --hard origin/main
# Build installer-ui embed then release binary
cd installer-ui
npm ci --silent
npm run build
cd ..
cargo build --release --locked --bin cpn-installer
sudo pkill -f /usr/bin/cpn-installer || true
sleep 1
sudo cp -f target/release/cpn-installer /usr/bin/cpn-installer
sudo chmod 755 /usr/bin/cpn-installer
# Keep firewalld lab access
sudo firewall-cmd --add-port=2087/tcp || true
sudo bash -lc 'nohup /usr/bin/cpn-installer --allow-remote --port 2087 >/tmp/cpn-installer-out.log 2>&1 &'
sleep 2
pgrep -a cpn-installer
grep -E 'fingerprint|Bootstrap|2087' /tmp/cpn-installer-out.log | head -10
# Confirm SPA strip marker present
strings /usr/bin/cpn-installer | grep -F 'stripTokenFromUrl' | head -2 || strings /usr/bin/cpn-installer | grep -F 'cpn_install_token' | head -2
echo REDEPLOY_OK=yes
"""
    sftp = client.open_sftp()
    with sftp.file("/tmp/cpn-redeploy-main.sh", "w") as handle:
        handle.write(script)
    sftp.chmod("/tmp/cpn-redeploy-main.sh", 0o755)
    sftp.close()
    print("=== rebuilding on AL9 (may take several minutes) ===", flush=True)
    _stdin, stdout, stderr = client.exec_command(
        "bash /tmp/cpn-redeploy-main.sh", timeout=2400, get_pty=True
    )
    while True:
        line = stdout.readline()
        if not line:
            break
        print(line, end="", flush=True)
    err = stderr.read().decode("utf-8", "replace")
    if err.strip():
        print(err, flush=True)
    code = stdout.channel.recv_exit_status()
    client.close()
    print("=== exit", code, "===", flush=True)
    return code


if __name__ == "__main__":
    raise SystemExit(main())
