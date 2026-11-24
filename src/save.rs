//! On-disk persistence: edited chunks in region files plus a small `world.dat` for everything else.
//!
//! Each map layer has its own region files: `r.<rx>.<ry>` for tiles (walls, trees, beds...) and
//! `g.<rx>.<ry>` for the ground layer (floors). A region file holds 8x8 chunks at fixed offsets: a 64-byte header (one byte per
//! chunk, 1 = present) followed by 64 slots of 256 tile bytes. Unwritten slots stay sparse, so a
//! region with one edited chunk costs a few KB on disk. Loading or saving a chunk is one pread or
//! pwrite; nothing is parsed.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

pub const CHUNK_BYTES: usize = 256;
const REGION: i32 = 8; // chunks per side
const SLOTS: usize = (REGION * REGION) as usize;
const VERSION: u32 = 1;

/// Which map layer a chunk belongs to.
#[derive(Clone, Copy)]
pub enum Layer {
    Tiles = 0,
    Ground = 1,
}
const PREFIX: [&str; 2] = ["r", "g"];

pub struct Store {
    dir: PathBuf,
    /// The most recently used region file per layer (the player usually stays in one).
    open: [Option<((i32, i32), File)>; 2],
}

/// Default save directory: $NM_SAVE, else ~/.local/share/nightmason. NM_SAVE="" disables saving.
pub fn default_dir() -> Option<PathBuf> {
    match std::env::var("NM_SAVE") {
        Ok(p) if p.is_empty() => None,
        Ok(p) => Some(p.into()),
        Err(_) => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            Some(PathBuf::from(home).join(".local/share/nightmason"))
        }
    }
}

impl Store {
    pub fn open(dir: PathBuf) -> io::Result<Store> {
        fs::create_dir_all(&dir)?;
        Ok(Store { dir, open: [None, None] })
    }

    fn region(&mut self, layer: Layer, k: (i32, i32), create: bool) -> Option<&File> {
        let r = (k.0.div_euclid(REGION), k.1.div_euclid(REGION));
        let open = &mut self.open[layer as usize];
        if open.as_ref().map(|o| o.0) != Some(r) {
            let path = self.dir.join(format!("{}.{}.{}", PREFIX[layer as usize], r.0, r.1));
            let f = OpenOptions::new().read(true).write(true).create(create).open(path).ok()?;
            *open = Some((r, f));
        }
        open.as_ref().map(|o| &o.1)
    }

    fn slot(k: (i32, i32)) -> u64 {
        (k.1.rem_euclid(REGION) * REGION + k.0.rem_euclid(REGION)) as u64
    }

    /// Reads a saved chunk into `tiles`; false if it was never saved.
    pub fn load(&mut self, layer: Layer, k: (i32, i32), tiles: &mut [u8; CHUNK_BYTES]) -> bool {
        let s = Self::slot(k);
        let Some(f) = self.region(layer, k, false) else { return false };
        let mut present = [0u8];
        if f.read_exact_at(&mut present, s).is_err() || present[0] != 1 {
            return false;
        }
        f.read_exact_at(tiles, SLOTS as u64 + s * CHUNK_BYTES as u64).is_ok()
    }

    pub fn store(&mut self, layer: Layer, k: (i32, i32), tiles: &[u8; CHUNK_BYTES]) -> io::Result<()> {
        let s = Self::slot(k);
        let f = self.region(layer, k, true).ok_or_else(|| io::Error::other("cannot open region"))?;
        // Tiles first, then the presence flag, so a torn write never exposes a half chunk.
        f.write_all_at(tiles, SLOTS as u64 + s * CHUNK_BYTES as u64)?;
        f.write_all_at(&[1], s)
    }

    /// Everything that isn't a chunk, as `key value...` lines. Written atomically.
    pub fn write_meta(&self, lines: &str) -> io::Result<()> {
        let tmp = self.dir.join("world.dat.tmp");
        let mut f = File::create(&tmp)?;
        write!(f, "nightmason {VERSION}\n{lines}")?;
        f.sync_all()?;
        fs::rename(tmp, self.dir.join("world.dat"))
    }

    /// Parsed `world.dat`, or None for a fresh world.
    pub fn read_meta(&self) -> Option<Meta> {
        let text = fs::read_to_string(self.dir.join("world.dat")).ok()?;
        let mut lines = text.lines();
        if lines.next()? != format!("nightmason {VERSION}") {
            return None;
        }
        Some(Meta(lines.filter_map(|l| l.split_once(' ')).map(|(k, v)| (k.into(), v.into())).collect()))
    }

    /// Deletes this world's files (for starting a new world).
    pub fn wipe(&mut self) {
        self.open = [None, None];
        if let Ok(rd) = fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                let name = e.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("r.") || name.starts_with("g.") || name.starts_with("world.dat") {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
    }
}

pub struct Meta(Vec<(String, String)>);

impl Meta {
    /// Whitespace-separated values for `key` (empty if missing).
    pub fn get<T: std::str::FromStr>(&self, key: &str) -> Vec<T> {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.split_whitespace().filter_map(|s| s.parse().ok()).collect())
            .unwrap_or_default()
    }
    pub fn one<T: std::str::FromStr>(&self, key: &str) -> Option<T> {
        self.get(key).into_iter().next()
    }
}
