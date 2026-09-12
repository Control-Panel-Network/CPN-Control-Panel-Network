#!/usr/bin/env bash
# CPN Control Panel Network: install the latest matching release package.
# Official one-liner (asks for sudo automatically; News Targeted host, then GitHub raw fallback):
#   bash <(curl -fsSL https://cpn.newstargeted.com/install.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh || wget -O - https://cpn.newstargeted.com/install.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh)
#
# Env:
#   CPN_RELEASE_TAG          pin a Release tag (example: v0.2.6-alpha.22); default: newest non-draft release
#   CPN_BRANCH / CPN_REF     pin a git branch or tag name; resolves to a matching Release tag for packages
#   CPN_STABLE_ONLY          1 to skip GitHub prereleases (future stable line); default includes alphas
#   CPN_INCLUDE_PRERELEASE   legacy alias: 0 with CPN_STABLE_ONLY unset still includes prereleases
#   CPN_GITHUB_REPO          owner/name (default: Control-Panel-Network/CPN-Control-Panel-Network)
#   CPN_REQUIRE_GPG          1 (default) require SHA256SUMS.asc + matching fingerprint
#   CPN_ALLOW_UNSIGNED       1 allow missing GPG assets (lab only; not for production)
# CLI: -b|--branch|--ref REF  same as CPN_BRANCH (example: bash <(curl -fsSL .../install.sh) -b 1.0.0-dev)
set -euo pipefail

CPN_GITHUB_REPO="${CPN_GITHUB_REPO:-Control-Panel-Network/CPN-Control-Panel-Network}"
CPN_REQUIRE_GPG="${CPN_REQUIRE_GPG:-1}"
CPN_ALLOW_UNSIGNED="${CPN_ALLOW_UNSIGNED:-0}"
CPN_STABLE_ONLY="${CPN_STABLE_ONLY:-0}"
CPN_EXPECTED_FPR="${CPN_EXPECTED_FPR:-FE70B9718F63B10BB70A6F70BECBB7488AE5C3E5}"
CPN_REF_ARG="${CPN_REF_ARG:-}"
CPN_BOOTSTRAP_LIB_REF="${CPN_BOOTSTRAP_LIB_REF:-stable}"
API_BASE="https://api.github.com/repos/${CPN_GITHUB_REPO}"
RAW_KEY_URL="https://raw.githubusercontent.com/${CPN_GITHUB_REPO}/stable/packaging/RPM-GPG-KEY-CPN"

die() { echo "CPN install error: $*" >&2; exit 1; }
info() { echo "CPN: $*"; }

require_root() {
  [[ "$(id -u)" -eq 0 ]] || die "automatic sudo elevation failed"
}

