//! Game content tables. To add something new:
//!   - a resource: add a `Res` variant and a `RESOURCES` entry (same order)
//!   - a tile: add an id constant and a `TILES` entry at that index (max 255 kinds);
//!     give it a `turret` to make it shoot at zombies
//!   - a build item: add a `RECIPES` entry pointing at its tile, with a one-line `desc` (<= 40
//!     chars, using only glyphs in gfx.rs). Art up to 2x2 blocks fits a build-menu cell
//! Sprites come from tools/pack_sprites.py (`make sprites`).
use crate::gfx::rgb;
use crate::sprites as spr;

#[derive(Clone, Copy, PartialEq)]
pub enum Res {
    Stone,
    Wood,
}

pub struct ResDef {
    pub name: &'static str,
    pub icon: usize,
}

pub const RESOURCES: &[ResDef] = &[
    ResDef { name: "STONE", icon: spr::ROCK0 },
    ResDef { name: "WOOD", icon: spr::WOOD },
];
pub const RES_COUNT: usize = RESOURCES.len();

pub type Tile = u8;
pub const GRASS: Tile = 0;
pub const ROCK: Tile = 1;
pub const WALL: Tile = 2;
pub const TREE: Tile = 3;
pub const DOOR: Tile = 4;
pub const CATAPULT: Tile = 5;
pub const BED: Tile = 6; // ids are saved on disk: append new tiles, never renumber
pub const BALLISTA: Tile = 7;

