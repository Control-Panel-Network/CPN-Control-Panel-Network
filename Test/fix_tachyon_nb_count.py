#!/usr/bin/env python3
"""One-shot live fix: Tachyon/SnappyMail nb %count% -> %COUNT% in NEW_MESSAGE_NOTIFICATION."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOTS = [
    Path("/opt/cpn-webmail/tachyon"),
    Path("/opt/cpn-webmail/snappymail"),
]


def find_nb_user_json(root: Path) -> list[Path]:
    if not root.is_dir():
        return []
    return list(root.glob("**/app/localization/nb/user.json"))


def find_notification(obj):
    if isinstance(obj, dict):
        if "NEW_MESSAGE_NOTIFICATION" in obj:
            return obj["NEW_MESSAGE_NOTIFICATION"]
        for v in obj.values():
            found = find_notification(v)
            if found is not None:
                return found
    elif isinstance(obj, list):
        for v in obj:
            found = find_notification(v)
            if found is not None:
                return found
    return None


def fix_file(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    pattern = re.compile(r'("NEW_MESSAGE_NOTIFICATION"\s*:\s*")([^"]*)(")')

    def repl(m: re.Match) -> str:
        value = m.group(2)
        fixed = re.sub(r"%[Cc][Oo][Uu][Nn][Tt]%", "%COUNT%", value)
        return f"{m.group(1)}{fixed}{m.group(3)}"

    new_text, n = pattern.subn(repl, text, count=1)
    if n == 0:
        return f"SKIP no NEW_MESSAGE_NOTIFICATION in {path}"
    if new_text == text:
        data = json.loads(text)
        current = find_notification(data)
        return f"OK already {path}: {current!r}"

    bak = path.with_suffix(".json.bak-countfix")
    if not bak.exists():
        bak.write_text(text, encoding="utf-8")
    path.write_text(new_text, encoding="utf-8")
    data = json.loads(new_text)
    current = find_notification(data)
    return f"FIXED {path}: {current!r}"


def main() -> int:
    paths: list[Path] = []
    for root in ROOTS:
        paths.extend(find_nb_user_json(root))
    if len(sys.argv) > 1:
        paths = [Path(p) for p in sys.argv[1:]]
    if not paths:
        print("No nb/user.json found under webmail roots")
        return 1
    for path in paths:
        print(fix_file(path))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
