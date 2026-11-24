#!/usr/bin/env python3
"""Hand-drawn 8x8 toolbench, pickaxe and axe in the Tiny Ranch palette, with the tool heads
recoloured per tier. Writes assets/src/tools.png
(56x8: toolbench | stone pickaxe, axe, sword | iron pickaxe, axe, sword)."""
import os
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PAL = {
    "O": "2e222f", "m": "7f708a", "S": "9babb2", "L": "c7dcd0", "s": "694f62", "W": "fbf5ef",
    "w": "6e2727", "r": "9e4539", "o": "cd683d", "y": "e6904e",
}
BENCH = [  # a sturdy table with a hammer and a saw blade on top
    ".LS..mS.",
    "..r.mSSm",
    "OOOOOOOO",
    "OyyyyyyO",
    "OooooooO",
    "OwOwwOwO",
    "Or....rO",
    "Ow....wO",
]
PICKAXE = [  # curved iron head, handle running down to the left
    ".mSSSm..",
    "m..yLSm.",
    "...yo.Sm",
    "..yo...m",
    ".yo.....",
    "yo......",
    "w.......",
    "........",
]
AXE = [  # blade on the right of a long handle
    "...mSS..",
    "..yLLSm.",
    "..oSLSm.",
    ".yomSm..",
    ".yo.....",
    "yo......",
    "w.......",
    "........",
]

SWORD = [  # blade up to the right, gold crossguard, wrapped grip
    "......SL",
    ".....SLm",
    "....SLm.",
    ".y.SLm..",
    "..yLm...",
    "..oy....",
    ".w..y...",
    "w.......",
]
# Tool heads are drawn with m/S/L; each tier swaps those for its own shades.
STONE = str.maketrans("mSL", "smS")  # darker, duller
IRON = str.maketrans("mSL", "SLW")  # bright steel
sprites = [BENCH] + [[r.translate(t) for r in tool] for t in (STONE, IRON) for tool in (PICKAXE, AXE, SWORD)]
out = Image.new("RGBA", (8 * len(sprites), 8))
for n, rows in enumerate(sprites):
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c != ".":
                out.putpixel((n * 8 + x, y), tuple(int(PAL[c][i:i + 2], 16) for i in (0, 2, 4)) + (255,))
out.save(os.path.join(ROOT, "assets/src/tools.png"))
