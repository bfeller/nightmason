#!/usr/bin/env python3
"""Iron wall (riveted plates) and iron door (the Tiny Farm door with its planks turned to banded
steel). Writes assets/src/iron.png (16x8)."""
import os
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HI, MID, SEAM, RIVET, BASE = (170, 185, 200), (110, 125, 145), (70, 78, 98), (225, 235, 245), (69, 41, 63)
WALL = [  # H highlight, M plate, S seam, R rivet, E base shadow
    "HHHHHHHH",
    "HMMMMMMS",
    "HMRMMRMS",
    "HMMMMMMS",
    "SSSSSSSS",
    "HMRMMRMS",
    "HMMMMMMS",
    "EEEEEEEE",
]
KEY = {"H": HI, "M": MID, "S": SEAM, "R": RIVET, "E": BASE}

out = Image.new("RGBA", (16, 8))
for y, row in enumerate(WALL):
    for x, k in enumerate(row):
        out.putpixel((x, y), KEY[k] + (255,))

# Door: keep the stone frame, swap the red planks for steel and add two riveted bands.
sheet = Image.open(os.path.join(ROOT, "assets/src/TinyFarm_Structures.png")).convert("RGBA")
door = sheet.crop((3 * 8, 3 * 8, 4 * 8, 4 * 8))
PLANKS = {(234, 79, 54): HI, (179, 56, 49): MID, (46, 34, 47): SEAM}
for y in range(8):
    for x in range(8):
        p = door.getpixel((x, y))
        if p[:3] in PLANKS:
            door.putpixel((x, y), PLANKS[p[:3]] + (255,))
for y in (3, 6):
    for x in range(2, 6):
        door.putpixel((x, y), (RIVET if x in (2, 5) else SEAM) + (255,))
out.alpha_composite(door, (8, 0))
out.save(os.path.join(ROOT, "assets/src/iron.png"))