pub enum Look {
    /// Nothing drawn over the grass.
    Ground,
    /// 8x8 sprite; the variant is picked per tile by hash. `bool` = may be mirrored.
    Small(&'static [usize], bool),
    /// Multi-block sprite (w x h blocks of 8x8) standing on the tile, centred horizontally and
    /// overhanging the rows above (trees, towers). `bool` = may be mirrored.
    Big(&'static [usize], i32, i32, bool),
}

/// A tile that shoots the nearest zombie, paid for from the player's inventory.
pub struct Turret {
    /// true: lobbed in an arc, clearing walls. false: flies flat, so it needs a clear line to
    /// the target and stops on anything zombies can't walk through.
    pub arc: bool,
    /// Range in art pixels (8 per tile), measured from the tile centre.
    pub range: f32,
    /// Ticks between shots (30 ticks = 1 s).
    pub reload: u16,
    pub ammo: (Res, u32),
    pub damage: i32,
    /// Everything within this many pixels of the impact is hit.
    pub splash: f32,
    /// Ticks in the air.
    pub flight: u16,
    /// Sprite shown just after firing or when out of ammo (same block size as the tile's look).
    pub fired: usize,
}

pub struct TileDef {
    pub name: &'static str,
    pub look: Look,
    pub blocks_player: bool,
    pub blocks_zombie: bool,
    /// Durability; 0 = can't be damaged.
    pub hp: u8,
    /// Punches the player needs to break it.
    pub punches: u8,
    /// Zombies claw through it when it's in their way.
    pub zombies_break: bool,
    /// What the player gets for breaking it.
    pub drops: &'static [(Res, u32)],
    /// Particle colour when hit.
    pub debris: u16,
    pub turret: Option<Turret>,
    /// Building one makes it your respawn point (while it stands).
    pub spawn: bool,
}

/// Defaults for new entries: `TileDef { name: "X", hp: 5, ..PLAIN }`.
const PLAIN: TileDef = TileDef {
    name: "", look: Look::Ground, blocks_player: false, blocks_zombie: false,
    hp: 0, punches: 0, zombies_break: false, drops: &[], debris: 0, turret: None, spawn: false,
};

pub const TILES: &[TileDef] = &[
    TileDef { name: "GRASS", ..PLAIN },
    TileDef {
        name: "ROCK", look: Look::Small(&[spr::ROCK0, spr::ROCK1], true),
        blocks_player: true, blocks_zombie: true,
        hp: 3, punches: 3, zombies_break: false, drops: &[(Res::Stone, 2)], debris: rgb(160, 160, 180), turret: None, ..PLAIN
    },
    TileDef {
        name: "WALL", look: Look::Small(&[spr::WALL], false),
        blocks_player: true, blocks_zombie: true,
        hp: 12, punches: 4, zombies_break: true, drops: &[(Res::Stone, 1)], debris: rgb(200, 200, 210), turret: None, ..PLAIN
    },
    TileDef {
        name: "TREE", look: Look::Big(&[spr::PINE, spr::OAK], 2, 2, true),
        blocks_player: true, blocks_zombie: true,
        hp: 4, punches: 4, zombies_break: false, drops: &[(Res::Wood, 3)], debris: rgb(110, 170, 70), turret: None, ..PLAIN
    },
    TileDef {
        name: "DOOR", look: Look::Small(&[spr::DOOR], false),
        blocks_player: false, blocks_zombie: true,
        hp: 8, punches: 3, zombies_break: true, drops: &[(Res::Wood, 1)], debris: rgb(190, 110, 60), turret: None, ..PLAIN
    },
    TileDef {
        name: "CATAPULT", look: Look::Big(&[spr::CATAPULT], 1, 2, false),
        blocks_player: true, blocks_zombie: true,
        hp: 10, punches: 4, zombies_break: true, drops: &[(Res::Stone, 2), (Res::Wood, 1)],
        debris: rgb(190, 110, 60),
        turret: Some(Turret {
            arc: true, range: 56.0, reload: 50, ammo: (Res::Stone, 1), damage: 2, splash: 9.0, flight: 24,
            fired: spr::CATAPULT_FIRED,
        }),
        ..PLAIN
    },
    TileDef {
        name: "BED", look: Look::Small(&[spr::BED], false),
        blocks_player: false, blocks_zombie: true,
        hp: 6, punches: 2, zombies_break: true, drops: &[(Res::Wood, 2)], debris: rgb(77, 101, 180),
        spawn: true, ..PLAIN
    },
    TileDef {
        name: "BALLISTA", look: Look::Big(&[spr::BALLISTA], 1, 2, false),
        blocks_player: true, blocks_zombie: true,
        hp: 10, punches: 4, zombies_break: true, drops: &[(Res::Stone, 1), (Res::Wood, 2)],
        debris: rgb(190, 110, 60),
        turret: Some(Turret {
            arc: false, range: 72.0, reload: 40, ammo: (Res::Wood, 1), damage: 3, splash: 5.0, flight: 9,
            fired: spr::BALLISTA_FIRED,
        }),
        ..PLAIN
    },
];

pub fn tile(id: Tile) -> &'static TileDef {
    TILES.get(id as usize).unwrap_or(&TILES[0])
}

pub struct Recipe {
    pub tile: Tile,
    pub cost: &'static [(Res, u32)],
    /// One line (up to 40 characters) shown in the build menu.
    pub desc: &'static str,
}

/// Build menu entries, in menu order (number keys 1-9 pick the first nine directly).
pub const RECIPES: &[Recipe] = &[
    Recipe { tile: WALL, cost: &[(Res::Stone, 1)], desc: "STOPS ZOMBIES UNTIL THEY CLAW THROUGH" },
    Recipe { tile: DOOR, cost: &[(Res::Wood, 2)], desc: "YOU WALK THROUGH. ZOMBIES DO NOT" },
    Recipe {
        tile: CATAPULT, cost: &[(Res::Stone, 5), (Res::Wood, 3)],
        desc: "LOBS ROCKS AT ZOMBIES. 1 STONE A SHOT",
    },
    Recipe {
        tile: BALLISTA, cost: &[(Res::Stone, 2), (Res::Wood, 5)],
        desc: "1 WOOD A SHOT. CANT SHOOT OVER WALLS",
    },
    Recipe { tile: BED, cost: &[(Res::Wood, 4)], desc: "YOU RESPAWN HERE. ZOMBIES CAN BREAK IT" },
];

impl Recipe {
    pub fn name(&self) -> &'static str {
        tile(self.tile).name
    }
    pub fn icon(&self) -> usize {
        match tile(self.tile).look {
            Look::Small(v, _) | Look::Big(v, ..) => v[0],
            Look::Ground => spr::GRASS0,
        }
    }
    /// Full-size art: sprite id and size in 8x8 blocks.
    pub fn art(&self) -> (usize, i32, i32) {
        match tile(self.tile).look {
            Look::Small(v, _) => (v[0], 1, 1),
            Look::Big(v, w, h, _) => (v[0], w, h),
            Look::Ground => (spr::GRASS0, 1, 1),
        }
    }
    pub fn affordable(&self, inv: &[u32; RES_COUNT]) -> bool {
        self.cost.iter().all(|&(r, n)| inv[r as usize] >= n)
    }
}
