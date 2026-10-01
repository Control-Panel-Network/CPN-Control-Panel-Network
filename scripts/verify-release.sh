#!/usr/bin/env bash
# Verify a downloaded CPN release artifact before installing as root (issue #16).
# Usage:
#   ./scripts/verify-release.sh <artifact> [SHA256SUMS] [SHA256SUMS.asc]
#
# Env:
#   CPN_GPG_KEYRING   path to public key (default: packaging/RPM-GPG-KEY-CPN)
#   CPN_REQUIRE_GPG   if 1, require a valid detached signature on SHA256SUMS
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <artifact> [SHA256SUMS] [SHA256SUMS.asc]" >&2
  exit 2
fi

artifact="$1"
sums="${2:-SHA256SUMS}"
sums_asc="${3:-${sums}.asc}"
project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
keyring="${CPN_GPG_KEYRING:-$project_dir/packaging/RPM-GPG-KEY-CPN}"

[[ -f "$artifact" ]] || { echo "Missing artifact: $artifact" >&2; exit 1; }
[[ -f "$sums" ]] || { echo "Missing checksum file: $sums" >&2; exit 1; }

base="$(basename "$artifact")"
expected="$(awk -v f="$base" '$2 == f { print $1; exit }' "$sums")"
if [[ -z "$expected" ]]; then
  echo "No SHA-256 entry for $base in $sums" >&2
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$artifact" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$artifact" | awk '{print $1}')"
else
  echo "sha256sum or shasum required" >&2
  exit 1
fi

if [[ "$actual" != "$expected" ]]; then
  echo "SHA-256 mismatch for $base" >&2
  echo "  expected: $expected" >&2
  echo "  actual:   $actual" >&2
  exit 1
fi
echo "SHA-256 OK: $base"

if [[ -f "$sums_asc" ]]; then
  if ! command -v gpg >/dev/null 2>&1; then
    echo "gpg not found; cannot verify $sums_asc" >&2
    if [[ "${CPN_REQUIRE_GPG:-0}" == "1" ]]; then
      exit 1
    fi
  else
    GNUPGHOME="$(mktemp -d)"
    export GNUPGHOME
    chmod 700 "$GNUPGHOME"
    cleanup() { rm -rf "$GNUPGHOME"; }
    trap cleanup EXIT
    if [[ -f "$keyring" ]]; then
      gpg --batch --import "$keyring"
    fi
    gpg --batch --verify "$sums_asc" "$sums"
    echo "GPG OK: $sums_asc"
  fi
elif [[ "${CPN_REQUIRE_GPG:-0}" == "1" ]]; then
  echo "Missing $sums_asc and CPN_REQUIRE_GPG=1" >&2
  exit 1
fi

if [[ "$artifact" == *.rpm ]] && command -v rpm >/dev/null 2>&1; then
  if rpm --checksig "$artifact" 2>/dev/null | grep -qiE 'pgp|gpg|signatures OK|digests OK'; then
    echo "RPM signature check: $(rpm --checksig "$artifact" 2>&1 | tr '\n' ' ')"
  else
    # Import key into rpm db when possible, then re-check.
    if [[ -f "$keyring" ]] && command -v rpmkeys >/dev/null 2>&1; then
      rpmkeys --import "$keyring" 2>/dev/null || true
    fi
    if rpm --checksig "$artifact"; then
      echo "RPM --checksig OK: $artifact"
    else
      echo "Warning: rpm --checksig did not fully validate $artifact (detached .asc / SHA256SUMS still apply)." >&2
      if [[ "${CPN_REQUIRE_RPMSIGN:-0}" == "1" ]]; then
        exit 1
      fi
    fi
  fi
fi

echo "Verification passed for $artifact"
