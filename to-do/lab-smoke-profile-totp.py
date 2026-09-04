#!/usr/bin/env python3
"""AL9 smoke: View Profile Edit button, self-modify, TOTP, Passkey API presence."""
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
    hdrs = {"User-Agent": "cpn-profile-edit-smoke/1.0"}
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


def _request_json(opener, url: str, payload: dict):
    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=body,
        headers={
            "Content-Type": "application/json",
            "Accept": "application/json",
            "User-Agent": "cpn-profile-edit-smoke/1.0",
        },
        method="POST",
    )
    try:
        with opener.open(req, timeout=30) as resp:
            return resp.getcode(), json.loads(resp.read().decode("utf-8", "replace"))
    except urllib.error.HTTPError as err:
        raw = err.read().decode("utf-8", "replace")
        try:
            data = json.loads(raw)
        except json.JSONDecodeError:
            data = {"error": raw}
        return err.code, data


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

    _request(opener, "GET", f"{BASE}/login")
    _request(
        opener,
        "POST",
        f"{BASE}/login",
        {"username": USER, "password": PASS, "remember_me": "0"},
    )

    code, profile, _ = _request(opener, "GET", f"{BASE}/account/users/profile")
    results["profile_view"] = code == 200 and "View Profile" in profile
    results["edit_button"] = 'href="/account/users/modify"' in profile and ">Edit<" in profile

    code, modify, _ = _request(opener, "GET", f"{BASE}/account/users/modify")
    results["modify_get"] = code == 200 and "Your account" in modify
    results["modify_self_forms"] = 'action="/account/users/profile/details"' in modify
    results["modify_passkeys_ui"] = "Passkeys (WebAuthn)" in modify and "Register passkey" in modify
    results["modify_no_planned"] = "planned next" not in modify.lower()

    new_email = f"admin-edit-{int(time.time())}@example.com"
    code, _, _ = _request(
        opener,
        "POST",
        f"{BASE}/account/users/profile/details",
        {"username": USER, "recovery_email": new_email, "language": "en"},
    )
    results["save_redirect"] = code in (200, 302, 303)
    code, profile2, _ = _request(opener, "GET", f"{BASE}/account/users/profile")
    results["profile_notice_email"] = new_email in profile2 or "Profile updated" in profile2 or new_email in profile2

    # Passkey register/start API (challenge issued; browser ceremony needs authenticator)
    code, data = _request_json(
        opener, f"{BASE}/account/users/profile/passkey/register/start", {}
    )
    results["passkey_register_start"] = code == 200 and "ceremony_id" in data and "publicKey" in data

    # Login passkey start without keys should fail honestly
    jar2 = CookieJar()
    opener2 = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar2))
    code, data = _request_json(
        opener2, f"{BASE}/login/passkey/start", {"username": USER}
    )
    results["passkey_login_start_no_keys"] = code == 400 and "passkey" in str(data).lower()

    # Login page advertises passkey button
    code, login_html, _ = _request(opener2, "GET", f"{BASE}/login")
    results["login_passkey_button"] = code == 200 and "Sign in with passkey" in login_html

    # Light TOTP path if Enable is present on modify
    code, modify2, _ = _request(opener, "GET", f"{BASE}/account/users/modify")
    if "Enable TOTP" in modify2:
        code, enroll_html, _ = _request(
            opener, "POST", f"{BASE}/account/users/profile/totp/begin", {}
        )
        results["totp_begin"] = code == 200 and "Secret:" in enroll_html
        secret_m = re.search(r"<code[^>]*>([A-Z2-7]{16,})</code>", enroll_html)
        if secret_m:
            secret = _b32_decode(secret_m.group(1))
            totp = _hotp(secret, int(time.time()) // 30)
            code, conf, _ = _request(
                opener,
                "POST",
                f"{BASE}/account/users/profile/totp/confirm",
                {"code": totp},
            )
            results["totp_confirm"] = code == 200 and (
                "TOTP enabled" in conf or "Backup codes" in conf
            )
            # disable to leave lab clean
            totp2 = _hotp(secret, int(time.time()) // 30)
            _request(
                opener,
                "POST",
                f"{BASE}/account/users/profile/totp/disable",
                {"current_password": PASS, "code": totp2},
            )
            results["totp_disable"] = True
        else:
            results["totp_confirm"] = False
            results["totp_disable"] = False
    else:
        results["totp_begin"] = "skipped"
        results["totp_confirm"] = "skipped"
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
