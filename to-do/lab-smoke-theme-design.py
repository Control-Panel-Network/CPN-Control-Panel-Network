#!/usr/bin/env python3
"""Smoke color-mode and Design APIs on AL9 panel after login."""
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
        with opener.open(req, timeout=15) as resp:
            login_status = resp.status
            login_url = resp.geturl()
    except urllib.error.HTTPError as exc:
        login_status = exc.code
        login_url = exc.geturl() if hasattr(exc, "geturl") else ""
        # Follow redirects manually is automatic for 302 usually

    dash = opener.open(f"{BASE}/dashboard", timeout=15)
    dash_html = dash.read().decode("utf-8", "replace")
    manage = opener.open(
        f"{BASE}/websites/manage?domain=cpn-lab-test.example", timeout=15
    )
    manage_html = manage.read().decode("utf-8", "replace")

    checks = {
        "login_user": user,
        "login_status": login_status,
        "login_url": login_url,
        "dashboard_has_toggle": "cpn-color-toggle" in dash_html,
        "dashboard_has_dark_css": 'data-color-mode="light"' in dash_html
        or 'data-color-mode="dark"' in dash_html,
        "manage_has_design": "cpn-design-open" in manage_html,
        "manage_no_cyberpanel": "cyberpanel" not in manage_html.lower(),
    }

    # Toggle color mode via API
    mode_req = urllib.request.Request(
        f"{BASE}/api/panel/color-mode",
        data=json.dumps({"color_mode": "dark"}).encode(),
        method="POST",
        headers={"Content-Type": "application/json", "Accept": "application/json"},
    )
    with opener.open(mode_req, timeout=15) as resp:
        mode_payload = json.loads(resp.read().decode())
    checks["color_mode_set"] = mode_payload

    # Save custom design then restore
    design_req = urllib.request.Request(
        f"{BASE}/api/panel/design",
        data=json.dumps(
            {
                "tokens": {
                    "accent": "#112233",
                    "accent_focus": "#445566",
                    "radius_px": 12,
                    "density": "compact",
                    "font_scale": 1.05,
                }
            }
        ).encode(),
        method="POST",
        headers={"Content-Type": "application/json", "Accept": "application/json"},
    )
    with opener.open(design_req, timeout=15) as resp:
        design_payload = json.loads(resp.read().decode())
    checks["design_custom"] = {
        "preset": design_payload.get("preset"),
        "accent": (design_payload.get("tokens") or {}).get("accent"),
    }

    restore_req = urllib.request.Request(
        f"{BASE}/api/panel/design/restore",
        data=b"{}",
        method="POST",
        headers={"Content-Type": "application/json", "Accept": "application/json"},
    )
    with opener.open(restore_req, timeout=15) as resp:
        restore_payload = json.loads(resp.read().decode())
    checks["design_restore"] = {
        "preset": restore_payload.get("preset"),
        "accent": (restore_payload.get("tokens") or {}).get("accent"),
        "has_custom": restore_payload.get("has_custom"),
    }

    # reset color mode to light
    light_req = urllib.request.Request(
        f"{BASE}/api/panel/color-mode",
        data=json.dumps({"color_mode": "light"}).encode(),
        method="POST",
        headers={"Content-Type": "application/json", "Accept": "application/json"},
    )
    opener.open(light_req, timeout=15).read()

    print(json.dumps(checks, indent=2))
    ok = (
        checks["dashboard_has_toggle"]
        and checks["manage_has_design"]
        and checks["manage_no_cyberpanel"]
        and mode_payload.get("color_mode") == "dark"
        and checks["design_custom"]["preset"] == "custom"
        and checks["design_restore"]["preset"] == "default"
        and checks["design_restore"]["accent"] == "#0066cc"
    )
    print("SMOKE_OK=" + ("yes" if ok else "no"))
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
