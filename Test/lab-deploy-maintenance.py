#!/usr/bin/env python3
"""Hot-deploy panel_maintenance changes to AL10-prerelease lab and smoke-test the flag."""
from __future__ import annotations

import json
import os
import sys
import tarfile
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from lab_ssh_util import connect_cpn_lab  # noqa: E402

HOST = "127.0.0.1"
PORT = 2229
USER = "cpn"
REMOTE_BUILD = "/home/cpn/cpn-maint-build"
REMOTE_TAR = "/tmp/cpn-maint-src.tar.gz"

FILES = [
    "Cargo.toml",
    "Cargo.lock",
    "src/lib.rs",
    "src/main.rs",
    "src/installer.rs",
    "src/upgrade.rs",
    "src/cli_doctor.rs",
    "src/panel_maintenance_mode.rs",
    "src/panel_maintenance_page.rs",
    "src/panel_maintenance_guard.rs",
    "src/panel_maintenance_api.rs",
    "src/panel_brand.rs",
    "packaging/cpn-installer.service",
]


def ssh_password() -> str:
    # Prefer env; fall back to shared lab SSH password from operator env only.
    return os.environ.get("CPN_LAB_SSH_PASSWORD", "")


def run(client, cmd: str, timeout: int = 600) -> tuple[int, str, str]:
    stdin, stdout, stderr = client.exec_command(cmd, timeout=timeout)
    out = stdout.read().decode("utf-8", errors="replace")
    err = stderr.read().decode("utf-8", errors="replace")
    code = stdout.channel.recv_exit_status()
    return code, out, err


def main() -> int:
    password = ssh_password()
    if not password:
        print("Set CPN_LAB_SSH_PASSWORD", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory() as tmp:
        tar_path = Path(tmp) / "src.tar.gz"
        with tarfile.open(tar_path, "w:gz") as tar:
            for rel in FILES:
                path = ROOT / rel
                if not path.is_file():
                    print(f"missing {rel}", file=sys.stderr)
                    return 2
                tar.add(path, arcname=rel)
            # Include full src tree so cargo can link the crate.
            src = ROOT / "src"
            for path in src.rglob("*"):
                if path.is_file():
                    tar.add(path, arcname=str(path.relative_to(ROOT)).replace("\\", "/"))

        client = connect_cpn_lab(HOST, PORT, USER, password)
        try:
            sftp = client.open_sftp()
            sftp.put(str(tar_path), REMOTE_TAR)
            sftp.close()

            cmds = [
                f"rm -rf {REMOTE_BUILD} && mkdir -p {REMOTE_BUILD}",
                f"tar -xzf {REMOTE_TAR} -C {REMOTE_BUILD}",
                f"cd {REMOTE_BUILD} && cargo build --release -p cpn-installer 2>&1",
            ]
            for cmd in cmds:
                print(f">>> {cmd[:80]}...")
                code, out, err = run(client, cmd, timeout=1200)
                text = (out + err).strip()
                if text:
                    # Print last 40 lines only.
                    lines = text.splitlines()
                    print("\n".join(lines[-40:]))
                if code != 0:
                    print(f"FAIL exit={code} for: {cmd}", file=sys.stderr)
                    return code or 1

            install = (
                f"sudo install -m 755 {REMOTE_BUILD}/target/release/cpn-installer /usr/local/bin/cpn-installer "
                f"&& sudo install -m 755 {REMOTE_BUILD}/target/release/cpn /usr/local/bin/cpn "
                f"&& sudo systemctl restart cpn-installer "
                f"&& sleep 2 && systemctl is-active cpn-installer && curl -sI http://127.0.0.1:2087/login | head -5"
            )
            print(">>> install + restart...")
            code, out, err = run(client, install, timeout=120)
            print((out + err).strip())
            if code != 0:
                return code or 1

            # Simulate flag on (without full upgrade).
            flag = {
                "active": True,
                "phase": "installing",
                "progress": 42,
                "message": "Installing commit abc1234",
                "title": "Updating CPN Panel",
                "target": "abc1234",
                "source": "test",
                "started_at_unix": int(time.time()),
                "updated_at_unix": int(time.time()),
                "expires_at_unix": int(time.time()) + 3600,
                "bypass_token": "labtestbypasslabtestbypasslab12",
            }
            flag_json = json.dumps(flag, indent=2)
            write_flag = (
                "sudo tee /var/lib/cpn/maintenance.json >/dev/null <<'EOF'\n"
                + flag_json
                + "\nEOF\n"
                "sudo chmod 600 /var/lib/cpn/maintenance.json\n"
                "curl -sI http://127.0.0.1:2087/dashboard | head -8\n"
                "curl -s http://127.0.0.1:2087/api/panel-maintenance\n"
            )
            print(">>> flag ON smoke...")
            code, out, err = run(client, write_flag, timeout=60)
            print((out + err).strip()[:2000])
            if code != 0:
                return code or 1

            clear = (
                "sudo rm -f /var/lib/cpn/maintenance.json /var/lib/cpn/maintenance-page.html\n"
                "curl -sI http://127.0.0.1:2087/login | head -5\n"
                "curl -s http://127.0.0.1:2087/api/panel-maintenance\n"
            )
            print(">>> flag OFF smoke...")
            code, out, err = run(client, clear, timeout=60)
            print((out + err).strip()[:2000])
            return code
        finally:
            client.close()


if __name__ == "__main__":
    raise SystemExit(main())
