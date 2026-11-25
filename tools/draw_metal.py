#!/usr/bin/env python3
"""Metal walls (riveted plates) and doors (the Tiny Farm door with its planks turned to banded
metal), in iron and silver. Writes assets/src/metal.png (32x8: iron wall, iron door, silver
wall, silver door)."""
import os
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BASE = (69, 41, 63)
METALS = [  # (highlight, plate, seam, rivet)
    ((170, 185, 200), (110, 125, 145), (70, 78, 98), (225, 235, 245)),  # iron
    ((236, 243, 252), (178, 200, 230), (96, 116, 168), (255, 255, 255)),  # silver
]
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
sheet = Image.open(os.path.join(ROOT, "assets/src/TinyFarm_Structures.png")).convert("RGBA")
out = Image.new("RGBA", (16 * len(METALS), 8))
for n, (HI, MID, SEAM, RIVET) in enumerate(METALS):
    key = {"H": HI, "M": MID, "S": SEAM, "R": RIVET, "E": BASE}
    for y, row in enumerate(WALL):
        for x, k in enumerate(row):
            out.putpixel((n * 16 + x, y), key[k] + (255,))
    # Door: keep the stone frame, swap the red planks for metal and add two riveted bands.
    door = sheet.crop((3 * 8, 3 * 8, 4 * 8, 4 * 8))
    planks = {(234, 79, 54): HI, (179, 56, 49): MID, (46, 34, 47): SEAM}
    for y in range(8):
        for x in range(8):
            p = door.getpixel((x, y))
            if p[:3] in planks:
                door.putpixel((x, y), planks[p[:3]] + (255,))
    for y in (3, 6):
        for x in range(2, 6):
            door.putpixel((x, y), (RIVET if x in (2, 5) else SEAM) + (255,))
    out.alpha_composite(door, (n * 16 + 8, 0))
out.save(os.path.join(ROOT, "assets/src/metal.png"))
