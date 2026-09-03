#!/usr/bin/env bash
set -euo pipefail
if [[ $# -eq 0 ]]; then
  shopt -s nullglob
  set -- dist/* target/rpmbuild/RPMS/*/*.rpm target/release/cpn-installer
fi
files=()
for path in "$@"; do
  [[ -f "$path" ]] && files+=("$path")
done
if ((${#files[@]} == 0)); then
  echo "No artifacts found." >&2
  exit 1
fi
out_dir="$(cd "$(dirname "${files[0]}")" && pwd)"
out="$out_dir/SHA256SUMS"
: >"$out"
for path in "${files[@]}"; do
  (cd "$(dirname "$path")" && sha256sum "$(basename "$path")") >>"$out"
done
echo "Wrote $out"
cat "$out"
