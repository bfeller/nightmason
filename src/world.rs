//! Infinite tile world stored in 16x16 chunks (one byte per tile = 256 B per chunk).
//! Untouched chunks are regenerated from the seed on demand; edited ones are written to the save
//! (see save.rs) and read back when the player returns. Only chunks near the player stay resident,
//! so memory stays bounded no matter how much of the world has been built on.
use crate::content::{tile, Tile, GRASS, ROCK, TREE};
use crate::save::{Store, CHUNK_BYTES};
use std::cell::RefCell;
use std::collections::HashMap;

pub const CHUNK: i32 = 16;
const LOAD_RADIUS: i32 = 2; // chunks kept loaded around the player (covers the 24-tile flow field)
const KEEP_RADIUS: i32 = 3; // loaded chunks farther than this are saved if needed and dropped

pub fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA77)
        ^ seed.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// Bilinear value noise on an 8-tile lattice, 0..=255.
fn noise(x: i32, y: i32, seed: u32) -> u32 {
    let (cx, cy) = (x.div_euclid(8), y.div_euclid(8));
    let (fx, fy) = (x.rem_euclid(8) as u32, y.rem_euclid(8) as u32);
    let v = |a, b| hash(a, b, seed ^ 0x5BD1_E995) & 255;
    let top = v(cx, cy) * (8 - fx) + v(cx + 1, cy) * fx;
    let bot = v(cx, cy + 1) * (8 - fx) + v(cx + 1, cy + 1) * fx;
    (top * (8 - fy) + bot * fy) / 64
}

/// Two independent noise fields: rocky outcrops and forests.
pub fn generate(x: i32, y: i32, seed: u32) -> Tile {
    if x * x + y * y < 10 {
        return GRASS; // clear spawn
    }
    let h = hash(x, y, seed);
    let n = noise(x, y, seed).saturating_sub(96);
    if h & 255 < (n * n) >> 7 {
        return ROCK;
    }
    let f = noise(x, y, seed ^ 0xA5A5_1234).saturating_sub(100);
    if (h >> 8) & 255 < (f * f) >> 7 { TREE } else { GRASS }
}

struct Chunk {
    tiles: [Tile; CHUNK_BYTES],
    /// Differs from what the seed generates, so it lives in the save.
    edited: bool,
    /// Changed since it was last written.
    unsaved: bool,
}

pub struct World {
    pub seed: u32,
    /// Interior mutability so `get` can page chunks in on demand and is always correct, however
    /// far from the player it's asked about.
    chunks: RefCell<HashMap<(i32, i32), Box<Chunk>>>,
    store: RefCell<Option<Store>>,
    damage: HashMap<(i32, i32), u8>,
}

fn split(x: i32, y: i32) -> ((i32, i32), usize) {
    let k = (x.div_euclid(CHUNK), y.div_euclid(CHUNK));
    (k, (y.rem_euclid(CHUNK) * CHUNK + x.rem_euclid(CHUNK)) as usize)
}

impl World {
    pub fn new(seed: u32, store: Option<Store>) -> World {
        World { seed, chunks: RefCell::new(HashMap::new()), store: RefCell::new(store), damage: HashMap::new() }
    }

    fn make(&self, k: (i32, i32)) -> Box<Chunk> {
        let mut c = Box::new(Chunk { tiles: [GRASS; CHUNK_BYTES], edited: false, unsaved: false });
        if let Some(s) = self.store.borrow_mut().as_mut() {
            c.edited = s.load(k, &mut c.tiles);
        }
        if !c.edited {
            for (i, t) in c.tiles.iter_mut().enumerate() {
                let i = i as i32;
                *t = generate(k.0 * CHUNK + i % CHUNK, k.1 * CHUNK + i / CHUNK, self.seed);
            }
        }
        c
    }

    fn with_chunk<R>(&self, k: (i32, i32), f: impl FnOnce(&mut Chunk) -> R) -> R {
        let mut m = self.chunks.borrow_mut();
        f(m.entry(k).or_insert_with(|| self.make(k)))
    }

    pub fn get(&self, x: i32, y: i32) -> Tile {
        let (k, i) = split(x, y);
        if let Some(c) = self.chunks.borrow().get(&k) {
            return c.tiles[i];
        }
        self.with_chunk(k, |c| c.tiles[i])
    }

