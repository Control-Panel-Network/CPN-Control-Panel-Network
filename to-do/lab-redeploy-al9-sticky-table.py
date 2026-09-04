#!/usr/bin/env python3
"""Force a clean rebuild of fix/sticky-sidebar-table-overflow on AL9."""
from __future__ import annotations

import lab_ssh

BRANCH = "fix/sticky-sidebar-table-overflow"
SCRIPT = rf"""#!/bin/bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /home/cpn/CPN-Control-Panel-Network
# Stop anything holding cargo locks / old binaries
sudo pkill -9 -f /usr/bin/cpn-installer || true
pkill -9 -f 'cargo( |$)' || true
sleep 2
git fetch origin
git checkout -f {BRANCH}
git reset --hard origin/{BRANCH}
git rev-parse --short HEAD
grep -n 'align-self:stretch\|right:0\|719.98' src/panel_pages.rs | head -20
cd installer-ui
npm ci --silent
npm run build
cd ..
# Fresh release binary
rm -f target/release/cpn-installer target/release/cpn-installer.d
cargo build --release --locked --bin cpn-installer
# Prove binary embeds new CSS markers
strings target/release/cpn-installer | grep -F 'align-self:stretch' | head -2
strings target/release/cpn-installer | grep -F '719.98' | head -2
strings target/release/cpn-installer | grep -F 'right:0' | head -3
sudo cp -f target/release/cpn-installer /usr/bin/cpn-installer
sudo chmod 755 /usr/bin/cpn-installer
sudo firewall-cmd --add-port=2087/tcp || true
sudo bash -lc 'nohup /usr/bin/cpn-installer --allow-remote --port 2087 >/tmp/cpn-installer-out.log 2>&1 &'
sleep 2
pgrep -a cpn-installer
strings /usr/bin/cpn-installer | grep -F 'align-self:stretch' | head -1
echo REDEPLOY_OK=yes
"""


def main() -> int:
    client = lab_ssh.connect(host="127.0.0.1", port=2222, password="CpnLab2026!")
    sftp = client.open_sftp()
    with sftp.file("/tmp/cpn-redeploy-sticky-clean.sh", "w") as handle:
        handle.write(SCRIPT)
    sftp.chmod("/tmp/cpn-redeploy-sticky-clean.sh", 0o755)
    sftp.close()
    print("=== clean rebuild sticky branch ===", flush=True)
    _stdin, stdout, stderr = client.exec_command(
        "bash /tmp/cpn-redeploy-sticky-clean.sh", timeout=2400, get_pty=True
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
