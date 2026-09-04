#!/usr/bin/env python3
"""AL9 browser-ish smoke: edit own profile email/password and TOTP login challenge."""
from __future__ import annotations

import hashlib
import hmac
import json
import re
import struct
import time
import urllib.error
import urllib.parse
import urllib.request
from http.cookiejar import CookieJar

BASE = "http://127.0.0.1:2087"
USER = "Admin"


def panel_password() -> str:
    path = r"D:\OneDrive - v-man\Priv\VirtualBox VMs\CPN-lab-credentials.txt"
    lines = open(path, encoding="utf-8").read().splitlines()
    for index, line in enumerate(lines):
        if line.strip() == "Username: Admin" and index + 1 < len(lines):
            nxt = lines[index + 1]
            if nxt.lower().startswith("password:"):
                return nxt.split(":", 1)[1].strip()
    raise RuntimeError("Panel password not found in CPN-lab-credentials.txt")


PASS = panel_password()


def _request(opener, method: str, url: str, data: dict | None = None, headers: dict | None = None):
    body = None
    hdrs = {"User-Agent": "cpn-profile-totp-smoke/1.0"}
    if headers:
        hdrs.update(headers)
    if data is not None:
        body = urllib.parse.urlencode(data).encode("utf-8")
        hdrs["Content-Type"] = "application/x-www-form-urlencoded"
    req = urllib.request.Request(url, data=body, headers=hdrs, method=method)
    try:
        with opener.open(req, timeout=30) as resp:
            return resp.getcode(), resp.read().decode("utf-8", "replace"), dict(resp.headers)
    except urllib.error.HTTPError as err:
        return err.code, err.read().decode("utf-8", "replace"), dict(err.headers)


def _hotp(secret: bytes, counter: int) -> str:
    msg = struct.pack(">Q", counter)
    dig = hmac.new(secret, msg, hashlib.sha1).digest()
    offset = dig[-1] & 0x0F
    code = (
        ((dig[offset] & 0x7F) << 24)
        | ((dig[offset + 1] & 0xFF) << 16)
        | ((dig[offset + 2] & 0xFF) << 8)
        | (dig[offset + 3] & 0xFF)
    )
    return f"{code % 1_000_000:06d}"


def _b32_decode(value: str) -> bytes:
    alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
    cleaned = "".join(ch for ch in value.upper() if ch in alphabet)
    bits = "".join(f"{alphabet.index(ch):05b}" for ch in cleaned)
    out = bytearray()
    for i in range(0, len(bits) - 7, 8):
        out.append(int(bits[i : i + 8], 2))
    return bytes(out)