require_https_url() {
  local url="$1"
  [[ "$url" == https://* ]] || die "refusing non-HTTPS URL: $url"
}

have_cmd() { command -v "$1" >/dev/null 2>&1; }

elevate_if_needed() {
  [[ "$(id -u)" -eq 0 ]] && return 0
  have_cmd sudo || die "root access is required and sudo is not installed"
  [[ "${CPN_SELF_ELEVATED:-0}" != "1" ]] || die "sudo did not grant root access"

  local source_path elevated_script exit_code
  source_path="${BASH_SOURCE[0]:-}"
  [[ -n "$source_path" && -r "$source_path" ]] \
    || die "could not preserve the downloaded installer for sudo"
  elevated_script="$(mktemp "${TMPDIR:-/tmp}/cpn-install-elevated.XXXXXX")"
  chmod 700 "$elevated_script"
  cp -- "$source_path" "$elevated_script" \
    || die "could not copy the installer before sudo elevation"
  info "administrator privileges are required; requesting them with sudo"
  set +e
  sudo env \
    CPN_SELF_ELEVATED=1 \
    CPN_RELEASE_TAG="${CPN_RELEASE_TAG:-}" \
    CPN_BRANCH="${CPN_BRANCH:-}" \
    CPN_REF="${CPN_REF:-}" \
    CPN_STABLE_ONLY="$CPN_STABLE_ONLY" \
    CPN_INCLUDE_PRERELEASE="${CPN_INCLUDE_PRERELEASE:-}" \
    CPN_GITHUB_REPO="$CPN_GITHUB_REPO" \
    CPN_REQUIRE_GPG="$CPN_REQUIRE_GPG" \
    CPN_ALLOW_UNSIGNED="$CPN_ALLOW_UNSIGNED" \
    CPN_EXPECTED_FPR="$CPN_EXPECTED_FPR" \
    bash "$elevated_script" "$@"
  exit_code=$?
  set -e
  rm -f -- "$elevated_script"
  exit "$exit_code"
}

# Peek -b so shared lib can load from the same git ref.
cpn_peek_ref_arg() {
  local -a args=("$@")
  local i
  for ((i = 0; i < ${#args[@]}; i++)); do
    case "${args[$i]}" in
      -b|--branch|--ref)
        if ((i + 1 < ${#args[@]})); then
          CPN_BOOTSTRAP_LIB_REF="${args[$((i + 1))]}"
          CPN_REF_ARG="${args[$((i + 1))]}"
          export CPN_BOOTSTRAP_LIB_REF CPN_REF_ARG
        fi
        ;;
    esac
  done
}

cpn_source_bootstrap_lib() {
  local ref="${CPN_BOOTSTRAP_LIB_REF:-stable}"
  local here candidate url
  here="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd || true)"
  for candidate in \
    "${here}/cpn-bootstrap-lib.sh" \
    "./scripts/cpn-bootstrap-lib.sh" \
    "./cpn-bootstrap-lib.sh"; do
    if [[ -n "$candidate" && -f "$candidate" ]]; then
      # shellcheck disable=SC1090
      . "$candidate"
      return 0
    fi
  done
  for url in \
    "https://cpn.newstargeted.com/cpn-bootstrap-lib.sh" \
    "https://raw.githubusercontent.com/${CPN_GITHUB_REPO}/${ref}/scripts/cpn-bootstrap-lib.sh" \
    "https://raw.githubusercontent.com/${CPN_GITHUB_REPO}/stable/scripts/cpn-bootstrap-lib.sh"; do
    if have_cmd curl && . <(curl -fsSL --proto '=https' --tlsv1.2 --max-time 30 "$url"); then
      return 0
    fi
  done
  die "could not load cpn-bootstrap-lib.sh (host or GitHub raw)"
}

download() {
  local url="$1" dest="$2"
  require_https_url "$url"
  if have_cmd curl; then
    curl -fsSL --proto '=https' --tlsv1.2 --max-time 120 -o "$dest" "$url" \
      || die "download failed: $url"
  elif have_cmd wget; then
    wget -qO "$dest" --https-only --secure-protocol=TLSv1_2 "$url" \
      || die "download failed: $url"
  else
    die "curl or wget is required"
  fi
}

download_text() {
  local url="$1"
  require_https_url "$url"
  if have_cmd curl; then
    curl -fsSL --proto '=https' --tlsv1.2 --max-time 60 \
      -H "Accept: application/vnd.github+json" \
      -H "User-Agent: CPN-install.sh" \
      "$url"
  elif have_cmd wget; then
    wget -qO- --https-only \
      --header="Accept: application/vnd.github+json" \
      --header="User-Agent: CPN-install.sh" \
      "$url"
  else
    die "curl or wget is required"
  fi
}

sha256_file() {
  local path="$1"
  if have_cmd sha256sum; then
    sha256sum "$path" | awk '{print $1}'
  elif have_cmd shasum; then
    shasum -a 256 "$path" | awk '{print $1}'
  else
    die "sha256sum or shasum is required"
  fi
}

detect_guest() {
  [[ -r /etc/os-release ]] || die "missing /etc/os-release; unsupported OS"
  # shellcheck disable=SC1091
  . /etc/os-release
  local id="${ID:-}" id_like="${ID_LIKE:-}" version_id="${VERSION_ID:-}"
  local major="${version_id%%.*}"
  local arch
  arch="$(uname -m)"

  FAMILY=""
  EL_MAJOR=""
  PKG_ARCH=""
  DEB_ARCH=""
  DIST_LABEL=""

  case "$arch" in
    x86_64|amd64) PKG_ARCH="x86_64"; DEB_ARCH="amd64" ;;
    aarch64|arm64) PKG_ARCH="aarch64"; DEB_ARCH="arm64" ;;
    *) die "unsupported architecture: $arch" ;;
  esac

  case "$id" in
    almalinux|rocky|rhel|centos|cloudlinux|ol|eurolinux|scientific)
      FAMILY="dnf"
      EL_MAJOR="$major"
      DIST_LABEL="${id} ${version_id}"
      ;;
    ubuntu|debian)
      FAMILY="apt"
      DIST_LABEL="${id} ${version_id}"
      ;;
    *)
      case " $id_like " in
        *" rhel "*|*" centos "*|*" fedora "*)
          FAMILY="dnf"
          EL_MAJOR="$major"
          DIST_LABEL="${id} ${version_id}"
          ;;
        *" debian "*)
          FAMILY="apt"
          DIST_LABEL="${id} ${version_id}"
          ;;
        *)
          die "unknown or unsupported OS (ID=${id:-?}). See docs/SUPPORT.md"
          ;;
      esac
      ;;
  esac

  if [[ "$FAMILY" == "dnf" ]]; then
    [[ "$EL_MAJOR" =~ ^[0-9]+$ ]] || die "could not parse Enterprise Linux major from VERSION_ID=${version_id}"
    if [[ "$EL_MAJOR" -lt 9 ]]; then
      die "EL${EL_MAJOR} has no native CPN release RPM (OpenSSL / WebAuthn constraint). Use AlmaLinux/Rocky/RHEL 9 or 10, or Ubuntu/Debian."
    fi
    if [[ "$EL_MAJOR" -gt 10 ]]; then
      die "unsupported Enterprise Linux major: ${EL_MAJOR}"
    fi
  fi

  if [[ "$id" == "ubuntu" && "$major" -lt 22 ]]; then
    die "Ubuntu ${version_id} is refused for new CPN installs (use 22.04 or 24.04)"
  fi
  if [[ "$id" == "debian" && "$major" -lt 12 ]]; then
    die "Debian ${version_id} is refused for new CPN installs (use 12 or newer)"
  fi
}

