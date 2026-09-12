#!/usr/bin/env python3
"""Generate CPN favicon.ico / PNG marks from the brand geometry (PIL)."""

from __future__ import annotations

import io
import struct
from pathlib import Path

from PIL import Image, ImageDraw

BLUE = (0, 108, 250, 255)
CHAR = (22, 31, 43, 255)
GRAY = (200, 205, 213, 255)
TRACK = (228, 231, 236, 255)
DOT = (191, 217, 255, 255)
WHITE = (255, 255, 255, 255)

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "installer-ui" / "src" / "assets"


def draw_mark(size: int) -> Image.Image:
    im = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    s = size / 32.0

    def box(x0: float, y0: float, x1: float, y1: float):
        return (x0 * s, y0 * s, x1 * s, y1 * s)

    d.rounded_rectangle(
        box(2.5, 3.5, 23.5, 22.5),
        radius=max(1, 3.2 * s),
        outline=CHAR,
        width=max(1, round(1.7 * s)),
    )
    d.rounded_rectangle(box(2.5, 3.5, 23.5, 9.2), radius=max(1, 3.2 * s), fill=BLUE)
    d.rectangle(box(2.5, 6.5, 23.5, 9.2), fill=BLUE)
    for cx, alpha in ((5.6, 230), (7.7, 180), (9.8, 130)):
        r = max(1.0, 0.65 * s)
        col = (DOT[0], DOT[1], DOT[2], alpha)
        d.ellipse((cx * s - r, 5.85 * s - r, cx * s + r, 5.85 * s + r), fill=col)
    d.rounded_rectangle(box(5.2, 10.2, 8.6, 13.6), radius=max(1, 0.7 * s), fill=BLUE)
    d.rounded_rectangle(box(5.2, 14.4, 8.6, 17.8), radius=max(1, 0.7 * s), fill=GRAY)
    d.rounded_rectangle(box(5.2, 18.6, 8.6, 21.0), radius=max(1, 0.7 * s), fill=GRAY)
    d.rounded_rectangle(box(10.6, 11.1, 19.8, 13.3), radius=max(1, 1.1 * s), fill=TRACK)
    r = max(1.0, 1.35 * s)
    d.ellipse((17.8 * s - r, 12.2 * s - r, 17.8 * s + r, 12.2 * s + r), fill=BLUE)
    d.rounded_rectangle(box(10.6, 15.5, 19.8, 17.7), radius=max(1, 1.1 * s), fill=TRACK)
    d.ellipse(
        (12.6 * s - r, 16.6 * s - r, 12.6 * s + r, 16.6 * s + r),
        fill=WHITE,
        outline=BLUE,
        width=max(1, round(s)),
    )
    nodes = [(20.2, 22.4), (24.6, 19.6), (28.4, 21.2), (26.8, 24.8)]
    edges = [(0, 1), (0, 3), (1, 2), (1, 3), (2, 3)]
    lw = max(1, round(1.15 * s))
    for a, b in edges:
        d.line(
            (nodes[a][0] * s, nodes[a][1] * s, nodes[b][0] * s, nodes[b][1] * s),
            fill=CHAR,
            width=lw,
        )
    sizes = [2.15, 1.55, 1.35, 1.55]
    fills = [BLUE, CHAR, CHAR, CHAR]
    for (cx, cy), rad, fill in zip(nodes, sizes, fills):
        rr = max(1.0, rad * s)
        d.ellipse((cx * s - rr, cy * s - rr, cx * s + rr, cy * s + rr), fill=fill)
    return im


def png_bytes(im: Image.Image) -> bytes:
    buf = io.BytesIO()
    im.save(buf, format="PNG")
    return buf.getvalue()


def ico_bytes(images: list[tuple[int, bytes]]) -> bytes:
    count = len(images)
    header = struct.pack("<HHH", 0, 1, count)
    offset = 6 + 16 * count
    entries = b""
    data = b""
    for size, png in images:
        w = 0 if size >= 256 else size
        h = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", w, h, 0, 0, 1, 32, len(png), offset)
        offset += len(png)
        data += png
    return header + entries + data


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    mark16 = draw_mark(16)
    mark32 = draw_mark(32)
    mark48 = draw_mark(48)
    mark180 = draw_mark(180)
    mark32.save(OUT / "cpn-brand-mark.png")
    mark180.save(OUT / "apple-touch-icon.png")
    ico = ico_bytes(
        [
            (16, png_bytes(mark16)),
            (32, png_bytes(mark32)),
            (48, png_bytes(mark48)),
        ]
    )
    (OUT / "favicon.ico").write_bytes(ico)
    svg = (OUT / "cpn-brand-mark.svg").read_text(encoding="utf-8")
    (OUT / "favicon.svg").write_text(svg, encoding="utf-8")
    for name in (
        "favicon.ico",
        "favicon.svg",
        "cpn-brand-mark.svg",
        "cpn-brand-mark.png",
        "apple-touch-icon.png",
    ):
        path = OUT / name
        print(f"{name}: {path.stat().st_size} bytes")


if __name__ == "__main__":
    main()
