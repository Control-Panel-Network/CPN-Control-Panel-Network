#!/usr/bin/env python3
"""Smoke sticky sidebar + packages table overflow on AL9 panel (:2087)."""
from __future__ import annotations

import http.cookiejar
import json
import urllib.error
import urllib.parse
import urllib.request

BASE = "http://127.0.0.1:2087"
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


def opener_with_login():
    jar = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
    body = urllib.parse.urlencode(
        {"username": "Admin", "password": panel_password()}
    ).encode()
    req = urllib.request.Request(
        f"{BASE}/login",
        data=body,
        method="POST",
        headers={"Content-Type": "application/x-www-form-urlencoded"},
    )
    try:
        with opener.open(req, timeout=20) as resp:
            login_status = resp.status
    except urllib.error.HTTPError as err:
        login_status = err.code
    return opener, login_status, bool(jar)


def fetch(opener, path: str):
    with opener.open(f"{BASE}{path}", timeout=20) as resp:
        return resp.status, resp.read().decode("utf-8", "replace")


def shell_checks(name: str, html: str) -> dict:
    return {
        f"{name}_panel_layout": "panel-layout" in html,
        f"{name}_sidebar": 'id="panel-sidebar"' in html or 'class="sidebar"' in html,
        f"{name}_main_scroll": "overflow-y:auto" in html,
        f"{name}_dvh_shell": "100dvh" in html,
        f"{name}_locked_body": "overflow:hidden" in html,
    }


def main() -> int:
    opener, login_status, has_cookie = opener_with_login()
    print(f"login_status={login_status} cookie={'yes' if has_cookie else 'no'}", flush=True)

    st, packages = fetch(opener, "/packages")
    print(f"packages status={st} bytes={len(packages)}", flush=True)
    st_w, websites = fetch(opener, "/websites")
    print(f"websites status={st_w} bytes={len(websites)}", flush=True)

    checks = {}
    checks.update(shell_checks("packages", packages))
    checks["packages_table_wrap"] = "table-wrap" in packages
    checks["packages_overflow_x"] = "overflow-x:auto" in packages
    checks["packages_min_width_zero"] = "min-width:0" in packages
    checks["packages_actions"] = (
        "Hosting Packages" in packages or "List Packages" in packages
    ) and ("Edit" in packages or "Actions" in packages)
    checks.update(shell_checks("websites", websites))
    checks["websites_table_wrap"] = "table-wrap" in websites

    print(json.dumps(checks, indent=2), flush=True)
    ok = all(checks.values())
    print("SMOKE_OK=yes" if ok else "SMOKE_OK=no", flush=True)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
