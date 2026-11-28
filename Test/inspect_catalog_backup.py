#!/usr/bin/env python3
import json
from pathlib import Path

p = Path("/var/lib/cpn/plugin-catalog-cache.json")
d = json.loads(p.read_text())
entries = d.get("entries") or d.get("plugins") or []
print("keys", list(d)[:12], "n", len(entries))
for e in entries:
    if not isinstance(e, dict):
        continue
    i = e.get("id") or ""
    cat = (e.get("category") or "").lower()
    if (
        "backup" in i.lower()
        or "contabo" in i.lower()
        or cat in ("backup", "backups")
    ):
        print(
            i,
            "cat=",
            e.get("category"),
            "host=",
            e.get("host_scoped"),
            "site=",
            e.get("site_installable"),
            "cpn=",
            e.get("cpn_installable"),
            "name=",
            e.get("name"),
        )
