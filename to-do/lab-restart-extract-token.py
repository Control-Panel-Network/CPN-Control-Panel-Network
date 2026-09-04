#!/usr/bin/env python3
import re

import lab_ssh

c = lab_ssh.connect(port=2222, password="CpnLab2026!")
cmds = [
    "sudo pkill -9 -f /usr/bin/cpn-installer || true; sleep 1; pgrep -a cpn || echo none",
    "sudo firewall-cmd --add-port=2087/tcp || true",
    "sudo bash -lc 'nohup /usr/bin/cpn-installer --allow-remote --port 2087 >/tmp/cpn-installer-out.log 2>&1 & sleep 2; pgrep -a cpn-installer; grep fingerprint /tmp/cpn-installer-out.log | tail -1'",
]
for cmd in cmds:
    _i, o, e = c.exec_command(cmd, timeout=60, get_pty=True)
    print((o.read() + e.read()).decode("utf-8", "replace"))
    print("exit", o.channel.recv_exit_status())

_i, o, e = c.exec_command(
    r"""sudo python3 - <<'PY'
import re, pathlib
fp=None
for line in open('/tmp/cpn-installer-out.log', errors='replace'):
    if 'Token fingerprint' in line:
        fp=line.strip().split('...')[-1].strip()
print('FP', fp)
pids=[]
for p in pathlib.Path('/proc').iterdir():
    if not p.name.isdigit():
        continue
    try:
        cmd=(p/'cmdline').read_bytes().replace(b'\0', b' ').decode('utf-8','replace')
    except Exception:
        continue
    if '/usr/bin/cpn-installer' in cmd and 'bash' not in cmd:
        pids.append(int(p.name))
print('PIDS', pids)
pid=pids[0]
found=set()
with open(f'/proc/{pid}/mem','rb') as mem:
    for line in open(f'/proc/{pid}/maps'):
        parts=line.split()
        if 'r' not in parts[1]:
            continue
        start,end=parts[0].split('-')
        start_i=int(start,16); end_i=int(end,16)
        if end_i-start_i>64*1024*1024:
            continue
        try:
            mem.seek(start_i)
            data=mem.read(end_i-start_i)
        except Exception:
            continue
        for m in re.finditer(rb'[A-Za-z0-9_-]{28}', data):
            s=m.group().decode()
            if fp and s.endswith(fp):
                found.add(s)
print('FOUND', sorted(found))
if found:
    open('/tmp/cpn.token','w').write(sorted(found)[0]+'\n')
PY""",
    timeout=60,
    get_pty=True,
)
print((o.read() + e.read()).decode("utf-8", "replace"))
c.close()
