#!/usr/bin/env python3
"""Probe Email stack on CPN-AlmaLinux-10-install lab."""
import sys

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

try:
    from lab_ssh_util import connect_cpn_lab
except ImportError:
    print("lab_ssh_util missing", file=sys.stderr)
    sys.exit(2)

HOST = "127.0.0.1"
PORT = 2228
USER = "cpn"
PASSWORD = "CpnLab2026!"
SUDO = f"echo '{PASSWORD}' | sudo -S "

CMDS = [
    "rpm -q dovecot postfix dovecot-core dovecot-imapd 2>&1",
    "systemctl status dovecot postfix --no-pager 2>&1 | head -45",
    "systemctl is-enabled dovecot postfix 2>&1",
    "ls -la /usr/lib/systemd/system/*dovecot* 2>&1",
    "journalctl -u dovecot -n 80 --no-pager 2>&1",
    "systemctl enable --now dovecot 2>&1; echo enable_exit=$?",
    "ss -tlnp 2>&1 | grep -E ':(25|143|587) ' || true",
    "ls -la /var/lib/cpn/ 2>&1 | head -25",
    "find /var/lib/cpn -maxdepth 3 -iname '*mail*' 2>/dev/null | head -25",
    "dovecot -n 2>&1 | head -30",
]


def main() -> int:
    try:
        client = connect_cpn_lab(
            HOST, port=PORT, username=USER, password=PASSWORD, timeout=25
        )
    except Exception as exc:
        print(f"connect failed: {exc}", file=sys.stderr)
        return 1
    try:
        for cmd in CMDS:
            full = SUDO + cmd
            print(f"=== {cmd} ===")
            _stdin, stdout, stderr = client.exec_command(full, timeout=120)
            out = stdout.read().decode(errors="replace")
            err = stderr.read().decode(errors="replace")
            if out:
                print(out.rstrip())
            if err:
                print(err.rstrip(), file=sys.stderr)
            print()
    finally:
        client.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
