#!/usr/bin/env python3
"""Hand-drawn 8x16 ballista tower in the Tiny Ranch palette: a wall block with a bolt thrower on
top. Writes assets/src/ballista.png (16x16: loaded | fired)."""
import os
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PAL = {
    "O": "2e222f", "m": "7f708a", "S": "9babb2", "L": "c7dcd0",
    "w": "6e2727", "r": "9e4539", "o": "cd683d", "y": "e6904e",
}
LOADED = [  # bow drawn, bolt on the rail
    "...OO...",
    "...LS...",
    "O..mm..O",
    "OyOomOyO",
    ".OyoooO.",
    "..OSSO..",
    "..OroO..",
    ".OrrrrO.",
    "OrrrrrrO",
]
FIRED = [  # bow sprung, rail empty
    "........",
    "........",
    "OO....OO",
    "OyOOOOyO",
    ".OyoooO.",
    "..OSSO..",
    "..OroO..",
    ".OrrrrO.",
    "OrrrrrrO",
]

def frame(rows):
    im = Image.new("RGBA", (8, 16))
    wall = Image.open(os.path.join(ROOT, "assets/src/TinyFarm_Structures.png")).convert("RGBA")
    im.alpha_composite(wall.crop((16, 24, 24, 32)), (0, 8))
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c != ".":
                im.putpixel((x, y), tuple(int(PAL[c][i:i + 2], 16) for i in (0, 2, 4)) + (255,))
    return im

out = Image.new("RGBA", (16, 16))
out.alpha_composite(frame(LOADED), (0, 0))
out.alpha_composite(frame(FIRED), (8, 0))
out.save(os.path.join(ROOT, "assets/src/ballista.png"))
