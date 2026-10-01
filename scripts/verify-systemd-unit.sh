#!/usr/bin/env bash
# Verify packaging/cpn-installer.service directive placement (EL9/systemd-oriented).
# StartLimitIntervalSec / StartLimitBurst must live under [Unit], not [Service].
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UNIT="${1:-$ROOT/packaging/cpn-installer.service}"

if [[ ! -f "$UNIT" ]]; then
  echo "unit file missing: $UNIT" >&2
  exit 1
fi

python3 - "$UNIT" <<'PY'
import sys

path = sys.argv[1]
text = open(path, encoding="utf-8").read()
sections = {}
current = None
for raw in text.splitlines():
    line = raw.strip()
    if not line or line.startswith("#"):
        continue
    if line.startswith("[") and line.endswith("]"):
        current = line[1:-1]
        sections.setdefault(current, [])
        continue
    if current is None:
        continue
    key = line.split("=", 1)[0].strip()
    sections[current].append(key)

unit_keys = sections.get("Unit", [])
service_keys = sections.get("Service", [])
for key in ("StartLimitIntervalSec", "StartLimitBurst"):
    if key not in unit_keys:
        print(f"ERROR: {key} must be under [Unit]", file=sys.stderr)
        sys.exit(1)
    if key in service_keys:
        print(f"ERROR: {key} must not be under [Service]", file=sys.stderr)
        sys.exit(1)

print("[OK] StartLimit* directives are under [Unit]")
PY

if ! command -v systemd-analyze >/dev/null 2>&1; then
  echo "[WARN] systemd-analyze not installed; skipped verify (install systemd for EL9/CI parity)"
  exit 0
fi

# Copy into a local tmpfs path; some environments reject analyze on remote/OneDrive mounts.
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT
tmp="$tmp_dir/cpn-installer.service"
# Point ExecStart at an existing binary so verify does not fail on a missing package path.
sed 's|^ExecStart=.*|ExecStart=/bin/true|' "$UNIT" >"$tmp"
chmod 644 "$tmp"

set +e
out="$(systemd-analyze verify "$tmp" 2>&1)"
rc=$?
set -e

if [[ $rc -ne 0 ]]; then
  # WSL and some containers cannot prepare unit filenames; placement check above still passed.
  if echo "$out" | grep -Eiq 'Failed to prepare filename|No such file or directory|Invalid argument'; then
    echo "[WARN] systemd-analyze verify unavailable in this environment:"
    echo "$out" | head -n 5
    echo "[OK] StartLimit placement validated without systemd-analyze"
    exit 0
  fi
  echo "$out" >&2
  echo "ERROR: systemd-analyze verify failed for $UNIT" >&2
  exit 1
fi

if echo "$out" | grep -Eiq 'Unknown key.*StartLimit|StartLimit.*(Service|ignored)'; then
  echo "$out" >&2
  echo "ERROR: systemd reports StartLimit directive problems" >&2
  exit 1
fi
echo "[OK] systemd-analyze verify passed for cpn-installer.service"
