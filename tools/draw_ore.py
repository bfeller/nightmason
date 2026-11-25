#!/usr/bin/env python3
"""Iron and silver ore: the two Tiny Ranch rocks with rusty or pale blue-white specks.
Writes assets/src/ore.png (32x8: iron, iron, silver, silver)."""
import os
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUST, GLINT, DARK = (0xcd, 0x68, 0x3d), (0xfb, 0xb9, 0x54), (0x9e, 0x45, 0x39)
SPECKS = [  # (x, y, colour) per rock
    [(3, 3, GLINT), (4, 3, RUST), (2, 5, RUST), (5, 5, GLINT), (5, 6, DARK), (3, 6, RUST)],
    [(3, 3, GLINT), (2, 4, RUST), (5, 3, RUST), (4, 5, GLINT), (5, 5, RUST), (3, 6, DARK)],
]
# Silver: the same speck layout in cold white and blue.
SILVER = {RUST: (0x5f, 0x9c, 0xf0), GLINT: (0xff, 0xff, 0xff), DARK: (0x3a, 0x4c, 0xb0)}
sheet = Image.open(os.path.join(ROOT, "assets/src/TinyFarm_MapDecorations.png")).convert("RGBA")
out = Image.new("RGBA", (32, 8))
for n, recolour in enumerate((lambda c: c, lambda c: SILVER[c])):
    for i, specks in enumerate(SPECKS):
        rock = sheet.crop(((5 + i) * 8, 0, (6 + i) * 8, 8))
        for x, y, c in specks:
            rock.putpixel((x, y), recolour(c) + (255,))
        out.alpha_composite(rock, ((n * 2 + i) * 8, 0))
out.save(os.path.join(ROOT, "assets/src/ore.png"))
