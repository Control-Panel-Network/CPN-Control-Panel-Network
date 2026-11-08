#!/usr/bin/env bash
# Generate CycloneDX SBOMs for Cargo (installer) plus installer-ui and Panel npm trees.
# Output files land under dist/ (or CPN_SBOM_OUT). Used by .github/workflows/release.yml.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${CPN_SBOM_OUT:-$ROOT/dist}"
mkdir -p "$OUT"

echo "Generating Cargo CycloneDX SBOM..."
if ! command -v cargo-cyclonedx >/dev/null 2>&1 && ! cargo cyclonedx -V >/dev/null 2>&1; then
  cargo install cargo-cyclonedx --locked
fi
(
  cd "$ROOT"
  cargo cyclonedx -f json --override-filename cpn-installer.cdx
  mv -f cpn-installer.cdx.json "$OUT/cpn-installer.cdx.json"
)
test -s "$OUT/cpn-installer.cdx.json"
! grep -q 'SBOM generation skipped' "$OUT/cpn-installer.cdx.json"

generate_npm_sbom() {
  local dir="$1"
  local out_name="$2"
  local abs="$ROOT/$dir"
  if [[ ! -f "$abs/package-lock.json" ]]; then
    echo "Missing $dir/package-lock.json" >&2
    exit 1
  fi
  echo "Generating npm CycloneDX SBOM for $dir -> $out_name ..."
  (
    cd "$abs"
    if [[ ! -d node_modules ]]; then
      npm ci
    fi
    npx --yes @cyclonedx/cyclonedx-npm@4.0.0 \
      --output-file "$OUT/$out_name" \
      --output-reproducible \
      --spec-version 1.5
  )
  test -s "$OUT/$out_name"
  python3 - <<PY
import json, sys
path = r"""$OUT/$out_name"""
with open(path, encoding="utf-8") as f:
    data = json.load(f)
bom = data.get("bomFormat") or data.get("bomformat")
if bom != "CycloneDX":
    print(f"{path}: expected bomFormat CycloneDX, got {bom!r}", file=sys.stderr)
    sys.exit(1)
print(f"OK {path} components={len(data.get('components') or [])}")
PY
}

generate_npm_sbom "installer-ui" "cpn-installer-ui.cdx.json"
generate_npm_sbom "Panel" "cpn-panel.cdx.json"

echo "SBOMs written:"
ls -la "$OUT"/cpn-installer.cdx.json "$OUT"/cpn-installer-ui.cdx.json "$OUT"/cpn-panel.cdx.json
