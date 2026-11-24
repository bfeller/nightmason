#!/usr/bin/env python3
"""Hand-drawn 8x8 torch in the Tiny Ranch palette: a flame on a stake. Writes assets/src/torch.png."""
import os
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PAL = {
    "O": "2e222f", "w": "6e2727", "r": "9e4539", "o": "cd683d", "y": "e6904e", "g": "fbb954",
    "W": "fbf5ef",
}
TORCH = [
    "....g...",
    "...gyg..",
    "...yWg..",
    "...oyo..",
    "...OrO..",
    "....r...",
    "....w...",
    "...OwO..",
]
im = Image.new("RGBA", (8, 8))
for y, row in enumerate(TORCH):
    for x, ch in enumerate(row):
        if ch != ".":
            im.putpixel((x, y), tuple(int(PAL[ch][i:i + 2], 16) for i in (0, 2, 4)) + (255,))
im.save(os.path.join(ROOT, "assets", "src", "torch.png"))
