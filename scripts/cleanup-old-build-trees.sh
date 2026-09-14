#!/usr/bin/env bash
# Remove abandoned lab/agent cargo worktrees named cpn-build-* and stale
# /tmp|/var/tmp cpn-* extract dirs so disk does not fill from parallel deploys.
#
# Safe by design:
#   - Never deletes the main clone (CPN-Control-Panel-Network)
#   - Never deletes /var/lib/cpn, /etc/cpn, mail, databases, or site homes
#   - Never deletes the current keep dir (CPN_KEEP_BUILD, --keep, or PWD tree)
#   - Never deletes a tree with a live process cwd under it (cargo/npm/etc.)
#
# Usage (agents and humans):
#   ./scripts/cleanup-old-build-trees.sh
#   CPN_KEEP_BUILD=/home/cpn/cpn-build-my-feature ./scripts/cleanup-old-build-trees.sh
#   ./scripts/cleanup-old-build-trees.sh --dry-run
#   ./scripts/cleanup-old-build-trees.sh --keep-count 1
#
# Env:
#   CPN_BUILD_PARENT     Parent dir that holds cpn-build-* (default: /home/cpn or $HOME)
#   CPN_KEEP_BUILD       Absolute path of the active build tree to retain
#   CPN_KEEP_BUILD_COUNT Max newest *idle* trees to keep (beyond keep/in-use; default: 1).
#                        Build hooks pass 0 so only the current tree remains.
#   CPN_TMP_TTL_HOURS    Age before /tmp|/var/tmp cpn-* dirs may be removed (default: 6)
#   CPN_CLEANUP_DRY_RUN  Set to 1 for dry-run
set -euo pipefail

dry_run="${CPN_CLEANUP_DRY_RUN:-0}"
keep_count="${CPN_KEEP_BUILD_COUNT:-1}"
tmp_ttl_hours="${CPN_TMP_TTL_HOURS:-6}"
keep_build="${CPN_KEEP_BUILD:-}"
build_parent="${CPN_BUILD_PARENT:-}"

usage() {
  sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      dry_run=1
      shift
      ;;
    --keep)
      keep_build="${2:-}"
      shift 2
      ;;
    --keep-count)
      keep_count="${2:-1}"
      shift 2
      ;;
    --parent)
      build_parent="${2:-}"
      shift 2
      ;;
    --tmp-ttl-hours)
      tmp_ttl_hours="${2:-6}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if ! [[ "$keep_count" =~ ^[0-9]+$ ]]; then
  echo "CPN_KEEP_BUILD_COUNT / --keep-count must be a non-negative integer." >&2
  exit 2
fi
if ! [[ "$tmp_ttl_hours" =~ ^[0-9]+$ ]]; then
  echo "CPN_TMP_TTL_HOURS / --tmp-ttl-hours must be a non-negative integer." >&2
  exit 2
fi

log() {
  printf '%s\n' "$*"
}

resolve_path() {
  local target="$1"
  if command -v realpath >/dev/null 2>&1; then
    realpath -m "$target" 2>/dev/null || realpath "$target" 2>/dev/null || printf '%s' "$target"
  elif command -v readlink >/dev/null 2>&1; then
    readlink -f "$target" 2>/dev/null || printf '%s' "$target"
  else
    printf '%s' "$target"
  fi
}

