#!/usr/bin/env bash
# Write cpn-security-release.json for a GitHub Release (CRA Phase 2b metadata).
# Prefer this machine-readable flag over release title/tag keyword heuristics in auto-update.
#
# Env:
#   CPN_SECURITY_RELEASE=true|false   (required intent; default false)
#   CPN_RELEASE_TAG                   (e.g. v1.4.9)
#   CPN_RELEASE_VERSION               (optional; derived from tag)
#   CPN_SBOM_OUT / OUT dir            (default dist/)
#   CPN_SECURITY_ADVISORIES_JSON      (optional JSON array string of advisory ids)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${CPN_SBOM_OUT:-$ROOT/dist}"
mkdir -p "$OUT"

FLAG_RAW="${CPN_SECURITY_RELEASE:-false}"
FLAG_LC="$(printf '%s' "$FLAG_RAW" | tr '[:upper:]' '[:lower:]')"
case "$FLAG_LC" in
  1|true|yes|on) SECURITY=true ;;
  0|false|no|off|"") SECURITY=false ;;
  *)
    echo "Invalid CPN_SECURITY_RELEASE='$FLAG_RAW' (use true/false)" >&2
    exit 1
    ;;
esac

TAG="${CPN_RELEASE_TAG:-}"
if [[ -z "$TAG" ]]; then
  if [[ "${GITHUB_REF:-}" == refs/tags/* ]]; then
    TAG="${GITHUB_REF#refs/tags/}"
  else
    TAG="untagged"
  fi
fi
VERSION="${CPN_RELEASE_VERSION:-${TAG#v}}"
ADVISORIES_JSON="${CPN_SECURITY_ADVISORIES_JSON:-[]}"

export CPN_META_OUT="$OUT"
export CPN_META_SECURITY="$SECURITY"
export CPN_META_TAG="$TAG"
export CPN_META_VERSION="$VERSION"
export CPN_META_ADVISORIES="$ADVISORIES_JSON"
python3 - <<'PY'
import json, os, sys
from datetime import datetime, timezone

out_dir = os.environ["CPN_META_OUT"]
security = os.environ["CPN_META_SECURITY"] == "true"
tag = os.environ["CPN_META_TAG"]
version = os.environ["CPN_META_VERSION"]
try:
    advisories = json.loads(os.environ.get("CPN_META_ADVISORIES") or "[]")
except json.JSONDecodeError as exc:
    print(f"Invalid CPN_SECURITY_ADVISORIES_JSON: {exc}", file=sys.stderr)
    sys.exit(1)
if not isinstance(advisories, list):
    print("CPN_SECURITY_ADVISORIES_JSON must be a JSON array", file=sys.stderr)
    sys.exit(1)

meta = {
    "schema_version": 1,
    "product": "CPN",
    "product_name": "CPN Control Panel Network",
    "tag": tag,
    "version": version,
    "security_release": security,
    "free_security_updates": True,
    "support_period_note": (
        "Security updates are provided free of charge during the published "
        "support period for the latest two releases. See docs/SUPPORT.md and "
        "to-do/cra/support-period.md."
    ),
    "advisories": advisories,
    "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "detection": {
        "body_marker": "CPN-SECURITY-RELEASE: true|false",
        "asset_name": "cpn-security-release.json",
        "security_marker_asset": "CPN-SECURITY-RELEASE",
    },
}
path = os.path.join(out_dir, "cpn-security-release.json")
with open(path, "w", encoding="utf-8") as f:
    json.dump(meta, f, indent=2, ensure_ascii=False)
    f.write("\n")
print(f"Wrote {path} security_release={security}")

marker = os.path.join(out_dir, "CPN-SECURITY-RELEASE")
if security:
    with open(marker, "w", encoding="utf-8") as f:
        f.write("true\n")
    print(f"Wrote security marker {marker}")
elif os.path.exists(marker):
    os.remove(marker)
PY

MARKER_FILE="$OUT/cpn-security-release.body-marker.txt"
printf 'CPN-SECURITY-RELEASE: %s\n' "$SECURITY" > "$MARKER_FILE"
echo "Body marker: $(cat "$MARKER_FILE")"
