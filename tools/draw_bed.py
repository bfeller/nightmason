#!/usr/bin/env python3
"""Hand-drawn 8x8 bed in the Tiny Ranch palette: headboard, pillow, blue blanket.
Writes assets/src/bed.png."""
import os
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PAL = {
    "O": "2e222f", "r": "9e4539", "o": "cd683d", "y": "e6904e",
    "L": "c7dcd0", "S": "9babb2", "m": "7f708a",
    "B": "4d9be6", "b": "4d65b4", "d": "484a77",
}
BED = [
    ".OOOOOO.",
    "OyooooyO",
    "OLLLLLSO",
    "OSLLLSmO",
    "OBBBBBbO",
    "ObbbbbdO",
    "ObbbbbdO",
    "OrOOOOrO",
]
im = Image.new("RGBA", (8, 8))
for y, row in enumerate(BED):
    for x, ch in enumerate(row):
        if ch != ".":
            im.putpixel((x, y), tuple(int(PAL[ch][i:i + 2], 16) for i in (0, 2, 4)) + (255,))
im.save(os.path.join(ROOT, "assets", "src", "bed.png"))
