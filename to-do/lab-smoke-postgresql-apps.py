#!/usr/bin/env python3
"""Smoke /apps PostgreSQL card on AL9 panel (detect + optional install)."""
from __future__ import annotations

import http.cookiejar
import re
import urllib.error
import urllib.parse
import urllib.request

import lab_ssh

BASE = "http://127.0.0.1:2087"
SSH_HOST = "127.0.0.1"
SSH_PORT = 2222


def ssh_bootstrap_user() -> str:
    client = lab_ssh.connect(host=SSH_HOST, port=SSH_PORT, password="CpnLab2026!")
    cmd = (
        "sudo python3 -c "
        "'import json; p=json.load(open(\"/var/lib/cpn/panel-bootstrap.json\")); "
        "print(p.get(\"username\",\"\"))'"
    )
    _stdin, stdout, stderr = client.exec_command(cmd)
    out = stdout.read().decode("utf-8", "replace").strip()
    err = stderr.read().decode("utf-8", "replace").strip()
    client.close()
    if not out:
        raise RuntimeError(f"Could not read bootstrap username: {err}")
    return out


def panel_password() -> str:
    path = r"D:\OneDrive - v-man\Priv\VirtualBox VMs\CPN-lab-credentials.txt"
    try:
        with open(path, encoding="utf-8") as handle:
            lines = handle.read().splitlines()
        for idx, line in enumerate(lines):
            if line.strip() == "Username: Admin" and idx + 1 < len(lines):
                nxt = lines[idx + 1].strip()
                if nxt.startswith("Password:"):
                    return nxt.split(":", 1)[1].strip()
    except OSError:
        pass
    raise RuntimeError("Panel password not found in CPN-lab-credentials.txt")


def ssh_app_list() -> str:
    client = lab_ssh.connect(host=SSH_HOST, port=SSH_PORT, password="CpnLab2026!")
    _stdin, stdout, stderr = client.exec_command("sudo cpn app list", timeout=60)
    out = stdout.read().decode("utf-8", "replace")
    err = stderr.read().decode("utf-8", "replace")
    code = stdout.channel.recv_exit_status()
    client.close()
    if code != 0:
        raise RuntimeError(f"cpn app list failed ({code}): {err or out}")
    return out


def main() -> int:
    user = ssh_bootstrap_user()
    password = panel_password()
    jar = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))

    login_body = urllib.parse.urlencode(
        {"username": user, "password": password}
    ).encode()
    req = urllib.request.Request(
        f"{BASE}/login",
        data=login_body,
        method="POST",
        headers={"Content-Type": "application/x-www-form-urlencoded"},
    )
    try:
        with opener.open(req, timeout=30) as resp:
            login_status = resp.status
    except urllib.error.HTTPError as exc:
        login_status = exc.code

    apps = opener.open(f"{BASE}/apps", timeout=30)
    apps_html = apps.read().decode("utf-8", "replace")
    cli_list = ssh_app_list()

    checks = {
        "login_ok": login_status in (200, 302, 303),
        "apps_has_postgresql_heading": "<h2>PostgreSQL</h2>" in apps_html,
        "apps_has_postgresql_id": "postgresql" in apps_html
        and re.search(r"<code>postgresql</code>", apps_html) is not None,
        "apps_has_install_or_controls": (
            'name="name" value="postgresql"' in apps_html
            or "action=\"/apps/install\"" in apps_html
        ),
        "apps_opt_in_note": "Opt-in only" in apps_html
        or "default stack remains MariaDB" in apps_html.lower()
        or "Default stack remains MariaDB" in apps_html,
        "cli_lists_postgresql": "postgresql\t" in cli_list
        or cli_list.startswith("postgresql")
        or any(line.startswith("postgresql") for line in cli_list.splitlines()),
        "no_cyberpanel_brand": "CyberPanel" not in apps_html,
    }

    print("login_status=", login_status, flush=True)
    print("--- cpn app list (excerpt) ---", flush=True)
    for line in cli_list.splitlines():
        if "postgres" in line.lower() or "mariadb" in line.lower():
            print(line, flush=True)
    print("--- checks ---", flush=True)
    failed = []
    for key, ok in checks.items():
        print(f"{key}={'PASS' if ok else 'FAIL'}", flush=True)
        if not ok:
            failed.append(key)

    if failed:
        print("SMOKE_FAIL", ",".join(failed), flush=True)
        return 1
    print("SMOKE_OK=postgresql-apps", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
