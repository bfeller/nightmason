//! Game content tables. To add something new:
//!   - a resource: add a `Res` variant and a `RESOURCES` entry (same order)
//!   - a tile: add an id constant and a `TILES` entry at that index (max 255 kinds);
//!     give it a `turret` to make it shoot at zombies
//!   - a build-menu category: add a `Cat` variant and a `CATEGORIES` name (same order)
//!   - a tool: add a `TOOLS` entry (and a `Tool` kind if it's a new line of tools); set `tool` on
//!     the tiles it helps with
//!   - a build item: add a `RECIPES` entry pointing at its tile, in a category, with a one-line `desc` (<= 40
//!     chars, using only glyphs in gfx.rs). Art up to 2x2 blocks fits a build-menu cell
//! Sprites come from tools/pack_sprites.py (`make sprites`).
use crate::gfx::rgb;
use crate::sprites as spr;

#[derive(Clone, Copy, PartialEq)]
pub enum Res {
    Stone,
    Wood,
    Iron,
}

pub struct ResDef {
    pub name: &'static str,
    pub icon: usize,
}

pub const RESOURCES: &[ResDef] = &[
    ResDef { name: "STONE", icon: spr::ROCK0 },
    ResDef { name: "WOOD", icon: spr::WOOD },
    ResDef { name: "IRON", icon: spr::ORE0 },
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
pub const WOOD_FLOOR: Tile = 8;
pub const TORCH: Tile = 9;
pub const TOOLBENCH: Tile = 10;
pub const IRON_ORE: Tile = 11;

pub enum Look {
    /// Nothing drawn over the grass.
    Ground,
    /// Laid on the ground layer under everything else (floors): walkable, and you can build on
    /// top of it.
    Floor(&'static [usize]),
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
    /// Glow radius in art pixels at night (0 = none). Zombies never rise near a light.
    pub light: u8,
    /// The tool that breaks it faster (and, for natural things, yields more).
    pub tool: Option<Tool>,
    /// The tier of that tool you need to break it at all (0 = bare hands will do).
    pub min_tier: u8,
    /// Pressing B while facing it opens the toolbench.
    pub bench: bool,
}

/// Defaults for new entries: `TileDef { name: "X", hp: 5, ..PLAIN }`.
const PLAIN: TileDef = TileDef {
    name: "", look: Look::Ground, blocks_player: false, blocks_zombie: false,
    hp: 0, punches: 0, zombies_break: false, drops: &[], debris: 0, turret: None, spawn: false,
    light: 0, tool: None, min_tier: 0, bench: false,
};

pub const TILES: &[TileDef] = &[
    TileDef { name: "GRASS", ..PLAIN },
    TileDef {
        name: "ROCK", tool: Some(Tool::Pick), look: Look::Small(&[spr::ROCK0, spr::ROCK1], true),
        blocks_player: true, blocks_zombie: true,
        hp: 3, punches: 3, zombies_break: false, drops: &[(Res::Stone, 2)], debris: rgb(160, 160, 180), turret: None, ..PLAIN
    },
    TileDef {
        name: "WALL", tool: Some(Tool::Pick), look: Look::Small(&[spr::WALL], false),
        blocks_player: true, blocks_zombie: true,
        hp: 12, punches: 4, zombies_break: true, drops: &[(Res::Stone, 1)], debris: rgb(200, 200, 210), turret: None, ..PLAIN
    },
    TileDef {
        name: "TREE", tool: Some(Tool::Axe), look: Look::Big(&[spr::PINE, spr::OAK], 2, 2, true),
        blocks_player: true, blocks_zombie: true,
        hp: 4, punches: 4, zombies_break: false, drops: &[(Res::Wood, 3)], debris: rgb(110, 170, 70), turret: None, ..PLAIN
    },
    TileDef {
        name: "DOOR", tool: Some(Tool::Axe), look: Look::Small(&[spr::DOOR], false),
        blocks_player: false, blocks_zombie: true,
        hp: 8, punches: 3, zombies_break: true, drops: &[(Res::Wood, 1)], debris: rgb(190, 110, 60), turret: None, ..PLAIN
    },
    TileDef {
        name: "CATAPULT", tool: Some(Tool::Pick), look: Look::Big(&[spr::CATAPULT], 1, 2, false),
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
        name: "BED", tool: Some(Tool::Axe), look: Look::Small(&[spr::BED], false),
        blocks_player: false, blocks_zombie: true,
        hp: 6, punches: 2, zombies_break: true, drops: &[(Res::Wood, 2)], debris: rgb(77, 101, 180),
        spawn: true, ..PLAIN
    },
    TileDef {
        name: "BALLISTA", tool: Some(Tool::Pick), look: Look::Big(&[spr::BALLISTA], 1, 2, false),
        blocks_player: true, blocks_zombie: true,
        hp: 10, punches: 4, zombies_break: true, drops: &[(Res::Stone, 1), (Res::Wood, 2)],
        debris: rgb(190, 110, 60),
        turret: Some(Turret {
            arc: false, range: 72.0, reload: 40, ammo: (Res::Wood, 1), damage: 3, splash: 5.0, flight: 9,
            fired: spr::BALLISTA_FIRED,
        }),
        ..PLAIN
    },
    TileDef {
        name: "WOOD FLOOR", tool: Some(Tool::Axe), look: Look::Floor(&[spr::FLOOR]),
        hp: 2, punches: 2, drops: &[(Res::Wood, 1)], debris: rgb(205, 104, 61), ..PLAIN
    },
    TileDef {
        name: "TORCH", tool: Some(Tool::Axe), look: Look::Small(&[spr::TORCH], true),
        hp: 1, punches: 1, drops: &[(Res::Wood, 1)], debris: rgb(251, 185, 84), light: 28, ..PLAIN
    },
    TileDef {
        name: "TOOLBENCH", tool: Some(Tool::Axe), look: Look::Small(&[spr::TOOLBENCH], false),
        blocks_player: true, blocks_zombie: true, hp: 10, punches: 4, zombies_break: true,
        drops: &[(Res::Wood, 5), (Res::Stone, 2)], debris: rgb(205, 104, 61), bench: true, ..PLAIN
    },
    TileDef {
        name: "IRON ORE", tool: Some(Tool::Pick), look: Look::Small(&[spr::ORE0, spr::ORE1], true),
        blocks_player: true, blocks_zombie: true, hp: 6, punches: 6, min_tier: 1,
        drops: &[(Res::Iron, 1)], debris: rgb(205, 104, 61), ..PLAIN
    },
];

/// Lines of tools. You own at most one of each (your best), and it's used automatically on
/// whatever it helps with.
#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
    Pick,
    Axe,
    /// Doesn't break tiles: its `speed` is your punch damage against zombies.
    Sword,
}
pub const TOOL_KINDS: usize = 3;

pub struct ToolDef {
    pub kind: Tool,
    /// 1 = first tool of its line; higher tiers replace lower ones (for future upgrades).
    pub tier: u8,
    pub name: &'static str,
    pub icon: usize,
    /// Multiplies punch damage on tiles it helps with (for a sword: damage per hit on zombies).
    pub speed: u8,
    /// Extra of each drop from natural tiles (not from your own buildings).
    pub bonus: u32,
    pub cost: &'static [(Res, u32)],
    pub desc: &'static str,
}

/// Made at a toolbench. A tier above 1 upgrades the tier below it, which you need first.
pub const TOOLS: &[ToolDef] = &[
    ToolDef {
        kind: Tool::Pick, tier: 1, name: "STONE PICKAXE", icon: spr::PICKAXE, speed: 2, bonus: 1,
        cost: &[(Res::Wood, 50), (Res::Stone, 50)], desc: "MINES 2X FASTER, +1 DROP. MINES IRON",
    },
    ToolDef {
        kind: Tool::Axe, tier: 1, name: "STONE AXE", icon: spr::AXE, speed: 2, bonus: 1,
        cost: &[(Res::Wood, 50), (Res::Stone, 50)], desc: "CHOPS 2X FASTER. +1 WOOD FROM TREES",
    },
    ToolDef {
        kind: Tool::Sword, tier: 1, name: "STONE SWORD", icon: spr::SWORD, speed: 2, bonus: 0,
        cost: &[(Res::Wood, 50), (Res::Stone, 50)], desc: "HITS ZOMBIES FOR 2 INSTEAD OF 1",
    },
    ToolDef {
        kind: Tool::Pick, tier: 2, name: "IRON PICKAXE", icon: spr::IRON_PICKAXE, speed: 4, bonus: 2,
        cost: &[(Res::Iron, 40), (Res::Wood, 20), (Res::Stone, 20)],
        desc: "MINES 4X FASTER. +2 FROM ROCKS AND ORE",
    },
    ToolDef {
        kind: Tool::Axe, tier: 2, name: "IRON AXE", icon: spr::IRON_AXE, speed: 4, bonus: 2,
        cost: &[(Res::Iron, 40), (Res::Wood, 20), (Res::Stone, 20)],
        desc: "CHOPS 4X FASTER. +2 WOOD FROM TREES",
    },
    ToolDef {
        kind: Tool::Sword, tier: 2, name: "IRON SWORD", icon: spr::IRON_SWORD, speed: 3, bonus: 0,
        cost: &[(Res::Iron, 40), (Res::Wood, 20), (Res::Stone, 20)],
        desc: "HITS ZOMBIES FOR 3 INSTEAD OF 1",
    },
];

pub fn affordable(cost: &[(Res, u32)], inv: &[u32; RES_COUNT]) -> bool {
    cost.iter().all(|&(r, n)| inv[r as usize] >= n)
}

/// Something you can build (as opposed to something that grows in the world).
pub fn buildable(id: Tile) -> bool {
    RECIPES.iter().any(|r| r.tile == id)
}

impl TileDef {
    pub fn is_floor(&self) -> bool {
        matches!(self.look, Look::Floor(_))
    }
}

pub fn tile(id: Tile) -> &'static TileDef {
    TILES.get(id as usize).unwrap_or(&TILES[0])
}

