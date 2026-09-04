#!/usr/bin/env python3
"""Smoke sticky sidebar + packages table overflow on AL9 panel (:2087)."""
from __future__ import annotations

import json
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

BASE = "http://127.0.0.1:2087"


def fetch(url: str, data: bytes | None = None, headers: dict | None = None, method: str = "GET"):
    req = urllib.request.Request(url, data=data, headers=headers or {}, method=method)
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            body = resp.read().decode("utf-8", "replace")
            return resp.status, dict(resp.headers), body
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", "replace")
        return exc.code, dict(exc.headers), body


def main() -> int:
    status, headers, login_html = fetch(f"{BASE}/login")
    if status >= 400:
        # Installer may serve login at /
        status, headers, login_html = fetch(f"{BASE}/")
    print(f"login_page status={status}", flush=True)

    # Prefer cookie session via demo/bootstrap if present; otherwise scrape form.
    token_m = re.search(r'name="token"\s+value="([^"]+)"', login_html)
    user_field = "username"
    if "name=\"user\"" in login_html:
        user_field = "user"

    # Try common lab credentials
    candidates = [
        {"username": "admin", "password": "CpnLab2026!"},
        {"username": "admin", "password": "Admin123!"},
    ]
    jar = ""
    packages_html = ""
    for creds in candidates:
        form = {
            user_field: creds["username"],
            "password": creds["password"],
        }
        if token_m:
            form["token"] = token_m.group(1)
        body = urllib.parse.urlencode(form).encode()
        st, hdrs, _ = fetch(
            f"{BASE}/login",
            data=body,
            headers={"Content-Type": "application/x-www-form-urlencoded"},
            method="POST",
        )
        set_cookie = hdrs.get("Set-Cookie") or hdrs.get("set-cookie") or ""
        if "cpn" in set_cookie.lower() or st in (302, 303):
            # Follow with cookie if any
            cookie = set_cookie.split(";")[0] if set_cookie else ""
            jar = cookie
            st2, _, packages_html = fetch(
                f"{BASE}/packages",
                headers={"Cookie": jar} if jar else {},
            )
            if st2 == 200 and ("Hosting Packages" in packages_html or "List Packages" in packages_html):
                print(f"packages ok via {creds['username']} status={st2}", flush=True)
                break
            packages_html = ""
    if not packages_html:
        # Last resort: open packages without auth to report
        st, _, packages_html = fetch(f"{BASE}/packages")
        print(f"packages unauth-or-failed status={st}", flush=True)

    checks = {
        "has_panel_layout": "panel-layout" in packages_html,
        "has_sidebar": 'class="sidebar"' in packages_html or "id=\"panel-sidebar\"" in packages_html,
        "main_scroll_css": "overflow-y:auto" in packages_html,
        "table_wrap": "table-wrap" in packages_html,
        "overflow_x_auto": "overflow-x:auto" in packages_html,
        "min_width_zero": "min-width:0" in packages_html,
        "locked_body": re.search(r"html,\s*body\s*\{[^}]*overflow:hidden", packages_html) is not None
        or "overflow:hidden" in packages_html,
    }
    print(json.dumps(checks, indent=2), flush=True)
    ok = all(checks.values())
    print("SMOKE_OK=yes" if ok else "SMOKE_OK=no", flush=True)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
