#!/usr/bin/env python3
"""Verify Email stack + DKIM + create page on CPN-AlmaLinux-10-install lab."""
import sys
import urllib.parse

try:
    from lab_ssh_util import connect_cpn_lab
except ImportError:
    print("lab_ssh_util missing", file=sys.stderr)
    sys.exit(2)

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

HOST = "127.0.0.1"
PORT = 2228
USER = "cpn"
PASSWORD = "CpnLab2026!"
PANEL = "http://127.0.0.1:2087"
SUDO = f"echo '{PASSWORD}' | sudo -S "


def main() -> int:
    client = connect_cpn_lab(
        HOST, port=PORT, username=USER, password=PASSWORD, timeout=25
    )
    cmds = [
        "systemctl is-active postfix dovecot cpn-installer",
        "ss -tlnp | grep -E ':(143|25) ' || true",
        "curl -sI -m 10 -o /dev/null -w 'email_accounts=%{http_code} time=%{time_total}\\n' "
        + PANEL
        + "/email/accounts",
        "curl -sI -m 10 -o /dev/null -w 'email_create=%{http_code} time=%{time_total}\\n' "
        + PANEL
        + "/email/create",
        "curl -sI -m 10 -o /dev/null -w 'email_dkim=%{http_code} time=%{time_total}\\n' "
        + PANEL
        + "/email/dkim",
        "ls -la /var/lib/cpn/dkim 2>/dev/null | head -15",
    ]
    for cmd in cmds:
        print("===", cmd[:70], "===")
        _i, o, e = client.exec_command(cmd, timeout=60)
        print(o.read().decode(errors="replace"))
        err = e.read().decode(errors="replace")
        if err.strip():
            print(err, file=sys.stderr)
    client.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
