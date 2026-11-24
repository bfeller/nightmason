#!/usr/bin/env python3
"""Convert a 320x170 RGB565 framebuffer dump to PNG.  fbshot.py <in> <out.png> [scale]"""
import sys
from PIL import Image
raw = open(sys.argv[1], "rb").read(320 * 170 * 2)
im = Image.new("RGB", (320, 170))
px = [((c >> 11) << 3, ((c >> 5) & 63) << 2, (c & 31) << 3)
      for c in (int.from_bytes(raw[i:i + 2], "little") for i in range(0, len(raw), 2))]
im.putdata(px)
k = int(sys.argv[3]) if len(sys.argv) > 3 else 1
im.resize((320 * k, 170 * k), Image.NEAREST).save(sys.argv[2])
