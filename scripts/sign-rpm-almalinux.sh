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
printf '%s' "${GPG_PASSPHRASE:-}" >"$work/passphrase"
chmod 600 "$work/passphrase"
printf '%s\n' "${GPG_KEY_ID:-}" >"$work/key_id"

idx=0
for rpm in "$@"; do
  [[ -f "$rpm" ]] || { echo "Missing RPM: $rpm" >&2; exit 1; }
  cp "$rpm" "$work/rpms/$(printf '%03d' "$idx")-$(basename "$rpm")"
  idx=$((idx + 1))
done

"$engine" run --rm \
  -e CPN_REQUIRE_RPMSIGN="${CPN_REQUIRE_RPMSIGN:-1}" \
  -v "$work:/work:rw" \
  -w /work \
  almalinux:9 \
  bash -lc '
    set -euo pipefail
    dnf -y install --setopt=install_weak_deps=False gnupg2 rpm-sign rpm-build >/dev/null
    export GNUPGHOME=/work/gnupg
    mkdir -p "$GNUPGHOME"
    chmod 700 "$GNUPGHOME"
    gpg --batch --import /work/private.asc
    KEY_ID="$(tr -d "\r\n" </work/key_id)"
    if [[ -z "$KEY_ID" ]]; then
      KEY_ID="$(gpg --list-secret-keys --with-colons | awk -F: "/^fpr:/ {print \$10; exit}")"
    fi
    PASS="$(cat /work/passphrase)"
    cat >"$HOME/.rpmmacros" <<EOF
%_gpg_name $KEY_ID
%__gpg /usr/bin/gpg
%_gpg_path $GNUPGHOME
%__gpg_sign_cmd %{__gpg} gpg --batch --no-verbose --no-armor --pinentry-mode loopback --passphrase "$PASS" --no-secmem-warning -u "%{_gpg_name}" -sbo %{__signature_filename} --digest-algo sha256 %{__plaintext_filename}
EOF
    shopt -s nullglob
    for rpm in /work/rpms/*.rpm; do
      rpmsign --addsign "$rpm"
      rpm --checksig "$rpm"
      echo "rpmsign OK: $(basename "$rpm")"
    done
  '

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