pick_release_json() {
  local json body
  if [[ -n "${CPN_RELEASE_TAG:-}" ]]; then
    body="$(download_text "${API_BASE}/releases/tags/${CPN_RELEASE_TAG}")" \
      || die "release tag not found: ${CPN_RELEASE_TAG}"
    printf '%s' "$body"
    return
  fi
  body="$(download_text "${API_BASE}/releases?per_page=30")" \
    || die "could not list GitHub Releases"
  if have_cmd python3; then
    # Skip published tags that still have no matching package assets (release workflow mid-run).
    json="$(printf '%s' "$body" | FAMILY="$FAMILY" EL_MAJOR="${EL_MAJOR:-}" PKG_ARCH="$PKG_ARCH" DEB_ARCH="$DEB_ARCH" python3 -c '
import json,sys,os,re
items=json.load(sys.stdin)
# Alpha-only period: include prereleases by default. Set CPN_STABLE_ONLY=1 to skip them.
stable_only=os.environ.get("CPN_STABLE_ONLY","0").strip()=="1"
include_pre_legacy=os.environ.get("CPN_INCLUDE_PRERELEASE","1").strip()
include_pre = (not stable_only) and include_pre_legacy != "0"
family=os.environ.get("FAMILY","").strip()
el_major=os.environ.get("EL_MAJOR","").strip()
pkg_arch=os.environ.get("PKG_ARCH","").strip()
deb_arch=os.environ.get("DEB_ARCH","").strip()

def has_package(rel):
    names=[a.get("name","") for a in (rel.get("assets") or [])]
    if not names:
        return False
    if "SHA256SUMS" not in names:
        return False
    if family=="dnf":
        pat=re.compile(r"^cpn-installer-.*\.el%s\.%s\.rpm$" % (re.escape(el_major), re.escape(pkg_arch)))
        return any(pat.match(n) for n in names)
    suffix="_%s.deb" % deb_arch
    return any(n.startswith("cpn-installer_") and n.endswith(suffix) for n in names)

for item in items:
    if item.get("draft"):
        continue
    if item.get("prerelease") and not include_pre:
        continue
    if not has_package(item):
        continue
    print(json.dumps(item))
    break
else:
    sys.exit(2)
')" || die "no matching GitHub release with packages for this OS (set CPN_RELEASE_TAG, or wait for the Release workflow to finish uploading assets)"
    printf '%s' "$json"
    return
  fi
  die "python3 is required to select the latest GitHub release (or set CPN_RELEASE_TAG)"
}

