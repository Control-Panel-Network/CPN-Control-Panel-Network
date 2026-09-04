#!/usr/bin/env python3
"""Smoke sticky sidebar + packages/email/websites mobile CSS on AL9 panel (:2087)."""
from __future__ import annotations

import json
import re
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


def login_cookie() -> str:
    status, _headers, login_html = fetch(f"{BASE}/login")
    if status >= 400:
        _status, _headers, login_html = fetch(f"{BASE}/")
    token_m = re.search(r'name="token"\s+value="([^"]+)"', login_html)
    user_field = "user" if 'name="user"' in login_html else "username"
    candidates = [
        {"username": "admin", "password": "CpnLab2026!"},
        {"username": "admin", "password": "Admin123!"},
    ]
    for creds in candidates:
        form = {user_field: creds["username"], "password": creds["password"]}
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
            return set_cookie.split(";")[0] if set_cookie else ""
    return ""


def shell_checks(name: str, html: str) -> dict:
    return {
        f"{name}_panel_layout": "panel-layout" in html,
        f"{name}_sidebar": 'id="panel-sidebar"' in html or 'class="sidebar"' in html,
        f"{name}_main_scroll": "overflow-y:auto" in html,
        f"{name}_dvh_shell": "100dvh" in html,
        f"{name}_locked_body": "overflow:hidden" in html,
    }


def main() -> int:
    jar = login_cookie()
    headers = {"Cookie": jar} if jar else {}
    print(f"cookie={'yes' if jar else 'no'}", flush=True)

    pages = {
        "packages": "/packages",
        "email_hub": "/email",
        "websites": "/websites",
    }
    all_checks: dict = {}
    for name, path in pages.items():
        st, _, html = fetch(f"{BASE}{path}", headers=headers)
        print(f"{name} status={st} bytes={len(html)}", flush=True)
        all_checks.update(shell_checks(name, html))
        if name == "packages":
            all_checks["packages_table_wrap"] = "table-wrap" in html
            all_checks["packages_actions"] = (
                "Edit" in html or "Hosting Packages" in html or "List Packages" in html
            )
            all_checks["packages_sticky_cols"] = "position:sticky" in html and "right:0" in html
        if name == "email_hub":
            all_checks["email_hub_tiles"] = "hub-tile" in html or "Email" in html
            all_checks["email_hub_minmax"] = "minmax(min(100%" in html or "hub-tile-grid" in html
        if name == "websites":
            all_checks["websites_shell"] = "Websites" in html or "website" in html.lower()

    print(json.dumps(all_checks, indent=2), flush=True)
    ok = all(all_checks.values())
    print("SMOKE_OK=yes" if ok else "SMOKE_OK=no", flush=True)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