is_protected_path() {
  local path="$1"
  local name
  name="$(basename "$path")"

  case "$name" in
    CPN-Control-Panel-Network|CPN-Control-Panel-Network.git)
      return 0
      ;;
  esac

  case "$path" in
    /var/lib/cpn|/var/lib/cpn/*|/var/lib/cpn-webmail|/var/lib/cpn-webmail/*)
      return 0
      ;;
    /etc/cpn|/etc/cpn/*|/opt/cpn-webmail|/opt/cpn-webmail/*|/opt/nextcloud|/opt/nextcloud/*)
      return 0
      ;;
    /home/*/public_html|/home/*/public_html/*|/home/*/backups|/home/*/backups/*)
      return 0
      ;;
  esac
  return 1
}

path_is_under() {
  local child="$1"
  local parent="$2"
  [[ -n "$child" && -n "$parent" ]] || return 1
  [[ "$child" == "$parent" || "$child" == "$parent"/* ]]
}

detect_keep_from_pwd() {
  local pwd_path candidate parent name
  pwd_path="$(resolve_path "${PWD:-.}")"
  parent="$(dirname "$pwd_path")"
  name="$(basename "$pwd_path")"
  if [[ "$name" == cpn-build-* ]]; then
    printf '%s' "$pwd_path"
    return 0
  fi
  # Walk up a few levels (e.g. .../cpn-build-foo/target/release)
  candidate="$pwd_path"
  local i
  for i in 1 2 3 4 5 6 7 8; do
    name="$(basename "$candidate")"
    if [[ "$name" == cpn-build-* ]]; then
      printf '%s' "$candidate"
      return 0
    fi
    parent="$(dirname "$candidate")"
    [[ "$parent" == "$candidate" ]] && break
    candidate="$parent"
  done
  return 1
}

default_build_parent() {
  if [[ -n "$build_parent" ]]; then
    printf '%s' "$build_parent"
    return 0
  fi
  if [[ -d /home/cpn ]]; then
    printf '%s' /home/cpn
    return 0
  fi
  if [[ -n "${HOME:-}" && -d "${HOME}" ]]; then
    printf '%s' "$HOME"
    return 0
  fi
  printf '%s' ""
}

tree_has_active_cwd() {
  local tree="$1"
  local proc_cwd link
  [[ -d /proc ]] || return 1
  for proc_cwd in /proc/[0-9]*/cwd; do
    [[ -L "$proc_cwd" ]] || continue
    link="$(readlink "$proc_cwd" 2>/dev/null || true)"
    [[ -n "$link" ]] || continue
    link="$(resolve_path "$link")"
    if path_is_under "$link" "$tree"; then
      return 0
    fi
  done
  return 1
}

tree_named_in_build_cmdline() {
  local tree="$1"
  local base
  base="$(basename "$tree")"
  # Fallback when /proc cwd is restricted: cargo/rustc/npm command lines.
  if command -v pgrep >/dev/null 2>&1; then
    if pgrep -af '(cargo|rustc|rust-analyzer|npm|node|rpmbuild)' 2>/dev/null \
      | grep -F -- "$tree" >/dev/null 2>&1; then
      return 0
    fi
    if pgrep -af '(cargo|rustc|npm)' 2>/dev/null \
      | grep -F -- "/$base" >/dev/null 2>&1; then
      return 0
    fi
  fi
  return 1
}

should_skip_tree() {
  local tree="$1"
  local keep="$2"

  if is_protected_path "$tree"; then
    log "keep (protected): $tree"
    return 0
  fi
  if [[ -n "$keep" ]] && path_is_under "$tree" "$keep"; then
    log "keep (current build): $tree"
    return 0
  fi
  if [[ -n "$keep" ]] && path_is_under "$keep" "$tree"; then
    log "keep (current build parent): $tree"
    return 0
  fi
  if tree_has_active_cwd "$tree"; then
    log "keep (active process cwd): $tree"
    return 0
  fi
  if tree_named_in_build_cmdline "$tree"; then
    log "keep (active build cmdline): $tree"
    return 0
  fi
  return 1
}

remove_path() {
  local path="$1"
  if [[ "$dry_run" == "1" ]]; then
    log "dry-run would remove: $path"
    return 0
  fi
  if rm -rf -- "$path"; then
    log "removed: $path"
  else
    log "warning: could not remove: $path" >&2
  fi
}