asset_url_by_name() {
  local release_json="$1" name="$2"
  printf '%s' "$release_json" | python3 -c '
import json,sys
wanted=sys.argv[1]
rel=json.load(sys.stdin)
for asset in rel.get("assets") or []:
    if asset.get("name")==wanted:
        url=asset.get("browser_download_url") or ""
        if not url.startswith("https://"):
            raise SystemExit(2)
        print(url)
        raise SystemExit(0)
raise SystemExit(1)
' "$name"
}

select_package_name() {
  local release_json="$1"
  printf '%s' "$release_json" | python3 -c '
import json,sys,re
family, el_major, pkg_arch, deb_arch = sys.argv[1:5]
rel=json.load(sys.stdin)
names=[a.get("name","") for a in (rel.get("assets") or [])]
if family=="dnf":
    pat=re.compile(r"^cpn-installer-.*\.el%s\.%s\.rpm$" % (re.escape(el_major), re.escape(pkg_arch)))
    for name in names:
        if pat.match(name):
            print(name); raise SystemExit(0)
    raise SystemExit("no matching el%s %s RPM in this release" % (el_major, pkg_arch))
else:
    suffix="_%s.deb" % deb_arch
    for name in names:
        if name.startswith("cpn-installer_") and name.endswith(suffix):
            print(name); raise SystemExit(0)
    raise SystemExit("no matching %s .deb in this release" % deb_arch)
' "$FAMILY" "${EL_MAJOR:-}" "$PKG_ARCH" "$DEB_ARCH"
}

verify_checksum() {
  local artifact="$1" sums="$2"
  local base expected actual
  base="$(basename "$artifact")"
  expected="$(awk -v f="$base" '$2 == f { print $1; exit }' "$sums")"
  [[ -n "$expected" ]] || die "SHA256SUMS has no entry for $base"
  actual="$(sha256_file "$artifact")"
  [[ "$actual" == "$expected" ]] || die "SHA-256 mismatch for $base (expected $expected, got $actual)"
  info "SHA-256 OK: $base"
}

verify_gpg_sums() {
  local sums="$1" asc="$2" keyfile="$3"
  if [[ ! -f "$asc" ]]; then
    if [[ "$CPN_ALLOW_UNSIGNED" == "1" ]]; then
      info "warning: SHA256SUMS.asc missing; continuing because CPN_ALLOW_UNSIGNED=1"
      return
    fi
    die "missing SHA256SUMS.asc (set CPN_ALLOW_UNSIGNED=1 only for local lab builds)"
  fi
  if [[ "$CPN_REQUIRE_GPG" != "1" && "$CPN_ALLOW_UNSIGNED" == "1" ]]; then
    info "skipping GPG (CPN_REQUIRE_GPG=0)"
    return
  fi
  have_cmd gpg || die "gpg is required to verify SHA256SUMS.asc"
  local gnupghome fpr
  gnupghome="$(mktemp -d)"
  chmod 700 "$gnupghome"
  export GNUPGHOME="$gnupghome"
  # shellcheck disable=SC2064
  trap "rm -rf '$gnupghome'" RETURN
  gpg --batch --import "$keyfile" >/dev/null 2>&1 || die "could not import RPM-GPG-KEY-CPN"
  fpr="$(gpg --batch --with-colons --fingerprint | awk -F: '/^fpr:/ { print $10; exit }')"
  fpr="$(printf '%s' "$fpr" | tr -d ' ')"
  [[ "$fpr" == "$CPN_EXPECTED_FPR" ]] || die "GPG fingerprint mismatch (got ${fpr:-empty}, expected $CPN_EXPECTED_FPR)"
  gpg --batch --verify "$asc" "$sums" >/dev/null 2>&1 || die "GPG verification failed for SHA256SUMS.asc"
  info "GPG OK: SHA256SUMS.asc (fingerprint $CPN_EXPECTED_FPR)"
}

