#!/usr/bin/env bash
# Optional cosign signing when COSIGN_KEY / COSIGN_PASSWORD are present (issue #16).
# Without secrets, exits 0 so CI can still publish checksums + provenance.
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <artifact> [more...]" >&2
  exit 2
fi

if [[ -z "${COSIGN_KEY:-}" ]]; then
  echo "COSIGN_KEY not set; skipping cosign signatures."
  echo "Keyless provenance is handled separately via actions/attest-build-provenance."
  exit 0
fi

if ! command -v cosign >/dev/null 2>&1; then
  echo "cosign binary not found" >&2
  exit 1
fi

KEY_FILE="$(mktemp)"
cleanup() { rm -f "$KEY_FILE"; }
trap cleanup EXIT
printf '%s\n' "$COSIGN_KEY" >"$KEY_FILE"
chmod 600 "$KEY_FILE"

export COSIGN_PASSWORD="${COSIGN_PASSWORD:-}"
for path in "$@"; do
  [[ -f "$path" ]] || { echo "Missing file: $path" >&2; exit 1; }
  cosign sign-blob --yes --key "$KEY_FILE" --output-signature "${path}.sig" "$path"
  echo "cosign signed $path -> ${path}.sig"
done
