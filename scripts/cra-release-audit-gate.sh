#!/usr/bin/env bash
# CRA Phase 2b release gate: fail on unwaived critical/high findings from
# cargo audit and npm audit (installer-ui + Panel).
#
# Waivers: to-do/cra/audit-waivers.json (ids only; no secrets).
# Usage:
#   ./scripts/cra-release-audit-gate.sh
#   CPN_AUDIT_GATE_SKIP_NPM=1 ./scripts/cra-release-audit-gate.sh
# Exit 0 = pass; 1 = fail.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

WAIVERS_FILE="${CPN_AUDIT_WAIVERS:-$ROOT/to-do/cra/audit-waivers.json}"
TMPDIR_GATE="${TMPDIR:-/tmp}/cpn-audit-gate-$$"
mkdir -p "$TMPDIR_GATE"
cleanup() { rm -rf "$TMPDIR_GATE"; }
trap cleanup EXIT

echo "== CRA release audit gate =="

if [[ ! -f "$WAIVERS_FILE" ]]; then
  echo "Creating empty waiver file at $WAIVERS_FILE"
  mkdir -p "$(dirname "$WAIVERS_FILE")"
  printf '%s\n' '{' \
    '  "schema_version": 1,' \
    '  "description": "Allowed critical/high advisory ids for the CRA release audit gate. Each entry needs id, reason, and risk_assessment_ref. No secrets.",' \
    '  "waivers": []' \
    '}' > "$WAIVERS_FILE"
fi

python3 - <<'PY' "$WAIVERS_FILE" "$TMPDIR_GATE/waivers.txt"
import json, sys
path, out = sys.argv[1], sys.argv[2]
with open(path, encoding="utf-8") as f:
    data = json.load(f)
ids = []
for row in data.get("waivers") or []:
    i = (row.get("id") or "").strip()
    if i:
        ids.append(i.upper())
with open(out, "w", encoding="utf-8") as f:
    f.write("\n".join(ids))
print(f"Loaded {len(ids)} waiver id(s) from {path}")
PY

FAIL=0

echo "-- cargo audit --"
if ! command -v cargo-audit >/dev/null 2>&1 && ! cargo audit -V >/dev/null 2>&1; then
  cargo install cargo-audit --locked
fi
set +e
cargo audit --json > "$TMPDIR_GATE/cargo-audit.json" 2>"$TMPDIR_GATE/cargo-audit.err"
CARGO_RC=$?
set -e
if [[ ! -s "$TMPDIR_GATE/cargo-audit.json" ]]; then
  echo "cargo audit did not produce JSON (exit $CARGO_RC):" >&2
  cat "$TMPDIR_GATE/cargo-audit.err" >&2 || true
  FAIL=1
else
  set +e
  python3 - <<'PY' "$TMPDIR_GATE/cargo-audit.json" "$TMPDIR_GATE/waivers.txt" "$TMPDIR_GATE/cargo-unwaived.txt"
import json, sys
path, waivers_path, out = sys.argv[1], sys.argv[2], sys.argv[3]
waived = {line.strip().upper() for line in open(waivers_path, encoding="utf-8") if line.strip()}
data = json.load(open(path, encoding="utf-8"))
vulns = []
block = data.get("vulnerabilities") or {}
if isinstance(block, dict):
    vulns = block.get("list") or block.get("found") or []
elif isinstance(block, list):
    vulns = block
unwaived = []
for v in vulns:
    advisory = v.get("advisory") if isinstance(v, dict) else None
    if not isinstance(advisory, dict):
        continue
    sev = (advisory.get("severity") or "").lower()
    if sev not in ("critical", "high"):
        continue
    rid = (advisory.get("id") or "").strip()
    if rid.upper() in waived:
        print(f"WAIVED cargo {rid} ({sev})")
        continue
    title = advisory.get("title") or ""
    unwaived.append(f"{rid}|{sev}|{title}")
open(out, "w", encoding="utf-8").write("\n".join(unwaived))
print(f"cargo critical/high unwaived: {len(unwaived)}")
sys.exit(1 if unwaived else 0)
PY
  CARGO_GATE_RC=$?
  set -e
  if [[ "$CARGO_GATE_RC" -ne 0 ]]; then
    echo "Unwaived cargo audit findings:" >&2
    cat "$TMPDIR_GATE/cargo-unwaived.txt" >&2 || true
    FAIL=1
  else
    echo "cargo audit gate: pass"
  fi
