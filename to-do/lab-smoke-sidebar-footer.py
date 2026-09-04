#!/usr/bin/env python3
"""Smoke sidebar footer actions on AL9 panel :2087."""
from __future__ import annotations

import http.cookiejar
import json
import urllib.error
import urllib.parse
import urllib.request

import lab_ssh

BASE = "http://127.0.0.1:2087"
SSH_HOST = "127.0.0.1"
SSH_PORT = 2222
CREDS_PATH = r"D:\OneDrive - v-man\Priv\VirtualBox VMs\CPN-lab-credentials.txt"


def panel_password() -> str:
    with open(CREDS_PATH, encoding="utf-8") as handle:
        lines = handle.readlines()
    for idx, line in enumerate(lines):
        if line.strip() == "Username: Admin" and idx + 1 < len(lines):
            nxt = lines[idx + 1].strip()
            if nxt.startswith("Password:"):
                return nxt.split(":", 1)[1].strip()
    raise RuntimeError("Panel password not found in CPN-lab-credentials.txt")


def main() -> int:
    client = lab_ssh.connect(host=SSH_HOST, port=SSH_PORT, password="CpnLab2026!")
    _stdin, stdout, _stderr = client.exec_command(
        "curl -sI --max-time 5 http://127.0.0.1:2087/login | head -5; "
        "cd /home/cpn/CPN-Control-Panel-Network && git log -1 --oneline",
        timeout=30,
    )
    print(stdout.read().decode("utf-8", "replace"), flush=True)
    client.close()

    user = "Admin"
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
        with opener.open(req, timeout=20) as resp:
            login_status = resp.status
            login_url = resp.geturl()
    except urllib.error.HTTPError as err:
        login_status = err.code
        login_url = err.geturl()

    with opener.open(f"{BASE}/dashboard", timeout=20) as resp:
        html = resp.read().decode("utf-8", "replace")
        dash_status = resp.status

    push_req = urllib.request.Request(
        f"{BASE}/api/panel/notifications",
        data=json.dumps(
            {
                "title": "SSL renewed",
                "body": "Smoke test certificate renewal notice.",
                "category": "ssl",
            }
        ).encode(),
        method="POST",
        headers={
            "Content-Type": "application/json",
            "Accept": "application/json",
        },
    )
    with opener.open(push_req, timeout=20) as resp:
        push_payload = json.loads(resp.read().decode("utf-8", "replace"))
        push_status = resp.status

    with opener.open(f"{BASE}/api/panel/notifications", timeout=20) as resp:
        list_payload = json.loads(resp.read().decode("utf-8", "replace"))
        list_status = resp.status

    with opener.open(f"{BASE}/account/users/profile", timeout=20) as resp:
        profile_status = resp.status
        profile_html = resp.read().decode("utf-8", "replace")

    checks = {
        "login_status": login_status,
        "login_url": login_url,
        "dashboard_status": dash_status,
        "has_notify_btn": 'id="cpn-notify-btn"' in html,
        "has_notify_panel": 'id="cpn-notify-panel"' in html,
        "has_settings_gear": 'href="/account/users/profile"' in html
        and "Account settings" in html,
        "has_theme_toggle": 'id="cpn-color-toggle"' in html,
        "no_theme_text_label": "Light mode" not in html and "Dark mode" not in html,
        "has_logout_right": ">Log out</a>" in html,
        "has_footer_actions": "sidebar-footer-actions" in html,
        "has_collapse_btn": 'id="sidebar-collapse-btn"' in html,
        "push_status": push_status,
        "push_ok": bool(push_payload.get("ok")),
        "list_status": list_status,
        "list_unread": list_payload.get("unread_count", 0) >= 1,
        "profile_status": profile_status,
        "profile_ok": "View Profile" in profile_html or "Username" in profile_html,
        "no_cyberpanel_brand": "CyberPanel" not in html,
    }
    print(json.dumps(checks, indent=2), flush=True)
    failed = [
        key
        for key, value in checks.items()
        if key
        in {
            "has_notify_btn",
            "has_notify_panel",
            "has_settings_gear",
            "has_theme_toggle",
            "no_theme_text_label",
            "has_logout_right",
            "has_footer_actions",
            "push_ok",
            "list_unread",
            "profile_ok",
            "no_cyberpanel_brand",
        }
        and not value
    ]
    if dash_status != 200 or list_status != 200 or push_status != 200:
        failed.append("http_status")
    if failed:
        print("SMOKE_FAIL", failed, flush=True)
        return 1
    print("SMOKE_OK", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
