#!/usr/bin/env python3
"""Iron ore: the two Tiny Ranch rocks with rusty specks. Writes assets/src/ore.png (16x8)."""
import os
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUST, GLINT, DARK = (0xcd, 0x68, 0x3d), (0xfb, 0xb9, 0x54), (0x9e, 0x45, 0x39)
SPECKS = [  # (x, y, colour) per rock
    [(3, 3, GLINT), (4, 3, RUST), (2, 5, RUST), (5, 5, GLINT), (5, 6, DARK), (3, 6, RUST)],
    [(3, 3, GLINT), (2, 4, RUST), (5, 3, RUST), (4, 5, GLINT), (5, 5, RUST), (3, 6, DARK)],
]
sheet = Image.open(os.path.join(ROOT, "assets/src/TinyFarm_MapDecorations.png")).convert("RGBA")
out = Image.new("RGBA", (16, 8))
for i, specks in enumerate(SPECKS):
    rock = sheet.crop(((5 + i) * 8, 0, (6 + i) * 8, 8))
    for x, y, c in specks:
        rock.putpixel((x, y), c + (255,))
    out.alpha_composite(rock, (i * 8, 0))
out.save(os.path.join(ROOT, "assets/src/ore.png"))
