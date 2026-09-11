#!/usr/bin/env bash
# CPN Control Panel Network: upgrade the installed cpn-installer package from GitHub Releases.
# Preferred one-liner (run as root; News Targeted host, then GitHub raw fallback):
#   sh <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh || wget -O - https://cpn.newstargeted.com/upgrade.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh)
# This file is the GitHub-curled bootstrap alias (repo-root / scripts). Prefer /upgrade.sh on cpn.newstargeted.com.
#
# Env: same as scripts/install.sh (CPN_RELEASE_TAG, CPN_STABLE_ONLY, CPN_REQUIRE_GPG, CPN_ALLOW_UNSIGNED, ...)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd || true)"
if [[ -n "${SCRIPT_DIR}" && -f "${SCRIPT_DIR}/upgrade.sh" ]]; then
  # Local clone / packaged tree: reuse upgrade.sh implementation.
  # shellcheck disable=SC1091
  exec bash "${SCRIPT_DIR}/upgrade.sh" "$@"
fi
if [[ -n "${SCRIPT_DIR}" && -f "${SCRIPT_DIR}/scripts/upgrade.sh" ]]; then
  # Repo root preUpgrade.sh: prefer scripts/upgrade.sh.
  # shellcheck disable=SC1091
  exec bash "${SCRIPT_DIR}/scripts/upgrade.sh" "$@"
fi

# When curled as a standalone file (no sibling upgrade.sh), continue with embedded logic below.
CPN_GITHUB_REPO="${CPN_GITHUB_REPO:-Control-Panel-Network/CPN-Control-Panel-Network}"
CPN_REQUIRE_GPG="${CPN_REQUIRE_GPG:-1}"
CPN_ALLOW_UNSIGNED="${CPN_ALLOW_UNSIGNED:-0}"
CPN_EXPECTED_FPR="${CPN_EXPECTED_FPR:-FE70B9718F63B10BB70A6F70BECBB7488AE5C3E5}"
API_BASE="https://api.github.com/repos/${CPN_GITHUB_REPO}"
RAW_KEY_URL="https://raw.githubusercontent.com/${CPN_GITHUB_REPO}/stable/packaging/RPM-GPG-KEY-CPN"

die() { echo "CPN preUpgrade error: $*" >&2; exit 1; }
info() { echo "CPN: $*"; }

require_root() {
  if [[ "$(id -u)" -ne 0 ]]; then
    die "root is required (re-run with sudo or as root)"
  fi
}

require_https_url() {
  local url="$1"
  [[ "$url" == https://* ]] || die "refusing non-HTTPS URL: $url"
}

have_cmd() { command -v "$1" >/dev/null 2>&1; }

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
      -H "User-Agent: CPN-preUpgrade.sh" \
      "$url"
  elif have_cmd wget; then
    wget -qO- --https-only \
      --header="Accept: application/vnd.github+json" \
      --header="User-Agent: CPN-preUpgrade.sh" \
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

require_existing_install() {
  if ! have_cmd cpn-installer && [[ ! -x /usr/bin/cpn-installer ]]; then
    die "cpn-installer is not installed. Use the CPN install one-liner for a new install."
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
      die "EL${EL_MAJOR} has no native CPN release RPM. Upgrade the guest OS or install from a supported release asset."
    fi
    if [[ "$EL_MAJOR" -gt 10 ]]; then
      die "unsupported Enterprise Linux major: ${EL_MAJOR}"
    fi
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
  # Skip published tags that still have no matching package assets (release workflow mid-run).
  json="$(printf '%s' "$body" | FAMILY="$FAMILY" EL_MAJOR="${EL_MAJOR:-}" PKG_ARCH="$PKG_ARCH" DEB_ARCH="$DEB_ARCH" python3 -c '
import json,sys,os,re
items=json.load(sys.stdin)
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

upgrade_package() {
  local artifact="$1"
  case "$FAMILY" in
    dnf)
      if have_cmd dnf; then
        dnf upgrade -y "$artifact" || dnf install -y "$artifact"
      elif have_cmd yum; then
        yum upgrade -y "$artifact" || yum install -y "$artifact"
      else
        die "dnf or yum is required"
      fi
      ;;
    apt)
      if have_cmd apt-get; then
        apt-get install -y "$artifact"
      else
        die "apt-get is required"
      fi
      ;;
    *) die "internal error: unknown family $FAMILY" ;;
  esac
}

