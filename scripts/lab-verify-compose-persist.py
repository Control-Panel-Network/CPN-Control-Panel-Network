#!/usr/bin/env python3
"""Verify compose pull/up keeps host data on AL10 install lab."""
from __future__ import annotations

import sys

import paramiko

HOST = "127.0.0.1"
PORT = 2228
USER = "cpn"
PASSWORD = "CpnLab2026!"
ROOT = "CpnPanelAdmin2026!"


def main() -> int:
    script = f"""set -euo pipefail
echo '{ROOT}' | sudo -S bash <<'EOS'
set -euo pipefail
if ! command -v docker >/dev/null 2>&1; then
  echo "RESULT=NO_DOCKER"
  exit 0
fi
STACK=lab-persist-test
PROJ=/var/lib/cpn/docker/$STACK
DATA=/var/lib/cpn/docker-data/$STACK/data
MARKER=$DATA/marker.txt
mkdir -p "$DATA" "$PROJ"
echo "marker-$(date +%s)" > "$MARKER"
if [ ! -f "$PROJ/compose.yml" ]; then
  cat > "$PROJ/compose.yml" <<'YML'
services:
  app:
    image: nginx:alpine
    restart: unless-stopped
    volumes:
      - type: bind
        source: /var/lib/cpn/docker-data/lab-persist-test/data
        target: /usr/share/nginx/html
    labels:
      com.cpn.managed: "1"
      com.cpn.stack: "lab-persist-test"
YML
fi
(cd "$PROJ" && docker compose -f compose.yml up -d)
(cd "$PROJ" && docker compose -f compose.yml pull)
(cd "$PROJ" && docker compose -f compose.yml up -d --remove-orphans)
echo "RESULT=MARKER:$(cat "$MARKER")"
EOS
"""
    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(
        HOST,
        port=PORT,
        username=USER,
        password=PASSWORD,
        timeout=30,
        allow_agent=False,
        look_for_keys=False,
    )
    stdin, stdout, stderr = client.exec_command("bash -s", timeout=600)
    stdin.write(script)
    stdin.channel.shutdown_write()
    out = stdout.read().decode("utf-8", "replace")
    err = stderr.read().decode("utf-8", "replace")
    code = stdout.channel.recv_exit_status()
    sys.stdout.buffer.write(out.encode("utf-8", "replace"))
    if err.strip():
        sys.stderr.buffer.write(err.encode("utf-8", "replace"))
    client.close()
    if "RESULT=MARKER:" in out:
        return 0
    return code or 1


if __name__ == "__main__":
    raise SystemExit(main())