/// Build-menu tabs.
#[derive(Clone, Copy, PartialEq)]
pub enum Cat {
    Defense,
    Towers,
    Home,
}

pub const CATEGORIES: &[&str] = &["DEFENSE", "TOWERS", "HOME"];

pub struct Recipe {
    pub cat: Cat,
    pub tile: Tile,
    pub cost: &'static [(Res, u32)],
    /// One line (up to 40 characters) shown in the build menu.
    pub desc: &'static str,
}

/// Build menu entries, in menu order (number keys 1-9 pick the first nine directly).
pub const RECIPES: &[Recipe] = &[
    Recipe { cat: Cat::Defense, tile: WALL, cost: &[(Res::Stone, 1)], desc: "STOPS ZOMBIES UNTIL THEY CLAW THROUGH" },
    Recipe { cat: Cat::Defense, tile: DOOR, cost: &[(Res::Wood, 2)], desc: "YOU WALK THROUGH. ZOMBIES DO NOT" },
    Recipe {
        cat: Cat::Towers, tile: CATAPULT, cost: &[(Res::Stone, 5), (Res::Wood, 3)],
        desc: "LOBS ROCKS AT ZOMBIES. 1 STONE A SHOT",
    },
    Recipe {
        cat: Cat::Towers, tile: BALLISTA, cost: &[(Res::Stone, 2), (Res::Wood, 5)],
        desc: "1 WOOD A SHOT. CANT SHOOT OVER WALLS",
    },
    Recipe { cat: Cat::Home, tile: BED, cost: &[(Res::Wood, 4)], desc: "YOU RESPAWN HERE. ZOMBIES CAN BREAK IT" },
    Recipe {
        cat: Cat::Home, tile: TORCH, cost: &[(Res::Wood, 1), (Res::Stone, 1)],
        desc: "LIGHTS THE NIGHT. NO ZOMBIES RISE NEAR",
    },
    Recipe {
        cat: Cat::Home, tile: TOOLBENCH, cost: &[(Res::Wood, 10), (Res::Stone, 5)],
        desc: "MAKE TOOLS HERE. FACE IT AND PRESS B",
    },
    Recipe { cat: Cat::Home, tile: WOOD_FLOOR, cost: &[(Res::Wood, 1)], desc: "LAY IT DOWN, THEN BUILD ON TOP OF IT" },
];

impl Recipe {
    pub fn name(&self) -> &'static str {
        tile(self.tile).name
    }
    pub fn icon(&self) -> usize {
        match tile(self.tile).look {
            Look::Small(v, _) | Look::Big(v, ..) | Look::Floor(v) => v[0],
            Look::Ground => spr::GRASS0,
        }
    }
    /// Full-size art: sprite id and size in 8x8 blocks.
    pub fn art(&self) -> (usize, i32, i32) {
        match tile(self.tile).look {
            Look::Small(v, _) | Look::Floor(v) => (v[0], 1, 1),
            Look::Big(v, w, h, _) => (v[0], w, h),
            Look::Ground => (spr::GRASS0, 1, 1),
        }
    }
    pub fn affordable(&self, inv: &[u32; RES_COUNT]) -> bool {
        affordable(self.cost, inv)
    }
}
