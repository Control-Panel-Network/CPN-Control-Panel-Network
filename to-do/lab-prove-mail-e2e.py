#!/usr/bin/env python3
import paramiko

SCRIPT = r"""#!/bin/bash
set -euo pipefail
echo LISTENERS:
ss -lntp | grep -E ':143|:587|:25' || true
user=cpnmailprobe
pass="CpnProbe!$(date +%s)"
marker="CPN-MAIL-PROBE-$pass"
id "$user" >/dev/null 2>&1 || sudo useradd -m -s /sbin/nologin "$user"
echo "$user:$pass" | sudo chpasswd
sudo mkdir -p /home/"$user"/Maildir/{new,cur,tmp}
sudo chown -R "$user:$user" /home/"$user"/Maildir
printf "From: cpn-probe@localhost\nTo: %s@localhost\nSubject: %s\n\n%s\n" "$user" "$marker" "$marker" | sudo sendmail -t || \
  printf "Subject: %s\n\n%s\n" "$marker" "$marker" | sudo sendmail "$user"
ok=0
for i in $(seq 1 20); do
  if sudo doveadm search -u "$user" mailbox INBOX SUBJECT "$marker" 2>/dev/null | grep -q .; then
    ok=1
    break
  fi
  if sudo ls /home/"$user"/Maildir/new/* >/dev/null 2>&1 && sudo grep -q "$marker" /home/"$user"/Maildir/new/*; then
    ok=1
    break
  fi
  sleep 1
done
if sudo doveadm auth test "$user" "$pass" >/dev/null; then
  echo AUTH_OK=yes
else
  echo AUTH_OK=no
fi
if [ "$ok" = 1 ]; then
  echo DELIVER_OK=yes
else
  echo DELIVER_OK=no
fi
sudo userdel -r "$user" >/dev/null 2>&1 || true
test "$ok" = 1
echo ISSUE9_LAB_OK=yes
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
with sftp.file("/tmp/cpn-mail-e2e.sh", "w") as handle:
    handle.write(SCRIPT)
sftp.chmod("/tmp/cpn-mail-e2e.sh", 0o755)
sftp.close()
_i, o, e = c.exec_command("bash /tmp/cpn-mail-e2e.sh", timeout=120, get_pty=True)
print((o.read() + e.read()).decode("utf-8", "replace"))
print("exit", o.channel.recv_exit_status())
c.close()
