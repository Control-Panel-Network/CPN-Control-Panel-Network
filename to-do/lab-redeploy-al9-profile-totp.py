#!/usr/bin/env python3
"""Redeploy feature/self-profile-totp on AL9 lab (127.0.0.1:2222)."""
from __future__ import annotations

import lab_ssh

BRANCH = "feature/self-profile-totp"
SCRIPT = rf"""#!/bin/bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /home/cpn/CPN-Control-Panel-Network
sudo pkill -9 -f /usr/bin/cpn-installer || true
pkill -9 -f 'cargo( |$)' || true
sleep 2
git fetch origin
git checkout -f {BRANCH}
git reset --hard origin/{BRANCH}
git rev-parse --short HEAD
cd installer-ui
npm ci --silent
npm run build
cd ..
rm -f target/release/cpn-installer target/release/cpn-installer.d
cargo build --release --locked --bin cpn-installer
strings target/release/cpn-installer | grep -F 'Account profile' | head -2
strings target/release/cpn-installer | grep -F '/login/2fa' | head -2
sudo cp -f target/release/cpn-installer /usr/bin/cpn-installer
sudo chmod 755 /usr/bin/cpn-installer
sudo firewall-cmd --add-port=2087/tcp || true
sudo bash -lc 'nohup /usr/bin/cpn-installer --allow-remote --port 2087 >/tmp/cpn-installer-out.log 2>&1 &'
sleep 2
pgrep -a cpn-installer
echo REDEPLOY_OK=yes
"""


def main() -> int:
    client = lab_ssh.connect(host="127.0.0.1", port=2222, password="CpnLab2026!")
    sftp = client.open_sftp()
    with sftp.file("/tmp/cpn-redeploy-profile-totp.sh", "w") as handle:
        handle.write(SCRIPT)
    sftp.chmod("/tmp/cpn-redeploy-profile-totp.sh", 0o755)
    sftp.close()
    print("=== redeploy self-profile-totp ===", flush=True)
    _stdin, stdout, stderr = client.exec_command(
        "bash /tmp/cpn-redeploy-profile-totp.sh", timeout=2400, get_pty=True
    )
    while True:
        line = stdout.readline()
        if not line:
            break
        try:
            print(line, end="", flush=True)
        except UnicodeEncodeError:
            print(line.encode("ascii", "replace").decode("ascii"), end="", flush=True)
    err = stderr.read().decode("utf-8", "replace")
    if err.strip():
        print(err, flush=True)
    code = stdout.channel.recv_exit_status()
    client.close()
    print("=== exit", code, "===", flush=True)
    return code


if __name__ == "__main__":
    raise SystemExit(main())
