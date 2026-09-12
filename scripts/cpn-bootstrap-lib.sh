#!/usr/bin/env bash
# Shared helpers for CPN install.sh / upgrade.sh / preUpgrade.sh.
# Sourced by those scripts (local file, cpn.newstargeted.com, or GitHub raw).
# shellcheck shell=bash

cpn_bootstrap_die() { echo "CPN bootstrap error: $*" >&2; exit 1; }
cpn_bootstrap_info() { echo "CPN: $*"; }

# Usage: cpn_parse_bootstrap_args install|upgrade -- "$@"
cpn_parse_bootstrap_args() {
  local mode="$1"
  shift
  [[ "${1:-}" == "--" ]] && shift
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -b|--branch|--ref)
        [[ $# -ge 2 ]] || cpn_bootstrap_die "$1 requires a git ref (branch or tag name)"
        CPN_REF_ARG="$2"
        export CPN_REF_ARG
        shift 2
        ;;
      --bypass)
        if [[ "$mode" != "upgrade" ]]; then
          cpn_bootstrap_die "--bypass is only valid for upgrade.sh"
        fi
        CPN_UPGRADE_BYPASS_FLAG=1
        export CPN_UPGRADE_BYPASS_FLAG
        shift
        ;;
      -h|--help)
        if [[ "$mode" == "upgrade" ]]; then
          cat <<EOF
CPN upgrade.sh: upgrade cpn-installer from GitHub Releases, then auto-run panel maintenance when installed.

Usage: upgrade.sh [-b REF] [--bypass]

  -b, --branch, --ref REF   Pin packages to a GitHub Release matching REF (tries REF and vREF)
  --bypass                  Pass --bypass to cpn-installer --upgrade (CPN-managed Docker only)

Env: CPN_RELEASE_TAG, CPN_BRANCH, CPN_STABLE_ONLY, CPN_REQUIRE_GPG, CPN_ALLOW_UNSIGNED, CPN_UPGRADE_BYPASS=1
     CPN_GITHUB_TOKEN / GITHUB_TOKEN or /var/lib/cpn/secrets/github-token (API auth; never commit)

Raw script URLs (github.com/.../<ref>/upgrade.sh is not a raw file URL):
  https://raw.githubusercontent.com/${CPN_GITHUB_REPO:-Control-Panel-Network/CPN-Control-Panel-Network}/stable/scripts/upgrade.sh
  https://raw.githubusercontent.com/${CPN_GITHUB_REPO:-Control-Panel-Network/CPN-Control-Panel-Network}/<ref>/scripts/upgrade.sh

Examples:
  bash <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh) -b 1.0.0-dev
  bash <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh) --bypass
EOF
        else
          cat <<EOF
CPN install.sh: install cpn-installer from GitHub Releases.

Usage: install.sh [-b REF | --branch REF | --ref REF]

  -b, --branch, --ref REF   Pin packages to a GitHub Release matching REF (tries REF and vREF)

Env: CPN_RELEASE_TAG, CPN_BRANCH, CPN_STABLE_ONLY, CPN_REQUIRE_GPG, CPN_ALLOW_UNSIGNED
     CPN_GITHUB_TOKEN / GITHUB_TOKEN or /var/lib/cpn/secrets/github-token (API auth; never commit)

Raw script URLs (github.com/.../<ref>/install.sh is not a raw file URL):
  https://raw.githubusercontent.com/${CPN_GITHUB_REPO:-Control-Panel-Network/CPN-Control-Panel-Network}/stable/scripts/install.sh
  https://raw.githubusercontent.com/${CPN_GITHUB_REPO:-Control-Panel-Network/CPN-Control-Panel-Network}/<ref>/scripts/install.sh

Example:
  bash <(curl -fsSL https://cpn.newstargeted.com/install.sh) -b 1.0.0-dev
EOF
        fi
        exit 0
        ;;
      *)
        cpn_bootstrap_die "unknown argument: $1 (try --help)"
        ;;
    esac
  done
}

# Resolve -b / CPN_BRANCH into CPN_RELEASE_TAG when unset.
# Requires: download_text, API_BASE, die, info (from caller script).
cpn_apply_git_ref_pin() {
  local ref="${CPN_REF_ARG:-${CPN_BRANCH:-}}"
  [[ -n "$ref" ]] || return 0
  export CPN_BRANCH="$ref"
  info "git ref pin: $ref"
  if [[ -n "${CPN_RELEASE_TAG:-}" ]]; then
    info "CPN_RELEASE_TAG=${CPN_RELEASE_TAG} (explicit; not overridden by -b)"
    return 0
  fi
  local candidate body
  local -a candidates=("$ref")
  if [[ "$ref" == v* ]]; then
    candidates+=("${ref#v}")
  else
    candidates+=("v${ref}")
  fi
  for candidate in "${candidates[@]}"; do
    if body="$(download_text "${API_BASE}/releases/tags/${candidate}" 2>/dev/null)" \
      && [[ -n "$body" ]]; then
      CPN_RELEASE_TAG="$candidate"
      export CPN_RELEASE_TAG
      info "matched GitHub Release tag ${CPN_RELEASE_TAG} for ref ${ref}"
      return 0
    fi
  done
  die "No GitHub Release matches ref '${ref}' (tried: ${candidates[*]}). -b/--ref pins package install to a published Release tag. To load bootstrap scripts from a git ref without a Release, curl https://raw.githubusercontent.com/${CPN_GITHUB_REPO}/<ref>/scripts/install.sh or upgrade.sh (github.com/.../<ref>/install.sh is not a raw file)."
}

# Replace leftover retired 1.0.0/1.0.1 package identity with tip 0.2.x artifact.
# Returns 0 if retag applied, 1 if not a retag case (caller continues normal install).
cpn_try_retag_package() {
  local artifact="$1"
  local family="$2"
  local installed_ver=""
  case "$family" in
    dnf)
      command -v rpm >/dev/null 2>&1 || return 1
      installed_ver="$(rpm -q --qf '%{VERSION}' cpn-installer 2>/dev/null || true)"
      [[ "$installed_ver" == "1.0.0" || "$installed_ver" == "1.0.1" ]] || return 1
      info "retag migration: replacing retired CPN ${installed_ver} with current 0.2.x (rpm --oldpackage)"
      if rpm -Uvh --oldpackage "$artifact"; then
        return 0
      fi
      rpm -e --nodeps cpn-installer >/dev/null 2>&1 || true
      rpm -Uvh "$artifact" && return 0
      die "could not replace retired CPN ${installed_ver} with $(basename "$artifact")"
      ;;
    apt)
      command -v dpkg-query >/dev/null 2>&1 || return 1
      installed_ver="$(dpkg-query -W -f='${Version}' cpn-installer 2>/dev/null || true)"
      installed_ver="${installed_ver%%-*}"
      installed_ver="${installed_ver%%~*}"
      [[ "$installed_ver" == "1.0.0" || "$installed_ver" == "1.0.1" ]] || return 1
      info "retag migration: replacing retired CPN ${installed_ver} with current 0.2.x (apt allow-downgrades)"
      command -v apt-get >/dev/null 2>&1 || die "apt-get is required"
      apt-get update -y >/dev/null || true
      apt-get install -y --allow-downgrades "$artifact" && return 0
      die "could not replace retired CPN ${installed_ver} with $(basename "$artifact")"
      ;;
    *) return 1 ;;
  esac
}