def main() -> int:
    jar = CookieJar()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
    results = {}

    code, html, _ = _request(opener, "GET", f"{BASE}/login")
    results["login_get"] = code == 200

    code, _, _ = _request(
        opener,
        "POST",
        f"{BASE}/login",
        {"username": USER, "password": PASS, "remember_me": "0"},
    )
    # 303 to dashboard or 2fa
    results["login_post"] = code in (200, 302, 303)

    code, profile, _ = _request(opener, "GET", f"{BASE}/account/users/profile")
    results["profile_get"] = code == 200 and "Account profile" in profile
    results["profile_editable"] = 'action="/account/users/profile/details"' in profile
    results["profile_password_form"] = 'action="/account/users/profile/password"' in profile
    results["profile_totp"] = "Two-factor authentication" in profile

    new_email = f"admin-smoke-{int(time.time())}@example.com"
    code, _, _ = _request(
        opener,
        "POST",
        f"{BASE}/account/users/profile/details",
        {"username": USER, "recovery_email": new_email, "language": "en"},
    )
    results["email_update"] = code in (200, 302, 303)

    code, profile2, _ = _request(opener, "GET", f"{BASE}/account/users/profile")
    results["email_saved"] = new_email in profile2

    # Change password then change back
    new_pass = "SmokePass9!"
    code, _, _ = _request(
        opener,
        "POST",
        f"{BASE}/account/users/profile/password",
        {
            "current_password": PASS,
            "password": new_pass,
            "generate": "0",
        },
    )
    results["password_update"] = code in (200, 302, 303)

    # Re-login with new password
    jar.clear()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
    code, _, _ = _request(
        opener,
        "POST",
        f"{BASE}/login",
        {"username": USER, "password": new_pass, "remember_me": "0"},
    )
    results["login_new_password"] = code in (200, 302, 303)

    code, profile3, _ = _request(opener, "GET", f"{BASE}/account/users/profile")
    results["session_after_pw"] = code == 200 and "Account profile" in profile3

    # Enable TOTP if disabled
    if "Enable TOTP" in profile3:
        code, enroll_html, _ = _request(
            opener, "POST", f"{BASE}/account/users/profile/totp/begin", {}
        )
        results["totp_begin"] = code == 200 and "Secret:" in enroll_html
        secret_m = re.search(r"<code[^>]*>([A-Z2-7]{16,})</code>", enroll_html)
        if secret_m:
            secret_b32 = secret_m.group(1)
            secret = _b32_decode(secret_b32)
            totp = _hotp(secret, int(time.time()) // 30)
            code, conf_html, _ = _request(
                opener,
                "POST",
                f"{BASE}/account/users/profile/totp/confirm",
                {"code": totp},
            )
            results["totp_confirm"] = code == 200 and (
                "TOTP enabled" in conf_html or "Backup codes" in conf_html
            )
            # Restore original password before logout challenge
            _request(
                opener,
                "POST",
                f"{BASE}/account/users/profile/password",
                {
                    "current_password": new_pass,
                    "password": PASS,
                    "generate": "0",
                },
            )
            # Logout and login should challenge
            _request(opener, "GET", f"{BASE}/logout")
            jar.clear()
            opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
            code, loc_html, headers = _request(
                opener,
                "POST",
                f"{BASE}/login",
                {"username": USER, "password": PASS, "remember_me": "0"},
            )
            # Follow if needed
            code2, mfa_html, _ = _request(opener, "GET", f"{BASE}/login/2fa")
            results["totp_challenge"] = code2 == 200 and "Two-factor" in mfa_html
            totp2 = _hotp(secret, int(time.time()) // 30)
            code3, _, _ = _request(
                opener, "POST", f"{BASE}/login/2fa", {"code": totp2}
            )
            results["totp_login"] = code3 in (200, 302, 303)
            code4, dash, _ = _request(opener, "GET", f"{BASE}/dashboard")
            results["dashboard_after_totp"] = code4 == 200 and (
                "Dashboard" in dash or "CPN" in dash
            )
            # Disable TOTP to leave lab clean
            code5, prof, _ = _request(opener, "GET", f"{BASE}/account/users/profile")
            if "Disable TOTP" in prof:
                totp3 = _hotp(secret, int(time.time()) // 30)
                _request(
                    opener,
                    "POST",
                    f"{BASE}/account/users/profile/totp/disable",
                    {"current_password": PASS, "code": totp3},
                )
                results["totp_disable"] = True
            else:
                results["totp_disable"] = False
        else:
            results["totp_begin"] = False
            results["totp_confirm"] = False
            results["totp_challenge"] = False
            results["totp_login"] = False
            results["dashboard_after_totp"] = False
            results["totp_disable"] = False
            # restore password best-effort
            _request(
                opener,
                "POST",
                f"{BASE}/account/users/profile/password",
                {"current_password": new_pass, "password": PASS, "generate": "0"},
            )
    else:
        # Already enabled or unexpected; restore password
        _request(
            opener,
            "POST",
            f"{BASE}/account/users/profile/password",
            {"current_password": new_pass, "password": PASS, "generate": "0"},
        )
        results["totp_begin"] = "skipped_already_enabled"
        results["totp_confirm"] = "skipped"
        results["totp_challenge"] = "skipped"
        results["totp_login"] = "skipped"
        results["dashboard_after_totp"] = "skipped"
        results["totp_disable"] = "skipped"

    print(json.dumps(results, indent=2, ensure_ascii=False))
    failed = [k for k, v in results.items() if v is False]
    if failed:
        print("FAILED:", ", ".join(failed))
        return 1
    print("SMOKE_OK=yes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