install_package() {
  local artifact="$1"
  if cpn_try_retag_package "$artifact" "$FAMILY"; then
    return 0
  fi
  case "$FAMILY" in
    dnf)
      if have_cmd dnf; then
        dnf install -y "$artifact"
      elif have_cmd yum; then
        yum install -y "$artifact"
      else
        die "dnf or yum is required"
      fi
      ;;
    apt)
      if have_cmd apt-get; then
        apt-get update -y >/dev/null || true
        apt-get install -y "$artifact"
      else
        die "apt-get is required"
      fi
      ;;
    *) die "internal error: unknown family $FAMILY" ;;
  esac
}

print_next_steps() {
  cat <<'EOF'

CPN package install finished.

Start the installer (language is detected automatically):
  sudo cpn-installer
  # or explicitly:
  sudo cpn-installer --cli    # SSH/CLI questions in this terminal
  sudo cpn-installer --web    # Web UI (browser)

Default web listen address: 127.0.0.1:2087
Remote access for the web UI (SSH tunnel recommended):
  ssh -L 2087:127.0.0.1:2087 root@your-server

CPN is under active development. Prefer a test VPS/VM and keep backups.
EOF
}

main() {
  elevate_if_needed "$@"
  cpn_peek_ref_arg "$@"
  cpn_source_bootstrap_lib
  cpn_parse_bootstrap_args install -- "$@"
  require_root
  have_cmd python3 || die "python3 is required"
  detect_guest
  info "detected ${DIST_LABEL} (${FAMILY}, arch ${PKG_ARCH}/${DEB_ARCH})"
  cpn_apply_git_ref_pin

  local work release_json tag pkg_name pkg_url sums_url asc_url key_url
  work="$(mktemp -d /var/tmp/cpn-install.XXXXXX)"
  chmod 700 "$work"
  # shellcheck disable=SC2064
  trap "rm -rf '$work'" EXIT

  release_json="$(pick_release_json)"
  tag="$(printf '%s' "$release_json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("tag_name",""))')"
  [[ -n "$tag" ]] || die "release JSON missing tag_name"
  info "using release $tag"

  pkg_name="$(select_package_name "$release_json")" \
    || die "no compatible package for this OS in release $tag"
  pkg_url="$(asset_url_by_name "$release_json" "$pkg_name")" \
    || die "missing download URL for $pkg_name"
  sums_url="$(asset_url_by_name "$release_json" "SHA256SUMS")" \
    || die "release $tag is missing SHA256SUMS"
  asc_url="$(asset_url_by_name "$release_json" "SHA256SUMS.asc" || true)"

  download "$pkg_url" "$work/$pkg_name"
  download "$sums_url" "$work/SHA256SUMS"
  if [[ -n "${asc_url:-}" ]]; then
    download "$asc_url" "$work/SHA256SUMS.asc"
  fi
  key_url="$(asset_url_by_name "$release_json" "RPM-GPG-KEY-CPN" 2>/dev/null || true)"
  if [[ -n "${key_url:-}" ]]; then
    download "$key_url" "$work/RPM-GPG-KEY-CPN"
  else
    download "$RAW_KEY_URL" "$work/RPM-GPG-KEY-CPN"
  fi

  verify_checksum "$work/$pkg_name" "$work/SHA256SUMS"
  verify_gpg_sums "$work/SHA256SUMS" "$work/SHA256SUMS.asc" "$work/RPM-GPG-KEY-CPN"

  if [[ "$FAMILY" == "dnf" ]] && have_cmd rpm; then
    rpm --import "$work/RPM-GPG-KEY-CPN" >/dev/null 2>&1 || true
    if have_cmd rpmkeys; then
      rpmkeys --import "$work/RPM-GPG-KEY-CPN" >/dev/null 2>&1 || true
    fi
    if ! rpm --checksig "$work/$pkg_name" >/dev/null 2>&1; then
      info "warning: rpm --checksig did not fully validate $pkg_name; SHA256SUMS + GPG still applied"
    else
      info "RPM signature check OK"
    fi
  fi

  info "installing $pkg_name"
  install_package "$work/$pkg_name"
  print_next_steps
}

main "$@"
