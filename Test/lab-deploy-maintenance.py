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

INCLUDE_DIRS = ("src", "sql", "docs", "packaging", "assets", "installer-ui")
INCLUDE_FILES = ("Cargo.toml", "Cargo.lock", "build.rs")


def ssh_password() -> str:
    return os.environ.get("CPN_LAB_SSH_PASSWORD", "")


def run(client, cmd: str, timeout: int = 600) -> tuple[int, str, str]:
    _stdin, stdout, stderr = client.exec_command(cmd, timeout=timeout)
    out = stdout.read().decode("utf-8", errors="replace")
    err = stderr.read().decode("utf-8", errors="replace")
    code = stdout.channel.recv_exit_status()
    return code, out, err


def add_tree(tar: tarfile.TarFile, rel: str) -> None:
    path = ROOT / rel
    if not path.exists():
        return
    if path.is_file():
        tar.add(path, arcname=rel.replace("\\", "/"))
        return
    for child in path.rglob("*"):
        if not child.is_file():
            continue
        parts = set(child.parts)
        if "target" in parts or "__pycache__" in parts or "node_modules" in parts or ".git" in parts:
            continue
        arc = str(child.relative_to(ROOT)).replace("\\", "/")
        tar.add(child, arcname=arc)


def main() -> int:
    password = ssh_password()
    if not password:
        print("Set CPN_LAB_SSH_PASSWORD", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory() as tmp:
        tar_path = Path(tmp) / "src.tar.gz"
        with tarfile.open(tar_path, "w:gz") as tar:
            for rel in INCLUDE_FILES:
                add_tree(tar, rel)
            for rel in INCLUDE_DIRS:
                add_tree(tar, rel)

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
                print(f">>> {cmd[:90]}...")
                code, out, err = run(client, cmd, timeout=1200)
                text = (out + err).strip()
                if text:
                    print("\n".join(text.splitlines()[-50:]))
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
                "echo '--- dashboard headers ---'\n"
                "curl -sI http://127.0.0.1:2087/dashboard | head -10\n"
                "echo '--- api ---'\n"
                "curl -s http://127.0.0.1:2087/api/panel-maintenance\n"
                "echo\n"
                "echo '--- page snippet ---'\n"
                "curl -s http://127.0.0.1:2087/maintenance | head -c 500; echo\n"
            )
            print(">>> flag ON smoke...")
            code, out, err = run(client, write_flag, timeout=60)
            print((out + err).strip()[:3000])
            if code != 0:
                return code or 1

            clear = (
                "sudo rm -f /var/lib/cpn/maintenance.json /var/lib/cpn/maintenance-page.html\n"
                "echo '--- login after clear ---'\n"
                "curl -sI http://127.0.0.1:2087/login | head -5\n"
                "echo '--- api ---'\n"
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
