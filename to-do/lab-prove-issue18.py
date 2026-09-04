#!/usr/bin/env python3
"""AL9 lab proof for issue #18: cancel via AppState kills the live process group.

Runs the unix unit test `slow_child_dies_when_cancel_requested` on the lab checkout.
That test starts `run_command` with a slow process group, calls `request_cancel()`
(the same path SIGINT/SIGTERM uses), and asserts the PGID is gone.
"""
from __future__ import annotations

import lab_ssh

HOST = "127.0.0.1"
PORT = 2222


def main() -> int:
    client = lab_ssh.connect(host=HOST, port=PORT, password="CpnLab2026!")
    script = r"""#!/bin/bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /home/cpn/CPN-Control-Panel-Network
git fetch origin
BRANCH=$(git rev-parse --abbrev-ref HEAD)
echo BRANCH=$BRANCH
echo HEAD=$(git rev-parse --short HEAD)
cargo test --locked slow_child_dies_when_cancel_requested -- --nocapture
echo ISSUE18_CANCEL_PROOF=ok
"""
    sftp = client.open_sftp()
    with sftp.file("/tmp/cpn-issue18-cancel.sh", "w") as handle:
        handle.write(script)
    sftp.chmod("/tmp/cpn-issue18-cancel.sh", 0o755)
    sftp.close()
    print("=== issue #18 cancel proof on AL9 ===", flush=True)
    _stdin, stdout, stderr = client.exec_command(
        "bash /tmp/cpn-issue18-cancel.sh", timeout=1200, get_pty=True
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
