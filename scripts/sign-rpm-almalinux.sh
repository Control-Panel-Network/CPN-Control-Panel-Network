#!/usr/bin/env bash
# Embed RPM package signatures using AlmaLinux 9 (issue #16).
# Expects GPG_PRIVATE_KEY / GPG_PASSPHRASE / GPG_KEY_ID in the environment,
# and one or more .rpm paths as arguments (typically under dist/).
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <package.rpm> [more.rpm...]" >&2
  exit 2
fi

if [[ -z "${GPG_PRIVATE_KEY:-}" ]]; then
  if [[ "${CPN_REQUIRE_GPG:-0}" == "1" || "${CPN_REQUIRE_RPMSIGN:-0}" == "1" ]]; then
    echo "GPG_PRIVATE_KEY required for rpmsign" >&2
    exit 1
  fi
  echo "GPG_PRIVATE_KEY not set; skipping embedded RPM signatures."
  exit 0
fi

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
engine=""
if command -v docker >/dev/null 2>&1; then
  engine=docker
elif command -v podman >/dev/null 2>&1; then
  engine=podman
else
  echo "docker or podman required for AlmaLinux rpmsign" >&2
  exit 1
fi

work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT
mkdir -p "$work/rpms"
chmod 700 "$work"

printf '%s\n' "$GPG_PRIVATE_KEY" >"$work/private.asc"
chmod 600 "$work/private.asc"
# Exact passphrase bytes only (no trailing newline); strip CR from GitHub secret paste.
printf '%s' "${GPG_PASSPHRASE:-}" | tr -d '\r\n' >"$work/passphrase"
chmod 600 "$work/passphrase"
printf '%s\n' "${GPG_KEY_ID:-}" | tr -d '\r' >"$work/key_id"
echo "GPG passphrase length for rpmsign: $(wc -c <"$work/passphrase")"

idx=0
for rpm in "$@"; do
  [[ -f "$rpm" ]] || { echo "Missing RPM: $rpm" >&2; exit 1; }
  cp "$rpm" "$work/rpms/$(printf '%03d' "$idx")-$(basename "$rpm")"
  idx=$((idx + 1))
done

# Heredoc file avoids nested single-quote breakage inside bash -lc '...'.
cat >"$work/sign-inside.sh" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
dnf -y install --setopt=install_weak_deps=False gnupg2 rpm-sign rpm-build >/dev/null
# Do not place root-owned GnuPG state in the bind mount: the GitHub runner
# must be able to remove the workspace after the container exits.
export GNUPGHOME=/tmp/cpn-gnupg
mkdir -p "$GNUPGHOME"
chmod 700 "$GNUPGHOME"
# Required for non-interactive CI: without this, loopback passphrase looks like "Bad passphrase".
printf '%s\n' 'allow-loopback-pinentry' >"$GNUPGHOME/gpg-agent.conf"
printf '%s\n' 'pinentry-mode loopback' >"$GNUPGHOME/gpg.conf"
gpg --batch --import /work/private.asc
gpgconf --kill gpg-agent >/dev/null 2>&1 || true
KEY_ID="$(tr -d '\r\n' </work/key_id)"
if [[ -z "$KEY_ID" ]]; then
  KEY_ID="$(gpg --list-secret-keys --with-colons | awk -F: '/^fpr:/ {print $10; exit}')"
fi
echo preflight > /work/preflight.txt
if ! gpg --batch --yes --pinentry-mode loopback --passphrase-file /work/passphrase \
  --local-user "$KEY_ID" --detach-sign --armor --output /work/preflight.txt.asc /work/preflight.txt; then
  echo "GPG passphrase unlock failed inside AlmaLinux container (check GPG_PASSPHRASE secret)." >&2
  exit 1
fi
echo "GPG preflight unlock OK"
# The RPM database is separate from GNUPGHOME. Import the exact signing public
# key before verification so `rpm --checksig` validates the signature instead
# of failing with an untrusted-key status on a fresh AlmaLinux container.
gpg --batch --export --armor "$KEY_ID" >/tmp/cpn-rpm-signing.pub
rpm --import /tmp/cpn-rpm-signing.pub
cat >"$HOME/.rpmmacros" <<EOF
%_gpg_name $KEY_ID
%__gpg /usr/bin/gpg
%_gpg_path $GNUPGHOME
%__gpg_sign_cmd %{__gpg} gpg --batch --no-verbose --no-armor --pinentry-mode loopback --passphrase-file /work/passphrase --no-secmem-warning -u "%{_gpg_name}" -sbo %{__signature_filename} --digest-algo sha256 %{__plaintext_filename}
EOF
shopt -s nullglob
for rpm in /work/rpms/*.rpm; do
  rpmsign --addsign "$rpm"
  rpm --checksig "$rpm"
  echo "rpmsign OK: $(basename "$rpm")"
done
EOS
chmod 700 "$work/sign-inside.sh"

"$engine" run --rm \
  -e CPN_REQUIRE_RPMSIGN="${CPN_REQUIRE_RPMSIGN:-1}" \
  -v "$work:/work:rw" \
  -w /work \
  almalinux:9 \
  bash /work/sign-inside.sh

# Copy signed RPMs back over the originals (match by basename suffix after NNN-).
for signed in "$work"/rpms/*.rpm; do
  base="$(basename "$signed")"
  # strip NNN- prefix
  orig_name="${base#*-}"
  dest=""
  for candidate in "$@"; do
    if [[ "$(basename "$candidate")" == "$orig_name" ]]; then
      dest="$candidate"
      break
    fi
  done
  if [[ -z "$dest" ]]; then
    echo "Could not map signed RPM $base back to an input path" >&2
    exit 1
  fi
  cp "$signed" "$dest"
  echo "Updated signed RPM: $dest"
done

# Detached .asc via host sign-release (no CPN_RPMSIGN; embedding already done).
export CPN_RPMSIGN=0
chmod +x "$project_dir/scripts/sign-release.sh"
"$project_dir/scripts/sign-release.sh" "$@"