cleanup_build_trees() {
  local parent keep
  parent="$(default_build_parent)"
  if [[ -z "$parent" || ! -d "$parent" ]]; then
    log "skip build-tree cleanup: no build parent directory"
    return 0
  fi
  parent="$(resolve_path "$parent")"

  keep="$keep_build"
  if [[ -z "$keep" ]]; then
    keep="$(detect_keep_from_pwd || true)"
  fi
  if [[ -n "$keep" ]]; then
    keep="$(resolve_path "$keep")"
  fi

  log "build parent: $parent"
  [[ -n "$keep" ]] && log "keep build: $keep"
  log "keep-count (extra idle): $keep_count"

  local -a candidates=()
  local -a idle=()
  local entry name resolved

  shopt -s nullglob
  for entry in "$parent"/cpn-build-*; do
    [[ -d "$entry" ]] || continue
    name="$(basename "$entry")"
    [[ "$name" == cpn-build-* ]] || continue
    resolved="$(resolve_path "$entry")"
    if is_protected_path "$resolved"; then
      log "keep (protected name/path): $resolved"
      continue
    fi
    candidates+=("$resolved")
  done
  shopt -u nullglob

  if [[ ${#candidates[@]} -eq 0 ]]; then
    log "no cpn-build-* directories under $parent"
    return 0
  fi

  for resolved in "${candidates[@]}"; do
    if should_skip_tree "$resolved" "$keep"; then
      continue
    fi
    idle+=("$resolved")
  done

  if [[ ${#idle[@]} -eq 0 ]]; then
    log "no idle cpn-build-* trees to consider"
    return 0
  fi

  # Newest first by mtime (portable: ls -td).
  local -a sorted=()
  # shellcheck disable=SC2207
  sorted=($(ls -1dt -- "${idle[@]}" 2>/dev/null || true))
  if [[ ${#sorted[@]} -eq 0 ]]; then
    sorted=("${idle[@]}")
  fi

  local idx=0
  for resolved in "${sorted[@]}"; do
    # keep_count is extra idle trees beyond the active keep / in-use set.
    if (( idx < keep_count )); then
      log "keep (newest idle #$idx): $resolved"
      idx=$((idx + 1))
      continue
    fi
    remove_path "$resolved"
    idx=$((idx + 1))
  done
}

is_safe_tmp_cpn_dir_name() {
  local name="$1"
  case "$name" in
    cpn-upgrade*|cpn-install*|cpn-release*|cpn-gpg*|cpn-extract*|cpn-tmp*|cpn-staging*|cpn-build-*)
      return 0
      ;;
    cpn-*)
      # Broad match for agent extract dirs; still requires directory + TTL.
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

cleanup_stale_tmp_dirs() {
  local root name path mtime_hours age_ok
  local now
  now="$(date +%s)"

  for root in /tmp /var/tmp; do
    [[ -d "$root" ]] || continue
    shopt -s nullglob
    for path in "$root"/cpn-*; do
      [[ -d "$path" ]] || continue
      name="$(basename "$path")"
      if ! is_safe_tmp_cpn_dir_name "$name"; then
        continue
      fi
      if is_protected_path "$path"; then
        continue
      fi
      if tree_has_active_cwd "$path"; then
        log "keep tmp (active cwd): $path"
        continue
      fi
      # Age in hours from mtime.
      if ! mtime_hours="$(stat -c '%Y' "$path" 2>/dev/null || stat -f '%m' "$path" 2>/dev/null || true)"; then
        continue
      fi
      [[ -n "$mtime_hours" ]] || continue
      age_ok=$(( (now - mtime_hours) / 3600 ))
      if (( age_ok < tmp_ttl_hours )); then
        log "keep tmp (age ${age_ok}h < ${tmp_ttl_hours}h): $path"
        continue
      fi
      remove_path "$path"
    done
    shopt -u nullglob
  done
}

main() {
  if [[ "$(uname -s 2>/dev/null || true)" != "Linux" ]]; then
    log "skip: cleanup-old-build-trees.sh is intended for Linux lab/build hosts"
    exit 0
  fi

  log "=== CPN old build-tree cleanup ==="
  [[ "$dry_run" == "1" ]] && log "mode: dry-run"
  cleanup_build_trees
  cleanup_stale_tmp_dirs
  log "=== cleanup done ==="
}

main
