#!/usr/bin/env python3
"""Push local branch files to AL9 worktree and cargo check (OpenSSL available there)."""
from __future__ import annotations

import os
import subprocess
import sys

import lab_ssh

BRANCH = "feature/self-profile-totp"
REMOTE_REPO = "/home/cpn/CPN-Control-Panel-Network"


def main() -> int:
    # Ensure remote has latest commit once pushed; for uncommitted, rsync via git archive is harder.
    # This helper expects the branch already pushed; it checks out and builds.
    client = lab_ssh.connect(host="127.0.0.1", port=2222, password="CpnLab2026!")
    script = f"""#!/bin/bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd {REMOTE_REPO}
sudo pkill -9 -f /usr/bin/cpn-installer || true
pkill -9 -f 'cargo( |$)' || true
sleep 1
git fetch origin
git checkout -f {BRANCH}
git reset --hard origin/{BRANCH}
git rev-parse --short HEAD
# OpenSSL for webauthn-rs
rpm -q openssl-devel || sudo dnf install -y openssl-devel
cargo check --locked --lib 2>&1 | tee /tmp/cpn-passkey-check.log | tail -80
echo CHECK_EXIT=${{PIPESTATUS[0]}}
"""
    sftp = client.open_sftp()
    with sftp.file("/tmp/cpn-check-passkeys.sh", "w") as handle:
        handle.write(script)
    sftp.chmod("/tmp/cpn-check-passkeys.sh", 0o755)
    sftp.close()
    print("=== AL9 cargo check ===", flush=True)
    _stdin, stdout, stderr = client.exec_command(
        "bash /tmp/cpn-check-passkeys.sh", timeout=1800, get_pty=True
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
    return code


if __name__ == "__main__":
    raise SystemExit(main())
