//! 160x85 RGB565 canvas, drawn at 2x onto the 320x170 framebuffer.
pub const W: i32 = 160;
pub const H: i32 = 85;
pub const FB_W: usize = 320;
pub const FB_H: usize = 170;

static DATA: &[u8] = include_bytes!("../assets/sprites.bin");

pub const fn rgb(r: u32, g: u32, b: u32) -> u16 {
    (((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)) as u16
}

pub struct Canvas {
    pub px: Vec<u16>,
    /// Per-pixel light level scratch buffer for `light`.
    lm: Vec<u16>,
    pal: [u16; 256],
    spr: &'static [u8],
}

/// 3x5 font, rows top to bottom, 3 bits per row.
fn glyph(c: char) -> u16 {
    match c.to_ascii_uppercase() {
        '0' => 0b111_101_101_101_111, '1' => 0b010_110_010_010_111,
        '2' => 0b111_001_111_100_111, '3' => 0b111_001_011_001_111,
        '4' => 0b101_101_111_001_001, '5' => 0b111_100_111_001_111,
        '6' => 0b111_100_111_101_111, '7' => 0b111_001_010_010_010,
        '8' => 0b111_101_111_101_111, '9' => 0b111_101_111_001_111,
        'A' => 0b010_101_111_101_101, 'B' => 0b110_101_110_101_110,
        'C' => 0b011_100_100_100_011, 'D' => 0b110_101_101_101_110,
        'E' => 0b111_100_110_100_111, 'F' => 0b111_100_110_100_100,
        'G' => 0b011_100_101_101_011, 'H' => 0b101_101_111_101_101,
        'I' => 0b111_010_010_010_111, 'J' => 0b001_001_001_101_010,
        'K' => 0b101_101_110_101_101, 'L' => 0b100_100_100_100_111,
        'M' => 0b101_111_111_101_101, 'N' => 0b110_101_101_101_101,
        'O' => 0b010_101_101_101_010, 'P' => 0b110_101_110_100_100,
        'Q' => 0b010_101_101_110_011, 'R' => 0b110_101_110_101_101,
        'S' => 0b011_100_010_001_110, 'T' => 0b111_010_010_010_010,
        'U' => 0b101_101_101_101_111, 'V' => 0b101_101_101_101_010,
        'W' => 0b101_101_111_111_101, 'X' => 0b101_101_010_101_101,
        'Y' => 0b101_101_010_010_010, 'Z' => 0b111_001_010_100_111,
        ':' => 0b000_010_000_010_000, '!' => 0b010_010_010_000_010,
        '-' => 0b000_000_111_000_000, '/' => 0b001_001_010_100_100,
        '.' => 0b000_000_000_000_010, '*' => 0b000_101_010_101_000,
        '>' => 0b100_010_001_010_100, '<' => 0b001_010_100_010_001,
        '?' => 0b111_001_010_000_010, ',' => 0b000_000_000_010_100,
        '+' => 0b000_010_111_010_000,
        '↑' => 0b000_010_111_000_000, '↓' => 0b000_000_111_010_000,
        _ => 0,
    }
}

impl Canvas {
    pub fn new() -> Canvas {
        let n = DATA[0] as usize;
        let mut pal = [0u16; 256];
        for i in 0..n {
            pal[i + 1] = u16::from_le_bytes([DATA[1 + i * 2], DATA[2 + i * 2]]);
        }
        Canvas { px: vec![0; (W * H) as usize], lm: vec![0; (W * H) as usize], pal, spr: &DATA[1 + n * 2..] }
    }

    #[inline]
    pub fn pixel(&mut self, x: i32, y: i32, c: u16) {
        if (0..W).contains(&x) && (0..H).contains(&y) {
            self.px[(y * W + x) as usize] = c;
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: u16) {
        for yy in y.max(0)..(y + h).min(H) {
            for xx in x.max(0)..(x + w).min(W) {
                self.px[(yy * W + xx) as usize] = c;
            }
        }
    }

    /// Halve the brightness of a rectangle (shadows, overlays).
    pub fn darken(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for yy in y.max(0)..(y + h).min(H) {
            for xx in x.max(0)..(x + w).min(W) {
                let p = &mut self.px[(yy * W + xx) as usize];
                *p = (*p >> 1) & 0b01111_011111_01111;
            }
        }
    }

    /// Draw an 8x8 sprite; `solid` paints every opaque pixel one colour (hit flash).
    pub fn sprite(&mut self, id: usize, x: i32, y: i32, flip: bool, solid: Option<u16>) {
        let s = &self.spr[id * 64..id * 64 + 64];
        for j in 0..8 {
            for i in 0..8 {
                let idx = s[(j * 8 + if flip { 7 - i } else { i }) as usize];
                if idx != 0 {
                    let c = solid.unwrap_or(self.pal[idx as usize]);
                    self.pixel(x + i, y + j, c);
                }
            }
        }
    }

    /// Draw a w x h block sprite made of consecutive 8x8 blocks (see pack_sprites.py).
    pub fn sprite_big(&mut self, id: usize, w: i32, h: i32, x: i32, y: i32, flip: bool) {
        for j in 0..h {
            for i in 0..w {
                let col = if flip { w - 1 - i } else { i };
                self.sprite(id + (j * w + col) as usize, x + i * 8, y + j * 8, flip, None);
            }
        }
    }

    /// Dithered one-colour silhouette (a sprite seen through whatever is in front of it).
    pub fn sprite_ghost(&mut self, id: usize, x: i32, y: i32, flip: bool, c: u16) {
        let s = &self.spr[id * 64..id * 64 + 64];
        for j in 0..8 {
            for i in 0..8 {
                if (i + j) % 2 == 0 && s[(j * 8 + if flip { 7 - i } else { i }) as usize] != 0 {
                    self.pixel(x + i, y + j, c);
                }
            }
        }
    }

    pub fn text(&mut self, x: i32, y: i32, s: &str, c: u16, scale: i32) {
        let mut cx = x;
        for ch in s.chars() {
            let g = glyph(ch);
            for r in 0..5 {
                for b in 0..3 {
                    if g >> (14 - (r * 3 + b)) & 1 == 1 {
                        let (px, py) = (cx + b * scale, y + r * scale);
                        self.fill(px + 1, py + 1, scale, scale, 0);
                        self.fill(px, py, scale, scale, c);
                    }
                }
            }
            cx += 4 * scale;
        }
    }

    pub fn text_centered(&mut self, y: i32, s: &str, c: u16, scale: i32) {
        let w = s.chars().count() as i32 * 4 * scale - scale;
        self.text((W - w) / 2, y, s, c, scale);
    }

    /// Darken by ambient light `amb` (0..=256), brightened by point lights (x, y, radius).
    /// Darken to `amb` (256 = full daylight) except around point lights (x, y, radius). Each light
    /// only touches its own square, so many small lights stay cheap.
    pub fn light(&mut self, amb: i32, lights: &[(i32, i32, i32)]) {
        self.lm.fill(amb as u16);
        for &(lx, ly, r) in lights {
            for y in (ly - r).max(0)..(ly + r).min(H) {
                for x in (lx - r).max(0)..(lx + r).min(W) {
                    let d2 = (x - lx).pow(2) + (y - ly).pow(2);
                    if d2 < r * r {
                        let l = &mut self.lm[(y * W + x) as usize];
                        *l = (*l).max((256 - 256 * d2 / (r * r)) as u16);
                    }
                }
            }
        }
        for (p, &l) in self.px.iter_mut().zip(&self.lm) {
            let l = l as i32;
            if l >= 256 {
                continue;
            }
            let l = (l / 32 * 32).max(amb) as u32; // banded, retro falloff
            let lb = l + (256 - l) / 4; // keep a blue tint in the dark
            let c = *p as u32;
            let (r, g, b) = (c >> 11, (c >> 5) & 63, c & 31);
            *p = (((r * l >> 8) << 11) | ((g * l >> 8) << 5) | ((b * lb >> 8).max(2))) as u16;
        }
    }

    /// Scale 2x into a little-endian RGB565 framebuffer image.
    pub fn present(&self, out: &mut [u8]) {
        for y in 0..FB_H {
            let src = &self.px[(y / 2) * W as usize..(y / 2 + 1) * W as usize];
            let row = &mut out[y * FB_W * 2..(y + 1) * FB_W * 2];
            for (i, &c) in src.iter().enumerate() {
                let [lo, hi] = c.to_le_bytes();
                row[i * 4..i * 4 + 4].copy_from_slice(&[lo, hi, lo, hi]);
            }
        }
    }
}
