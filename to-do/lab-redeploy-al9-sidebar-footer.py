#!/usr/bin/env python3
"""Rebuild cpn-installer on AL9 from feature/sidebar-footer-actions and restart."""
from __future__ import annotations

import lab_ssh

HOST = "127.0.0.1"
PORT = 2222
BRANCH = "feature/sidebar-footer-actions"


def main() -> int:
    client = lab_ssh.connect(host=HOST, port=PORT, password="CpnLab2026!")
    script = rf"""#!/bin/bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /home/cpn/CPN-Control-Panel-Network
git fetch origin
git checkout -f {BRANCH}
git reset --hard origin/{BRANCH}
if [[ ! -f installer-ui/dist/index.html ]]; then
  cd installer-ui
  rm -rf node_modules
  npm ci --silent
  npm run build
  cd ..
else
  echo "KEEPING_EXISTING_INSTALLER_UI_DIST=yes"
fi
cargo build --release --locked --bin cpn-installer
sudo pkill -f /usr/bin/cpn-installer || true
sleep 1
sudo cp -f target/release/cpn-installer /usr/bin/cpn-installer
sudo chmod 755 /usr/bin/cpn-installer
sudo firewall-cmd --add-port=2087/tcp || true
sudo bash -lc 'nohup /usr/bin/cpn-installer --allow-remote --port 2087 >/tmp/cpn-installer-out.log 2>&1 &'
sleep 2
pgrep -a cpn-installer
grep -E 'fingerprint|Bootstrap|2087|listening' /tmp/cpn-installer-out.log | head -15 || true
strings /usr/bin/cpn-installer | grep -F 'cpn-notify-btn' | head -3
strings /usr/bin/cpn-installer | grep -F '/api/panel/notifications' | head -3
strings /usr/bin/cpn-installer | grep -F 'Light mode' | head -3 || echo 'NO_LIGHT_MODE_LABEL=yes'
echo REDEPLOY_OK=yes
"""
    sftp = client.open_sftp()
    with sftp.file("/tmp/cpn-redeploy-sidebar-footer.sh", "w") as handle:
        handle.write(script)
    sftp.chmod("/tmp/cpn-redeploy-sidebar-footer.sh", 0o755)
    sftp.close()
    print("=== rebuilding AL9 from", BRANCH, "(may take several minutes) ===", flush=True)
    _stdin, stdout, stderr = client.exec_command(
        "bash /tmp/cpn-redeploy-sidebar-footer.sh", timeout=2400, get_pty=True
    )
    while True:
        line = stdout.readline()
        if not line:
            break
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
