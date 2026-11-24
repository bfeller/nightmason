#!/usr/bin/env python3
"""Hand-drawn 8x8 wood floor in the Tiny Ranch palette: staggered planks that tile seamlessly.
Writes assets/src/floor.png."""
import os
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PAL = {"w": "6e2727", "r": "9e4539", "o": "cd683d", "y": "e6904e"}
FLOOR = [  # two long planks per tile with soft seams, a little grain and one butt joint
    "yyyyyyyy",
    "ooooyooo",
    "oooooooo",
    "rrrrrrrr",
    "yyyywyyy",
    "oyoowooo",
    "oooowooo",
    "rrrrrrrr",
]
im = Image.new("RGBA", (8, 8))
for y, row in enumerate(FLOOR):
    for x, ch in enumerate(row):
        im.putpixel((x, y), tuple(int(PAL[ch][i:i + 2], 16) for i in (0, 2, 4)) + (255,))
im.save(os.path.join(ROOT, "assets", "src", "floor.png"))
