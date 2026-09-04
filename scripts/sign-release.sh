#!/usr/bin/env bash
# Optional GPG signing for release artifacts (issue #16).
# Required secrets when signing is enabled:
#   GPG_PRIVATE_KEY  armored private key
#   GPG_PASSPHRASE   passphrase (may be empty)
#   GPG_KEY_ID       optional key id / fingerprint
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <artifact-or-SHA256SUMS> [more files...]" >&2
  exit 2
fi

if [[ -z "${GPG_PRIVATE_KEY:-}" ]]; then
  if [[ "${CPN_REQUIRE_GPG:-0}" == "1" ]]; then
    echo "GPG_PRIVATE_KEY required for official signed releases (issue #16)." >&2
    exit 1
  fi
  echo "GPG_PRIVATE_KEY not set; skipping GPG signatures (checksums still required)."
  exit 0
fi

GNUPGHOME="$(mktemp -d)"
export GNUPGHOME
chmod 700 "$GNUPGHOME"
cleanup() { rm -rf "$GNUPGHOME"; }
trap cleanup EXIT

printf '%s\n' "$GPG_PRIVATE_KEY" | gpg --batch --import
KEY_ID="${GPG_KEY_ID:-$(gpg --list-secret-keys --with-colons | awk -F: '/^sec:/ {print $5; exit}')}"
if [[ -z "$KEY_ID" ]]; then
  echo "Could not determine GPG key id" >&2
  exit 1
fi

sign_one() {
  local file="$1"
  local args=(--batch --yes --detach-sign --armor --local-user "$KEY_ID")
  if [[ -n "${GPG_PASSPHRASE:-}" ]]; then
    args+=(--pinentry-mode loopback --passphrase "$GPG_PASSPHRASE")
  fi
  gpg "${args[@]}" --output "${file}.asc" "$file"
  echo "Signed $file -> ${file}.asc"
}

for path in "$@"; do
  [[ -f "$path" ]] || { echo "Missing file: $path" >&2; exit 1; }
  sign_one "$path"
done