fi

npm_audit_tree() {
  local dir="$1"
  local label="$2"
  echo "-- npm audit ($label) --"
  (
    cd "$ROOT/$dir"
    if [[ ! -d node_modules ]]; then
      npm ci
    fi
    set +e
    npm audit --json --omit=dev > "$TMPDIR_GATE/npm-$label.json" 2>"$TMPDIR_GATE/npm-$label.err"
    local rc=$?
    set -e
    if [[ ! -s "$TMPDIR_GATE/npm-$label.json" ]]; then
      echo "npm audit ($label) produced no JSON (exit $rc):" >&2
      cat "$TMPDIR_GATE/npm-$label.err" >&2 || true
      exit 1
    fi
    python3 - <<PY "$TMPDIR_GATE/npm-$label.json" "$TMPDIR_GATE/waivers.txt" "$TMPDIR_GATE/npm-$label-unwaived.txt" "$label"
import json, sys
path, waivers_path, out, label = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
waived = {line.strip().upper() for line in open(waivers_path, encoding="utf-8") if line.strip()}
data = json.load(open(path, encoding="utf-8"))
unwaived = []
vuln_map = data.get("vulnerabilities") or {}
if isinstance(vuln_map, dict) and vuln_map:
    for name, meta in vuln_map.items():
        if not isinstance(meta, dict):
            continue
        sev = (meta.get("severity") or "").lower()
        if sev not in ("critical", "high"):
            continue
        via = meta.get("via") or []
        ids = []
        if isinstance(via, list):
            for item in via:
                if isinstance(item, dict):
                    for key in ("source", "url", "ghsa", "cve", "id"):
                        val = item.get(key)
                        if val is None:
                            continue
                        s = str(val)
                        if s.upper().startswith("GHSA-") or s.upper().startswith("CVE-"):
                            ids.append(s)
                        elif key == "url" and "GHSA-" in s.upper():
                            ids.append(s.rstrip("/").split("/")[-1])
                elif isinstance(item, str) and (item.upper().startswith("GHSA-") or item.upper().startswith("CVE-")):
                    ids.append(item)
        if not ids:
            ids = [f"npm:{name}"]
        if all(i.upper() in waived for i in ids):
            print(f"WAIVED npm/{label} {'/'.join(ids)} ({sev}) {name}")
            continue
        unwaived.append(f"{'|'.join(ids)}|{sev}|{name}")
else:
    adv_block = data.get("advisories")
    adv_iter = adv_block.values() if isinstance(adv_block, dict) else []
    for adv in adv_iter:
        if not isinstance(adv, dict):
            continue
        sev = (adv.get("severity") or "").lower()
        if sev not in ("critical", "high"):
            continue
        rid = str(adv.get("github_advisory_id") or adv.get("cve") or adv.get("id") or "").strip()
        if rid.upper() in waived:
            print(f"WAIVED npm/{label} {rid} ({sev})")
            continue
        unwaived.append(f"{rid}|{sev}|{adv.get('title') or ''}")
open(out, "w", encoding="utf-8").write("\n".join(unwaived))
print(f"npm/{label} critical/high unwaived: {len(unwaived)}")
sys.exit(1 if unwaived else 0)
PY
  )
}

if [[ "${CPN_AUDIT_GATE_SKIP_NPM:-0}" != "1" ]]; then
  if ! npm_audit_tree "installer-ui" "installer-ui"; then
    echo "Unwaived installer-ui npm findings:" >&2
    cat "$TMPDIR_GATE/npm-installer-ui-unwaived.txt" >&2 || true
    FAIL=1
  else
    echo "npm audit installer-ui gate: pass"
  fi
  if ! npm_audit_tree "Panel" "panel"; then
    echo "Unwaived Panel npm findings:" >&2
    cat "$TMPDIR_GATE/npm-panel-unwaived.txt" >&2 || true
    FAIL=1
  else
    echo "npm audit Panel gate: pass"
  fi
else
  echo "Skipping npm audit trees (CPN_AUDIT_GATE_SKIP_NPM=1)"
fi

if [[ "$FAIL" -ne 0 ]]; then
  echo "CRA release audit gate FAILED. Add justified ids to to-do/cra/audit-waivers.json and document in to-do/cra/risk-assessment.md, or fix the dependency." >&2
  exit 1
fi
echo "CRA release audit gate PASSED"
exit 0
