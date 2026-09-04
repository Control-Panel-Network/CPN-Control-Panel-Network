#!/usr/bin/env python3
import paramiko

body = r"""#!/bin/bash
set -uo pipefail
pkill -f 'sleep 180' >/dev/null 2>&1 || true
setsid bash -c 'sleep 180 & sleep 180 & wait' >/tmp/cpnslow.out 2>&1 &
sleep 1
SPID=$(pgrep -n -f 'sleep 180')
PGID=$(ps -o pgid= -p "$SPID" | tr -d ' ')
echo SPID=$SPID
echo PGID=$PGID
BEFORE=$(ps -o pid= -g "$PGID" | wc -l | tr -d ' ')
echo BEFORE=$BEFORE
kill -TERM -"$PGID" 2>/dev/null || true
sleep 2
kill -KILL -"$PGID" 2>/dev/null || true
sleep 1
AFTER_COUNT=$(ps -o pid= -g "$PGID" 2>/dev/null | wc -l | tr -d ' ')
AFTER_COUNT=${AFTER_COUNT:-0}
echo AFTER=$AFTER_COUNT
REMAIN=$(pgrep -a -f 'sleep 180' || true)
if [ -n "$REMAIN" ]; then echo REMAINING=yes; echo "$REMAIN"; exit 1; fi
echo REMAINING=no
if [ "$AFTER_COUNT" != "0" ]; then exit 1; fi
exit 0
"""

c = paramiko.SSHClient()
c.set_missing_host_key_policy(paramiko.AutoAddPolicy())
c.connect(
    "127.0.0.1",
    port=2222,
    username="cpn",
    password="CpnLab2026!",
    timeout=20,
    allow_agent=False,
    look_for_keys=False,
)
sftp = c.open_sftp()
with sftp.file("/tmp/cpn-issue18b.sh", "w") as f:
    f.write(body)
sftp.chmod("/tmp/cpn-issue18b.sh", 0o755)
sftp.close()
_i, o, e = c.exec_command("sudo bash /tmp/cpn-issue18b.sh", timeout=60, get_pty=True)
print((o.read() + e.read()).decode("utf-8", "replace"))
print("exit", o.channel.recv_exit_status())
c.close()
