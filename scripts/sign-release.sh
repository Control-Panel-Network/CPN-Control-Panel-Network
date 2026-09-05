#!/usr/bin/env bash
# GPG sign release artifacts (issue #16).
# Required for official tag releases (CPN_REQUIRE_GPG=1):
#   GPG_PRIVATE_KEY  armored private key
#   GPG_PASSPHRASE   passphrase (may be empty)
#   GPG_KEY_ID       optional key id / fingerprint
#
# When CPN_RPMSIGN=1 and the target is a .rpm, also embeds an RPM package
# signature via rpmsign --addsign (requires rpm-sign / rpmsign on PATH).
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

# Required for non-interactive CI signing (loopback passphrase).
printf '%s\n' 'allow-loopback-pinentry' >"$GNUPGHOME/gpg-agent.conf"
printf '%s\n' 'pinentry-mode loopback' >"$GNUPGHOME/gpg.conf"

printf '%s\n' "$GPG_PRIVATE_KEY" | gpg --batch --import
gpgconf --kill gpg-agent >/dev/null 2>&1 || true
KEY_ID="${GPG_KEY_ID:-$(gpg --list-secret-keys --with-colons | awk -F: '/^sec:/ {print $5; exit}')}"
if [[ -z "$KEY_ID" ]]; then
  echo "Could not determine GPG key id" >&2
  exit 1
fi

# Prefer primary fingerprint when GPG_KEY_ID looks like a short id.
FPR="$(gpg --list-secret-keys --with-colons | awk -F: '/^fpr:/ {print $10; exit}')"
SIGN_USER="${GPG_KEY_ID:-$FPR}"
if [[ -z "$SIGN_USER" ]]; then
  SIGN_USER="$KEY_ID"
fi

PASS_FILE="$GNUPGHOME/passphrase"
# Strip CR/LF so GitHub secret paste cannot break pinentry loopback.
printf '%s' "${GPG_PASSPHRASE:-}" | tr -d '\r\n' >"$PASS_FILE"
chmod 600 "$PASS_FILE"
echo "GPG passphrase length for detached sign: $(wc -c <"$PASS_FILE")"

gpg_sign_args=(--batch --yes --detach-sign --armor --local-user "$SIGN_USER")
if [[ -s "$PASS_FILE" ]]; then
  gpg_sign_args+=(--pinentry-mode loopback --passphrase-file "$PASS_FILE")
fi

sign_detached() {
  local file="$1"
  gpg "${gpg_sign_args[@]}" --output "${file}.asc" "$file"
  echo "Signed $file -> ${file}.asc"
}

rpm_embed_sign() {
  local rpm="$1"
  if [[ "${CPN_RPMSIGN:-0}" != "1" ]]; then
    return 0
  fi
  if ! command -v rpmsign >/dev/null 2>&1; then
    if [[ "${CPN_REQUIRE_RPMSIGN:-0}" == "1" ]]; then
      echo "rpmsign required but not installed (install rpm-sign)" >&2
      exit 1
    fi
    echo "rpmsign not available; skipping embedded RPM signature for $rpm"
    return 0
  fi
  # Prefer passphrase-file so special characters never break rpmmacros quoting.
  cat >"$GNUPGHOME/rpmmacros" <<EOF
%_gpg_name $SIGN_USER
%__gpg $(command -v gpg)
%_gpg_path $GNUPGHOME
%__gpg_sign_cmd %{__gpg} gpg --batch --no-verbose --no-armor --pinentry-mode loopback --passphrase-file $PASS_FILE --no-secmem-warning -u "%{_gpg_name}" -sbo %{__signature_filename} --digest-algo sha256 %{__plaintext_filename}
EOF
  HOME="$GNUPGHOME" rpmsign --addsign "$rpm"
  echo "Embedded RPM signature: $rpm"
  if command -v rpm >/dev/null 2>&1; then
    HOME="$GNUPGHOME" rpm --checksig "$rpm" || {
      echo "rpm --checksig failed for $rpm" >&2
      exit 1
    }
  fi
}

for path in "$@"; do
  [[ -f "$path" ]] || { echo "Missing file: $path" >&2; exit 1; }
  case "$path" in
    *.rpm)
      rpm_embed_sign "$path"
      sign_detached "$path"
      ;;
    *)
      sign_detached "$path"
      ;;
  esac
done
