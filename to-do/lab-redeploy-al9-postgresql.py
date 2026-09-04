#!/usr/bin/env python3
"""Rebuild cpn-installer on AL9 from feature/apps-postgresql and restart on :2087."""
from __future__ import annotations

import lab_ssh

HOST = "127.0.0.1"
PORT = 2222
BRANCH = "feature/apps-postgresql"


def main() -> int:
    client = lab_ssh.connect(host=HOST, port=PORT, password="CpnLab2026!")
    script = rf"""#!/bin/bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /home/cpn/CPN-Control-Panel-Network
git fetch origin
git checkout -f {BRANCH}
git reset --hard origin/{BRANCH}
cd installer-ui
npm ci --silent
npm run build
cd ..
cargo build --release --locked --bin cpn-installer --bin cpn
sudo pkill -f /usr/bin/cpn-installer || true
sleep 1
sudo cp -f target/release/cpn-installer /usr/bin/cpn-installer
sudo cp -f target/release/cpn /usr/bin/cpn
sudo chmod 755 /usr/bin/cpn-installer /usr/bin/cpn
sudo firewall-cmd --add-port=2087/tcp || true
sudo bash -lc 'nohup /usr/bin/cpn-installer --allow-remote --port 2087 >/tmp/cpn-installer-out.log 2>&1 &'
sleep 2
pgrep -a cpn-installer
# Confirm PostgreSQL app id is in the binary
strings /usr/bin/cpn-installer | grep -F 'postgresql' | head -5 || true
echo REDEPLOY_OK=yes
"""
    sftp = client.open_sftp()
    with sftp.file("/tmp/cpn-redeploy-postgresql.sh", "w") as handle:
        handle.write(script)
    sftp.chmod("/tmp/cpn-redeploy-postgresql.sh", 0o755)
    sftp.close()
    print("=== rebuilding AL9 with PostgreSQL apps branch ===", flush=True)
    _stdin, stdout, stderr = client.exec_command(
        "bash /tmp/cpn-redeploy-postgresql.sh", timeout=2400, get_pty=True
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