print_next_steps() {
  local ran_panel="${1:-0}"
  local panel_rc="${2:-0}"
  cat <<EOF

CPN package upgrade finished.
EOF
  if [[ "$ran_panel" == "1" ]]; then
    if [[ "$panel_rc" -eq 0 ]]; then
      cat <<'EOF'
Panel maintenance completed: ran `cpn-installer --upgrade` (cleanup + service verify).
EOF
    else
      cat <<'EOF'
Panel maintenance was started but exited with an error.
Retry: sudo cpn-installer --upgrade
Optional Docker refresh (CPN-managed only): sudo cpn-installer --upgrade --bypass
EOF
    fi
  else
    cat <<'EOF'
No completed panel install detected on this host (no install-manifest / panel-bootstrap).
Package binary updated only. Open the installer when ready:
  sudo cpn-installer
EOF
  fi
  cat <<'EOF'

Keep backups before upgrading production-like hosts. CPN is still under active development.
EOF
}

panel_install_detected() {
  local data_dir="${CPN_DATA_DIR:-/var/lib/cpn}"
  [[ -f "${data_dir}/install-manifest.json" || -f "${data_dir}/panel-bootstrap.json" ]]
}

run_panel_maintenance() {
  local bin=""
  if have_cmd cpn-installer; then
    bin="$(command -v cpn-installer)"
  elif [[ -x /usr/bin/cpn-installer ]]; then
    bin="/usr/bin/cpn-installer"
  else
    die "cpn-installer missing after package upgrade"
  fi
  local args=(--upgrade)
  if [[ "${CPN_UPGRADE_BYPASS:-0}" == "1" ]] || [[ "${CPN_UPGRADE_BYPASS_FLAG:-0}" == "1" ]]; then
    args+=(--bypass)
    info "passing --bypass (CPN-managed Docker refresh opt-in)"
  fi
  info "running panel maintenance: ${bin} ${args[*]}"
  # Non-interactive: --upgrade does not require a TTY.
  if "$bin" "${args[@]}"; then
    return 0
  fi
  return 1
}

ensure_panel_service_reachable() {
  if ! have_cmd systemctl; then
    return 0
  fi
  local data_dir="${CPN_DATA_DIR:-/var/lib/cpn}"
  local allow=0
  if [[ -f "${data_dir}/allow_remote" ]]; then
    case "$(tr -d '[:space:]' < "${data_dir}/allow_remote" | tr '[:upper:]' '[:lower:]')" in
      1|true|yes|on) allow=1 ;;
    esac
  fi
  if [[ "$allow" -eq 1 ]]; then
    mkdir -p /etc/systemd/system/cpn-installer.service.d
    cat > /etc/systemd/system/cpn-installer.service.d/10-allow-remote.conf <<'DROPIN'
# Generated by CPN upgrade.sh after an allow-remote install.
[Service]
Environment=CPN_ALLOW_REMOTE=1
DROPIN
  fi
  systemctl daemon-reload >/dev/null 2>&1 || true
  systemctl enable cpn-installer.service >/dev/null 2>&1 || true
  systemctl restart cpn-installer.service >/dev/null 2>&1 \
    || systemctl start cpn-installer.service >/dev/null 2>&1 \
    || true
  info "panel service restart attempted (allow_remote=${allow})"
}

main() {
  # Optional: upgrade.sh --bypass  or  CPN_UPGRADE_BYPASS=1
  for arg in "$@"; do
    case "$arg" in
      --bypass) CPN_UPGRADE_BYPASS_FLAG=1; export CPN_UPGRADE_BYPASS_FLAG ;;
      -h|--help)
        cat <<'EOF'
CPN upgrade.sh: upgrade the cpn-installer package, then run panel maintenance when installed.

Usage: upgrade.sh [--bypass]

  --bypass   Pass through to cpn-installer --upgrade --bypass (CPN-managed Docker only)

Env: CPN_RELEASE_TAG, CPN_STABLE_ONLY, CPN_REQUIRE_GPG, CPN_ALLOW_UNSIGNED, CPN_UPGRADE_BYPASS=1
EOF
        exit 0
        ;;
    esac
  done

  require_root
  have_cmd python3 || die "python3 is required"
  require_existing_install
  detect_guest
  info "detected ${DIST_LABEL} (${FAMILY}, arch ${PKG_ARCH}/${DEB_ARCH})"

  local work release_json tag pkg_name pkg_url sums_url asc_url key_url
  work="$(mktemp -d /var/tmp/cpn-upgrade.XXXXXX)"
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
  asc_url="$(asset_url_by_name "$release_json" "SHA256SUMS.asc" 2>/dev/null || true)"

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
  fi

  info "upgrading with $pkg_name"
  upgrade_package "$work/$pkg_name"

  local ran_panel=0 panel_rc=0
  if panel_install_detected; then
    ran_panel=1
    if run_panel_maintenance; then
      panel_rc=0
    else
      panel_rc=1
    fi
    ensure_panel_service_reachable
  else
    info "panel install markers not found; skipped automatic cpn-installer --upgrade"
  fi

  print_next_steps "$ran_panel" "$panel_rc"
  if [[ "$ran_panel" == "1" && "$panel_rc" -ne 0 ]]; then
    exit 1
  fi
}

main "$@"
