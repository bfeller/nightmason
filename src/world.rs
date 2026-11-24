//! Infinite tile world stored in 16x16 chunks (one byte per tile = 256 B per chunk).
//! Untouched chunks are regenerated from the seed on demand; edited ones are written to the save
//! (see save.rs) and read back when the player returns. Only chunks near the player stay resident,
//! so memory stays bounded no matter how much of the world has been built on.
use crate::content::{tile, Tile, GRASS, IRON_ORE, ROCK, TREE};
use crate::save::{Layer, Store, CHUNK_BYTES};
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

/// The world gets one danger tier wilder every this many tiles from spawn: tougher zombies, and
/// more of the rocks are iron ore.
pub const TIER_TILES: f32 = 40.0;

/// Danger tier at a point in tile units: 1 around spawn, rising with distance.
pub fn tier_at(tx: f32, ty: f32) -> u64 {
    1 + ((tx * tx + ty * ty).sqrt() / TIER_TILES) as u64
}

/// Two independent noise fields: rocky outcrops and forests. Out past tier 1 some rocks are iron
/// ore, about 6% more of them each tier (up to ~40%).
pub fn generate(x: i32, y: i32, seed: u32) -> Tile {
    if x * x + y * y < 10 {
        return GRASS; // clear spawn
    }
    let h = hash(x, y, seed);
    let n = noise(x, y, seed).saturating_sub(96);
    if h & 255 < (n * n) >> 7 {
        let iron = ((tier_at(x as f32, y as f32) - 1) * 16).min(100) as u32;
        return if (h >> 16) & 255 < iron { IRON_ORE } else { ROCK };
    }
    let f = noise(x, y, seed ^ 0xA5A5_1234).saturating_sub(100);
    if (h >> 8) & 255 < (f * f) >> 7 { TREE } else { GRASS }
}

struct Chunk {
    tiles: [Tile; CHUNK_BYTES],
    /// What's underfoot (floors); GRASS = plain ground. Only allocated once something is laid.
    ground: Option<Box<[Tile; CHUNK_BYTES]>>,
    /// Tiles differ from what the seed generates, so they live in the save.
    edited: bool,
    /// Changed since it was last written.
    unsaved: bool,
}

impl Chunk {
    /// Has anything the seed can't regenerate.
    fn keep(&self) -> bool {
        self.edited || self.ground.is_some()
    }
    fn write(&self, s: &mut Store, k: (i32, i32)) -> std::io::Result<()> {
        if self.edited {
            s.store(Layer::Tiles, k, &self.tiles)?;
        }
        if let Some(g) = &self.ground {
            s.store(Layer::Ground, k, g)?;
        }
        Ok(())
    }
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
        let mut c = Box::new(Chunk { tiles: [GRASS; CHUNK_BYTES], ground: None, edited: false, unsaved: false });
        if let Some(s) = self.store.borrow_mut().as_mut() {
            c.edited = s.load(Layer::Tiles, k, &mut c.tiles);
            let mut g = Box::new([GRASS; CHUNK_BYTES]);
            if s.load(Layer::Ground, k, &mut g) {
                c.ground = Some(g);
            }
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

    /// The ground layer (floors) at a tile; GRASS if nothing has been laid.
    pub fn ground(&self, x: i32, y: i32) -> Tile {
        let (k, i) = split(x, y);
        if let Some(c) = self.chunks.borrow().get(&k) {
            return c.ground.as_ref().map_or(GRASS, |g| g[i]);
        }
        self.with_chunk(k, |c| c.ground.as_ref().map_or(GRASS, |g| g[i]))
    }

    pub fn set_ground(&mut self, x: i32, y: i32, t: Tile) {
        let (k, i) = split(x, y);
        self.with_chunk(k, |c| {
            c.ground.get_or_insert_with(|| Box::new([GRASS; CHUNK_BYTES]))[i] = t;
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
            if !c.keep() {
                return false;
            }
            match store.as_mut() {
                // Keep it in memory if the write fails rather than lose the player's work.
                Some(s) if c.unsaved => c.write(s, k).is_err(),
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
                match c.write(s, k) {
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

    #[test]
    fn iron_only_past_the_first_tier_and_more_further_out() {
        let ores = |r0: i32, r1: i32| {
            let mut n = (0, 0); // (iron, rocks + iron)
            for y in -r1..=r1 {
                for x in -r1..=r1 {
                    let d2 = x * x + y * y;
                    if d2 >= r0 * r0 && d2 < r1 * r1 {
                        match generate(x, y, 7) {
                            IRON_ORE => (n.0 += 1, n.1 += 1),
                            ROCK => ((), n.1 += 1),
                            _ => ((), ()),
                        };
                    }
                }
            }
            n
        };
        assert_eq!(ores(0, 40).0, 0, "no iron in tier 1");
        let (near, far) = (ores(45, 75), ores(165, 195)); // tiers 2 and 5
        assert!(near.0 > 0);
        assert!(far.0 * near.1 > 2 * near.0 * far.1, "iron share grows with distance: {near:?} vs {far:?}");
    }

    #[test]
    fn floors_persist_on_their_own_layer() {
        use crate::content::{BED, WOOD_FLOOR};
        let dir = std::env::temp_dir().join(format!("nm-ground-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // An open grass tile in a chunk that will hold nothing but a floor.
        let (fx, fy) = (32..48).flat_map(|y| (32..48).map(move |x| (x, y))).find(|&(x, y)| generate(x, y, 7) == GRASS).unwrap();
        let mut w = World::new(7, Some(Store::open(dir.clone()).unwrap()));
        w.set_ground(fx, fy, WOOD_FLOOR);
        w.set_ground(-3, 5, WOOD_FLOOR);
        w.set(-3, 5, BED); // a bed standing on a floor
        assert!(!w.blocks(fx, fy, true) && !w.blocks(fx, fy, false), "floors don't block");
        w.maintain(0, 0);
        w.maintain(5000, 5000);
        assert!(w.peek(fx, fy).is_none());
        let w2 = World::new(7, Some(Store::open(dir.clone()).unwrap()));
        assert_eq!(w2.ground(fx, fy), WOOD_FLOOR);
        assert_eq!(w2.get(fx + 1, fy), generate(fx + 1, fy, 7), "tiles still come from the seed");
        assert_eq!((w2.get(-3, 5), w2.ground(-3, 5)), (BED, WOOD_FLOOR));
        assert_eq!(w2.ground(-4, 5), GRASS);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