    /// The tile if its chunk is resident, without loading anything.
    pub fn peek(&self, x: i32, y: i32) -> Option<Tile> {
        let (k, i) = split(x, y);
        self.chunks.borrow().get(&k).map(|c| c.tiles[i])
    }

    pub fn blocks(&self, x: i32, y: i32, zombie: bool) -> bool {
        let d = tile(self.get(x, y));
        if zombie { d.blocks_zombie } else { d.blocks_player }
    }

    pub fn set(&mut self, x: i32, y: i32, t: Tile) {
        let (k, i) = split(x, y);
        self.with_chunk(k, |c| {
            c.tiles[i] = t;
            c.edited = true;
            c.unsaved = true;
        });
        self.damage.remove(&(x, y));
    }

    pub fn damage(&self, x: i32, y: i32) -> u8 {
        self.damage.get(&(x, y)).copied().unwrap_or(0)
    }

    /// Adds damage to a tile and returns the new total.
    pub fn hit(&mut self, x: i32, y: i32, n: u8) -> u8 {
        let d = self.damage.entry((x, y)).or_insert(0);
        *d = d.saturating_add(n);
        *d
    }

    /// Tiles of resident chunks that match `f` (used to find turrets as chunks load).
    pub fn find(&self, f: impl Fn(Tile) -> bool) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for (&(cx, cy), c) in self.chunks.borrow().iter() {
            for (i, &t) in c.tiles.iter().enumerate() {
                if f(t) {
                    out.push((cx * CHUNK + i as i32 % CHUNK, cy * CHUNK + i as i32 / CHUNK));
                }
            }
        }
        out
    }

    /// Load chunks around the player; save and drop distant ones. Without a save, edited chunks
    /// stay in memory.
    pub fn maintain(&mut self, tx: i32, ty: i32) {
        let (pcx, pcy) = (tx.div_euclid(CHUNK), ty.div_euclid(CHUNK));
        for cy in pcy - LOAD_RADIUS..=pcy + LOAD_RADIUS {
            for cx in pcx - LOAD_RADIUS..=pcx + LOAD_RADIUS {
                self.with_chunk((cx, cy), |_| ());
            }
        }
        let mut store = self.store.borrow_mut();
        self.chunks.get_mut().retain(|&k, c| {
            if (k.0 - pcx).abs() <= KEEP_RADIUS && (k.1 - pcy).abs() <= KEEP_RADIUS {
                return true;
            }
            if !c.edited {
                return false;
            }
            match store.as_mut() {
                // Keep it in memory if the write fails rather than lose the player's work.
                Some(s) if c.unsaved => s.store(k, &c.tiles).is_err(),
                Some(_) => false,
                None => true,
            }
        });
        let chunks = self.chunks.get_mut();
        self.damage.retain(|&(x, y), _| chunks.contains_key(&split(x, y).0));
    }

    /// Write every changed chunk. Returns false if there's no save or a write failed.
    pub fn flush(&mut self) -> bool {
        let Some(s) = self.store.get_mut().as_mut() else { return false };
        let mut ok = true;
        for (&k, c) in self.chunks.get_mut().iter_mut() {
            if c.unsaved {
                match s.store(k, &c.tiles) {
                    Ok(()) => c.unsaved = false,
                    Err(_) => ok = false,
                }
            }
        }
        ok
    }

    pub fn store(&self) -> std::cell::Ref<'_, Option<Store>> {
        self.store.borrow()
    }

    pub fn take_store(&mut self) -> Option<Store> {
        self.store.get_mut().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::WALL;

    #[test]
    fn edited_chunks_survive_eviction_and_restart() {
        let dir = std::env::temp_dir().join(format!("nm-world-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut w = World::new(7, Some(Store::open(dir.clone()).unwrap()));
        w.set(-3, 5, WALL);
        w.set(100, -40, WALL);
        w.maintain(0, 0);
        w.maintain(5000, 5000); // everything near the origin is paged out
        assert!(w.peek(-3, 5).is_none(), "far chunk should be evicted");
        assert!(w.chunks.borrow().len() <= 49);
        assert_eq!(w.get(-3, 5), WALL); // paged back in from disk
        assert_eq!(w.get(100, -40), WALL);
        assert_eq!(w.get(-4, 5), generate(-4, 5, 7));
        // A fresh World on the same directory sees the edits too.
        let w2 = World::new(7, Some(Store::open(dir.clone()).unwrap()));
        assert_eq!(w2.get(-3, 5), WALL);
        assert_eq!(w2.get(100, -40), WALL);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
