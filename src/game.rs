//! Game rules: player, punching, building, day/night and zombies.
//! What exists (tiles, resources, build items) lives in content.rs.
use crate::content::{
    affordable, buildable, tile, Look, Res, Tile, TileDef, Tool, ToolDef, CATEGORIES, GRASS, RECIPES, RESOURCES, RES_COUNT,
    TOOLS, TOOL_KINDS,
};
use crate::gfx::{rgb, Canvas, H, W};
use crate::input::Input;
use crate::save::Store;
use crate::sprites as spr;
use crate::world::{hash, tier_at, World};

// Linux key codes. The Cardputer's ; , . / keys double as arrows.
const K_ESC: u16 = 1;
const UP: &[u16] = &[17, 103, 39]; // W, Up, ;
const DOWN: &[u16] = &[31, 108, 52]; // S, Down, .
const LEFT: &[u16] = &[30, 105, 51]; // A, Left, ,
const RIGHT: &[u16] = &[32, 106, 53]; // D, Right, /
const PUNCH: &[u16] = &[57, 36, 28]; // Space, J, Enter
const BUILD: &[u16] = &[48, 37, 18]; // B, K, E
const MENU: &[u16] = &[15, 16]; // Tab, Q
const PAUSE: &[u16] = &[25, 1]; // P, Esc
const RESTART: &[u16] = &[28, 57]; // Enter, Space
const NUM1: u16 = 2; // KEY_1..KEY_9 = 2..10
// Inside the build menu Q/E switch category tabs, so only these close or confirm it.
const TAB_PREV: &[u16] = &[16]; // Q
const TAB_NEXT: &[u16] = &[18]; // E
const MENU_CLOSE: &[u16] = &[15, 1]; // Tab, Esc
const MENU_OK: &[u16] = &[57, 36, 28, 48, 37]; // Space, J, Enter, B, K

// One day is 190 s at 30 ticks/s.
pub const DAY_LEN: u64 = 5700;
const DUSK: u64 = 2700; // light starts fading
const NIGHT: u64 = 3150; // fully dark
const DAWN: u64 = 5400; // light returns
const SPAWN_START: u64 = 2925; // zombies rise halfway through dusk
const NIGHT_AMBIENT: i32 = 56;

const MAX_HP: i32 = 10;
const PLAYER_SPEED: f32 = 0.9;
const HALF: f32 = 3.0; // entity collision box half-size (art pixels)
/// Build menu grid: cells of MENU_CELL px, MENU_COLS x MENU_ROWS visible, scrolling by row.
const MENU_COLS: usize = 8;
const MENU_ROWS: usize = 3;
const MENU_CELL: i32 = 19;
/// Autosave interval (the game also saves on quit).
const SAVE_EVERY: u64 = 600;

/// Zombies path-find on a flow field this many tiles around the player.
const FLOW_R: i32 = 24;
const FLOW_N: usize = (2 * FLOW_R + 1) as usize;
/// Path cost of a tile zombies must break through (vs 1 for open ground).
const BREAK_COST: u16 = 10;

/// Zombies never rise within this many art pixels (8 per tile) of a light such as a torch.
const LIGHT_SAFE: f32 = 64.0;

const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (-1, 0), (1, 0)]; // down, up, left, right

struct Zombie {
    x: f32,
    y: f32,
    hp: i32,
    flash: u8,
    cooldown: u16,
    stuck: u16,
    wander: (f32, f32, u16),
    left: bool,
}

/// Runtime state of a turret tile (the spec lives in content.rs).
struct TurretState {
    tx: i32,
    ty: i32,
    cooldown: u16,
    fired: u16,
}

/// A shot travelling along the ground from (x0, y0) to (x1, y1). Lobbed shots (`arc`) rise and
/// fall, passing over walls; flat ones fly at tower height and stop on anything that blocks
/// zombies.
struct Projectile {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    t: u16,
    dur: u16,
    damage: i32,
    splash: f32,
    arc: bool,
    /// The tile it was fired from (a flat shot doesn't hit its own tower).
    from: (i32, i32),
}

impl Projectile {
    /// Ground position and height above it.
    fn pos(&self) -> (f32, f32, f32) {
        let f = self.t as f32 / self.dur as f32;
        let x = self.x0 + (self.x1 - self.x0) * f;
        let y = self.y0 + (self.y1 - self.y0) * f;
        let z = if self.arc { (1.0 - f) * 10.0 + 72.0 * f * (1.0 - f) } else { 8.0 };
        (x, y, z)
    }
}

/// First point on the segment (sampled every 2 px) inside a tile that blocks zombies, ignoring
/// the `skip` tile. None means the line is clear.
fn first_blocked(w: &World, (x0, y0): (f32, f32), (x1, y1): (f32, f32), skip: (i32, i32)) -> Option<(f32, f32)> {
    let steps = (((x1 - x0).abs().max((y1 - y0).abs())) / 2.0).ceil().max(1.0) as i32;
    (1..=steps).map(|i| i as f32 / steps as f32).map(|f| (x0 + (x1 - x0) * f, y0 + (y1 - y0) * f)).find(|&(x, y)| {
        let t = (tile_of(x), tile_of(y));
        t != skip && w.blocks(t.0, t.1, true)
    })
}

/// The start menu, which is also the pause menu.
/// What pressing B at the target tile would do.
enum Plan {
    /// Mend the damaged building there (`id` is the thing being mended) for `cost`.
    Repair { id: Tile, cost: Vec<(Res, u32)> },
    /// Build the selected item.
    Build,
    /// The selected item doesn't fit there.
    Nothing,
}

struct PauseMenu {
    /// Shown at launch (title screen) rather than mid-game.
    title: bool,
    sel: usize,
    /// "New world" was picked once; picking it again deletes this world.
    confirm: bool,
}

const PAUSE_ITEMS: usize = 3; // play/continue/resume, new world, quit

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: u8,
    c: u16,
}

pub struct Game {
    pub tick: u64,
    world: World,
    rng: u32,
    x: f32,
    y: f32,
    face: usize,
    walking: bool,
    turn: u8,
    hp: i32,
    pub inv: [u32; RES_COUNT],
    selected: usize,
    menu: bool,
    hurt: u8,
    punch: u8,
    regen: u16,
    zombies: Vec<Zombie>,
    particles: Vec<Particle>,
    turrets: Vec<TurretState>,
    projectiles: Vec<Projectile>,
    ammo_warn: u16,
    /// Path distance to the player per tile in a window around them (u16::MAX = unreachable).
    flow: Vec<u16>,
    flow_origin: (i32, i32),
    /// Last targeted tile and when it became the target (range rings show briefly).
    aim: ((i32, i32), u64),
    spawn_night: u64,
    spawned: u32,
    nights: u32,
    kills: u32,
    banner: (String, u16),
    note: (String, u16),
    dead: Option<u16>,
    pause: Option<PauseMenu>,
    esc: u16,
    /// Respawn point set by building a bed (a tile with `spawn`); None = the world spawn.
    bed: Option<(i32, i32)>,
    /// Light-giving tiles (torches) in resident chunks.
    lights: Vec<(i32, i32)>,
    /// Best tier owned of each tool line (0 = none).
    tools: [u8; TOOL_KINDS],
    /// Toolbench screen, when open: the selected TOOLS entry.
    bench: Option<usize>,
}

fn tile_of(v: f32) -> i32 {
    (v / 8.0).floor() as i32
}

fn box_free(w: &World, x: f32, y: f32, zombie: bool) -> bool {
    let (l, r) = (tile_of(x - HALF), tile_of(x + HALF - 0.01));
    let (t, b) = (tile_of(y - HALF), tile_of(y + HALF - 0.01));
    !(w.blocks(l, t, zombie) || w.blocks(r, t, zombie) || w.blocks(l, b, zombie) || w.blocks(r, b, zombie))
}

/// Axis-separated move; returns which axes were blocked.
fn slide(w: &World, x: &mut f32, y: &mut f32, dx: f32, dy: f32, zombie: bool) -> (bool, bool) {
    let bx = dx != 0.0 && !box_free(w, *x + dx, *y, zombie);
    if dx != 0.0 && !bx {
        *x += dx;
    }
    let by = dy != 0.0 && !box_free(w, *x, *y + dy, zombie);
    if dy != 0.0 && !by {
        *y += dy;
    }
    (bx, by)
}

fn overlaps_tile(x: f32, y: f32, tx: i32, ty: i32) -> bool {
    let (l, t) = (tx as f32 * 8.0, ty as f32 * 8.0);
    x + HALF > l && x - HALF < l + 8.0 && y + HALF > t && y - HALF < t + 8.0
}

/// Danger tier at a point in art pixels: 1 around spawn, rising with distance (world::TIER_TILES).
/// Difficulty depends on where you are, not how long you've played, so home stays safe.
fn tier(x: f32, y: f32) -> u64 {
    tier_at(x / 8.0, y / 8.0)
}

/// Zombies are faster in more dangerous ground.
fn zombie_speed(x: f32, y: f32) -> f32 {
    0.30 + 0.02 * tier(x, y).min(10) as f32
}

fn tile_center(tx: i32, ty: i32) -> (f32, f32) {
    (tx as f32 * 8.0 + 4.0, ty as f32 * 8.0 + 4.0)
}

impl Game {
    /// A fresh world.
    pub fn new(seed: u32, store: Option<Store>) -> Game {
        let seed = seed.max(1);
        let mut g = Game {
            tick: 0,
            world: World::new(seed, store),
            rng: seed,
            x: 4.0,
            y: 4.0,
            face: 0,
            walking: false,
            turn: 0,
            hp: MAX_HP,
            inv: [0; RES_COUNT],
            selected: 0,
            menu: false,
            hurt: 0,
            punch: 0,
            regen: 0,
            zombies: Vec::new(),
            particles: Vec::new(),
            turrets: Vec::new(),
            projectiles: Vec::new(),
            ammo_warn: 0,
            flow: vec![u16::MAX; FLOW_N * FLOW_N],
            flow_origin: (0, 0),
            aim: ((0, 0), 0),
            spawn_night: 0,
            spawned: 0,
            nights: 0,
            kills: 0,
            banner: ("DAY 1".into(), 90),
            note: (String::new(), 0),
            dead: None,
            pause: None,
            esc: 0,
            bed: None,
            lights: Vec::new(),
            tools: [0; TOOL_KINDS],
            bench: None,
        };
        g.maintain();
        g
    }

    /// Continue the world saved in `store`, or start a new one with `seed`.
    pub fn open(store: Option<Store>, seed: u32) -> Game {
        let Some(m) = store.as_ref().and_then(|s| s.read_meta()) else {
            let mut g = Game::new(seed, store);
            g.save();
            g.pause = Some(PauseMenu { title: true, sel: 0, confirm: false });
            return g;
        };
        let mut g = Game::new(m.one("seed").unwrap_or(seed), store);
        g.tick = m.one("tick").unwrap_or(0);
        if let [x, y, hp, face, ..] = m.get::<f32>("player")[..] {
            (g.x, g.y, g.hp, g.face) = (x, y, (hp as i32).min(MAX_HP), (face as usize).min(3));
        }
        if g.hp <= 0 {
            g.dead = Some(45); // quit on the death screen: back to it
        }
        for (slot, n) in g.inv.iter_mut().zip(m.get("inv")) {
            *slot = n;
        }
        g.selected = m.one::<usize>("selected").unwrap_or(0).min(RECIPES.len() - 1);
        if let [n, k, ..] = m.get("stats")[..] {
            (g.nights, g.kills) = (n, k);
        }
        if let [night, n, ..] = m.get("night")[..] {
            (g.spawn_night, g.spawned) = (night, n as u32);
        }
        for (slot, t) in g.tools.iter_mut().zip(m.get("tools")) {
            *slot = t;
        }
        if let [x, y, ..] = m.get("bed")[..] {
            g.bed = Some((x, y));
        }
        for z in m.get::<f32>("zombies").chunks_exact(3) {
            g.zombies.push(Zombie {
                x: z[0], y: z[1], hp: z[2] as i32, flash: 0, cooldown: 0, stuck: 0, wander: (0.0, 0.0, 0), left: false,
            });
        }
        let night = (SPAWN_START..DAWN).contains(&g.phase());
        g.banner = (format!("{} {}", if night { "NIGHT" } else { "DAY" }, g.day()), 90);
        g.maintain();
        g.pause = Some(PauseMenu { title: true, sel: 0, confirm: false });
        g
    }

    /// Write changed chunks and the player/world state. False if there's no save or it failed.
    pub fn save(&mut self) -> bool {
        let chunks_ok = self.world.flush();
        let inv: Vec<String> = self.inv.iter().map(|n| n.to_string()).collect();
        let zombies: Vec<String> = self.zombies.iter().map(|z| format!("{:.1} {:.1} {}", z.x, z.y, z.hp)).collect();
        let meta = format!(
            "seed {}\ntick {}\nplayer {:.2} {:.2} {} {}\ninv {}\nselected {}\nstats {} {}\nnight {} {}\nzombies {}\n",
            self.world.seed, self.tick, self.x, self.y, self.hp, self.face, inv.join(" "), self.selected,
            self.nights, self.kills, self.spawn_night, self.spawned, zombies.join(" "),
        );
        let tools: Vec<String> = self.tools.iter().map(|t| t.to_string()).collect();
        let meta = format!("{meta}tools {}\n", tools.join(" "));
        let meta = match self.bed {
            Some((x, y)) => format!("{meta}bed {x} {y}\n"),
            None => meta,
        };
        let meta_ok = self.world.store().as_ref().is_some_and(|s| s.write_meta(&meta).is_ok());
        chunks_ok && meta_ok
    }

    /// Page chunks in and out around the player and pick up turrets and lights in newly loaded
    /// ones.
    fn maintain(&mut self) {
        self.world.maintain(tile_of(self.x), tile_of(self.y));
        self.lights = self.world.find(|t| tile(t).light > 0);
        for (tx, ty) in self.world.find(|t| tile(t).turret.is_some()) {
            if !self.turrets.iter().any(|s| s.tx == tx && s.ty == ty) {
                self.turrets.push(TurretState { tx, ty, cooldown: 30, fired: 0 });
            }
        }
    }

    /// Back at your bed (or the world spawn) with full health; the world and your stuff stay.
    fn respawn(&mut self) {
        let bed = self.bed.filter(|&(x, y)| tile(self.world.get(x, y)).spawn);
        let (ox, oy) = bed.unwrap_or((0, 0));
        let (tx, ty) = (0..12)
            .flat_map(|r: i32| (-r..=r).flat_map(move |y| (-r..=r).map(move |x| (ox + x, oy + y))))
            .find(|&(x, y)| !self.world.blocks(x, y, false))
            .unwrap_or((ox, oy));
        (self.x, self.y) = tile_center(tx, ty);
        self.hp = MAX_HP;
        self.hurt = 60;
        self.dead = None;
        self.zombies.clear();
        self.projectiles.clear();
        (self.nights, self.kills) = (0, 0);
        self.banner = (if bed.is_some() { "RESPAWNED AT BED" } else { "RESPAWNED" }.into(), 60);
        if self.bed.is_some() && bed.is_none() {
            self.bed = None;
            self.say("YOUR BED IS GONE".into(), 90);
        }
        self.maintain();
        self.save();
    }

    /// Delete the save and start over with a new seed.
    fn new_world(&mut self) {
        let mut store = self.world.take_store();
        if let Some(s) = &mut store {
            s.wipe();
        }
        let seed = self.rand();
        *self = Game::new(seed, store);
        self.save();
    }

    fn rand(&mut self) -> u32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng
    }
    fn randf(&mut self) -> f32 {
        (self.rand() & 0xFFFF) as f32 / 65535.0
    }

    fn day(&self) -> u64 {
        self.tick / DAY_LEN + 1
    }
    fn phase(&self) -> u64 {
        self.tick % DAY_LEN
    }
    fn ambient(&self) -> i32 {
        let p = self.phase() as i32;
        let lerp = |a: i32, b: i32, t: i32, len: i32| a + (b - a) * t / len;
        match self.phase() {
            p2 if p2 < DUSK => 256,
            p2 if p2 < NIGHT => lerp(256, NIGHT_AMBIENT, p - DUSK as i32, (NIGHT - DUSK) as i32),
            p2 if p2 < DAWN => NIGHT_AMBIENT,
            _ => lerp(NIGHT_AMBIENT, 256, p - DAWN as i32, (DAY_LEN - DAWN) as i32),
        }
    }

    fn target(&self) -> (i32, i32) {
        let (dx, dy) = DIRS[self.face];
        (tile_of(self.x) + dx, tile_of(self.y) + dy)
    }

    fn say(&mut self, msg: String, ticks: u16) {
        self.note = (msg, ticks);
    }

    fn burst(&mut self, x: f32, y: f32, n: usize, c: u16) {
        for _ in 0..n {
            let (vx, vy) = (self.randf() * 2.0 - 1.0, self.randf() * -1.5 - 0.2);
            let life = 8 + (self.rand() % 10) as u8;
            self.particles.push(Particle { x, y, vx, vy, life, c });
        }
    }

    /// Damage a tile; the player collects its drops when it breaks.
    /// Damage what stands on a tile or, if nothing does and the player is punching, the floor.
    fn damage_tile(&mut self, tx: i32, ty: i32, by_player: bool) {
        let on_top = self.world.get(tx, ty);
        let floor = on_top == GRASS && by_player && self.world.ground(tx, ty) != GRASS;
        let id = if floor { self.world.ground(tx, ty) } else { on_top };
        let def = tile(id);
        if def.hp == 0 {
            return;
        }
        // Some things (ore) need at least a certain tool to break at all.
        if let (true, Some(kind)) = (by_player && def.min_tier > 0, def.tool) {
            if self.tools[kind as usize] < def.min_tier {
                let need = TOOLS.iter().find(|t| t.kind == kind && t.tier == def.min_tier).map_or("", |t| t.name);
                self.say(format!("{} NEEDS A {}", def.name, need), 60);
                let (cx, cy) = tile_center(tx, ty);
                self.burst(cx, cy, 2, rgb(170, 170, 190));
                return;
            }
        }
        let tool = if by_player { self.tool_for(def) } else { None };
        let speed = tool.map_or(1, |t| t.speed);
        let amount = if by_player { def.hp.div_ceil(def.punches.max(1)).saturating_mul(speed) } else { 1 };
        let (cx, cy) = tile_center(tx, ty);
        if self.world.hit(tx, ty, amount) >= def.hp {
            if floor {
                self.world.set_ground(tx, ty, GRASS);
            } else {
                self.world.set(tx, ty, GRASS);
            }
            if def.light > 0 {
                self.lights.retain(|&l| l != (tx, ty));
            }
            if self.bed == Some((tx, ty)) {
                self.bed = None;
                self.say(if by_player { "SPAWN POINT RESET" } else { "ZOMBIES BROKE YOUR BED!" }.into(), 90);
            }
            if by_player {
                // Tools only add to what grows in the world, never to refunds from your buildings.
                let bonus = tool.filter(|_| !buildable(id)).map_or(0, |t| t.bonus);
                for &(r, n) in def.drops {
                    self.inv[r as usize] += n + bonus;
                }
            }
            self.burst(cx, cy, 12, def.debris);
        } else {
            self.burst(cx, cy, if by_player { 4 } else { 2 }, def.debris);
        }
    }

    /// Advance one tick. Returns false when the player quits.
    pub fn update(&mut self, inp: &Input) -> bool {
        if inp.held(&[K_ESC]) {
            self.esc += 1;
            if self.esc >= 20 {
                return false;
            }
        } else {
            self.esc = 0;
        }
        if self.pause.is_some() {
            return self.update_pause(inp);
        }
        if let Some(t) = &mut self.dead {
            *t = t.saturating_add(1);
            if *t > 45 && inp.pressed(RESTART) {
                self.respawn();
            }
            self.update_particles();
            return true;
        }
        if self.bench.is_some() {
            self.update_bench(inp);
            self.note.1 = self.note.1.saturating_sub(1);
            return true;
        }
        for i in 0..RECIPES.len().min(9) {
            if inp.pressed(&[NUM1 + i as u16]) {
                self.selected = i;
                self.menu = false;
                self.aim.1 = self.tick;
                self.say(format!("{} SELECTED", RECIPES[i].name()), 45);
            }
        }
        if self.menu {
            self.update_menu(inp);
            return true;
        }
        if inp.pressed(MENU) {
            self.menu = true;
            return true;
        }
        if inp.pressed(PAUSE) {
            self.pause = Some(PauseMenu { title: false, sel: 0, confirm: false });
            self.save();
            return true;
        }
        self.tick += 1;
        let phase = self.phase();
        if phase == 0 {
            self.banner = (format!("DAY {}", self.day()), 90);
        } else if phase == SPAWN_START {
            self.banner = (format!("NIGHT {}", self.day()), 90);
        } else if phase == DAWN {
            self.nights += 1;
        }
        self.banner.1 = self.banner.1.saturating_sub(1);
        self.note.1 = self.note.1.saturating_sub(1);

        self.update_player(inp);
        self.spawn_zombies();
        self.update_zombies();
        self.update_turrets();
        self.update_projectiles();
        self.update_particles();
        if self.tick % 30 == 0 {
            self.maintain();
        }
        if self.tick % SAVE_EVERY == 0 && !self.save() && self.world.store().is_some() {
            self.say("SAVE FAILED".into(), 90);
        }
        if self.hp <= 0 {
            self.dead = Some(0);
            self.burst(self.x, self.y, 14, rgb(200, 30, 40));
        }
        true
    }

    /// Start/pause menu. Returns false on Quit.
    fn update_pause(&mut self, inp: &Input) -> bool {
        let Some(m) = &mut self.pause else { return true };
        if inp.pressed(UP) {
            (m.sel, m.confirm) = ((m.sel + PAUSE_ITEMS - 1) % PAUSE_ITEMS, false);
        }
        if inp.pressed(DOWN) {
            (m.sel, m.confirm) = ((m.sel + 1) % PAUSE_ITEMS, false);
        }
        if !m.title && inp.pressed(PAUSE) {
            self.pause = None;
            return true;
        }
        if !(inp.pressed(PUNCH) || inp.pressed(BUILD)) {
            return true;
        }
        match m.sel {
            0 => self.pause = None,
            1 if !m.confirm => m.confirm = true,
            1 => self.new_world(), // leaves the menu closed
            _ => return false,
        }
        true
    }

    /// Indices into RECIPES in the selected item's category, in menu order.
    fn menu_items(&self) -> Vec<usize> {
        let cat = RECIPES[self.selected].cat;
        (0..RECIPES.len()).filter(|&i| RECIPES[i].cat == cat).collect()
    }

    /// The build menu pauses the game while it's open. Q/E switch category tabs; arrows move
    /// around that category's grid.
    fn update_menu(&mut self, inp: &Input) {
        let tabs = CATEGORIES.len();
        let step = (inp.pressed(TAB_NEXT) as usize) + (inp.pressed(TAB_PREV) as usize) * (tabs - 1);
        if step % tabs != 0 {
            // Next category that has something in it; select its first item.
            let mut c = RECIPES[self.selected].cat as usize;
            for _ in 0..tabs {
                c = (c + step) % tabs;
                if let Some(i) = RECIPES.iter().position(|r| r.cat as usize == c) {
                    self.selected = i;
                    break;
                }
            }
        }
        let items = self.menu_items();
        let n = items.len();
        let s = items.iter().position(|&i| i == self.selected).unwrap_or(0);
        let mut p = s;
        if inp.pressed(LEFT) {
            p = (s + n - 1) % n;
        }
        if inp.pressed(RIGHT) {
            p = (s + 1) % n;
        }
        if inp.pressed(UP) && s >= MENU_COLS {
            p = s - MENU_COLS;
        }
        if inp.pressed(DOWN) && s / MENU_COLS < (n - 1) / MENU_COLS {
            p = (s + MENU_COLS).min(n - 1);
        }
        self.selected = items[p];
        if inp.pressed(MENU_OK) || inp.pressed(MENU_CLOSE) {
            self.menu = false;
            self.aim.1 = self.tick;
        }
    }

    fn update_player(&mut self, inp: &Input) {
        let mut dx = 0.0;
        let mut dy = 0.0;
        if inp.held(LEFT) {
            dx -= 1.0;
        }
        if inp.held(RIGHT) {
            dx += 1.0;
        }
        if inp.held(UP) {
            dy -= 1.0;
        }
        if inp.held(DOWN) {
            dy += 1.0;
        }
        let was_walking = self.walking;
        self.walking = false;
        if dx != 0.0 || dy != 0.0 {
            let want = if dx < 0.0 { 2 } else if dx > 0.0 { 3 } else if dy < 0.0 { 1 } else { 0 };
            // From a standstill, a quick tap only turns; hold to walk.
            if !was_walking && self.turn == 0 && want != self.face {
                self.turn = 5;
            }
            self.face = want;
        } else {
            self.turn = 0;
        }
        if self.turn > 0 {
            self.turn -= 1;
        } else if dx != 0.0 || dy != 0.0 {
            self.walking = true;
            let s = if dx != 0.0 && dy != 0.0 { PLAYER_SPEED * 0.707 } else { PLAYER_SPEED };
            let (mut x, mut y) = (self.x, self.y);
            let (bx, by) = slide(&self.world, &mut x, &mut y, dx * s, dy * s, false);
            // Corner assist: clipped by a neighbouring tile while walking straight? Slide into
            // line with our own row/column so we either pass it or face it squarely.
            let w = &self.world;
            if by && dx == 0.0 {
                let ax = tile_of(x) as f32 * 8.0 + 4.0;
                if box_free(w, ax, y + dy * s, false) {
                    x += (ax - x).clamp(-s, s);
                }
            } else if bx && dy == 0.0 {
                let ay = tile_of(y) as f32 * 8.0 + 4.0;
                if box_free(w, x + dx * s, ay, false) {
                    y += (ay - y).clamp(-s, s);
                }
            }
            (self.x, self.y) = (x, y);
        }
        self.hurt = self.hurt.saturating_sub(1);
        self.punch = self.punch.saturating_sub(1);

        let aim = self.target();
        if aim != self.aim.0 || inp.pressed(MENU) {
            self.aim = (aim, self.tick);
        }
        if inp.pressed(PUNCH) && self.punch == 0 {
            self.punch = 8;
            self.do_punch();
        }
        if inp.pressed(BUILD) {
            self.do_build();
        }
        if self.ambient() >= 200 && self.hp < MAX_HP {
            self.regen += 1;
            if self.regen >= 240 {
                self.regen = 0;
                self.hp += 1;
            }
        }
    }

    fn do_punch(&mut self) {
        let (dx, dy) = DIRS[self.face];
        let (fx, fy) = (self.x + dx as f32 * 7.0, self.y + dy as f32 * 7.0);
        let hit = self
            .zombies
            .iter()
            .enumerate()
            .filter(|(_, z)| (z.x - fx).abs() < 6.0 && (z.y - fy).abs() < 7.0)
            .min_by(|a, b| {
                let d = |z: &Zombie| (z.x - fx).abs() + (z.y - fy).abs();
                d(a.1).total_cmp(&d(b.1))
            })
            .map(|(i, _)| i);
        if let Some(i) = hit {
            let damage = self.best(Tool::Sword).map_or(1, |s| s.speed as i32);
            let z = &mut self.zombies[i];
            z.hp -= damage;
            z.flash = 6;
            if z.hp <= 0 {
                self.kills += 1;
            }
            let (mut zx, mut zy) = (z.x, z.y);
            slide(&self.world, &mut zx, &mut zy, dx as f32 * 5.0, dy as f32 * 5.0, true);
            (z.x, z.y) = (zx, zy);
            self.burst(fx, fy, 4, rgb(150, 230, 80));
            return;
        }
        let (tx, ty) = self.target();
        self.damage_tile(tx, ty, true);
    }

    /// What B would do at the target: repair a damaged building (or the floor, if nothing stands
    /// on it), else build the selected item if it fits there.
    fn plan(&self) -> Plan {
        let (tx, ty) = self.target();
        let on_top = self.world.get(tx, ty);
        let ground = self.world.ground(tx, ty);
        // Damage belongs to what stands on the tile, or to the floor when nothing does.
        let hurt = if on_top != GRASS { on_top } else { ground };
        let d = self.world.damage(tx, ty) as u32;
        if let (true, Some(r)) = (d > 0, RECIPES.iter().find(|r| r.tile == hurt && hurt != GRASS)) {
            let hp = tile(hurt).hp.max(1) as u32;
            let cost = r.cost.iter().map(|&(res, n)| (res, (n * d).div_ceil(hp))).collect();
            return Plan::Repair { id: hurt, cost };
        }
        // Floors go on bare ground, under nothing or under something you built (a torch, a wall);
        // everything else goes on an empty tile, floored or not.
        let placing = RECIPES[self.selected].tile;
        let fits = if tile(placing).is_floor() {
            ground == GRASS && (on_top == GRASS || buildable(on_top))
        } else {
            on_top == GRASS
        };
        if fits { Plan::Build } else { Plan::Nothing }
    }

    fn needs(what: &str, cost: &[(Res, u32)]) -> String {
        let need: Vec<String> = cost.iter().map(|&(r, n)| format!("{} {}", n, RESOURCES[r as usize].name)).collect();
        format!("{} NEEDS {}", what, need.join(" "))
    }

    fn do_build(&mut self) {
        let recipe = &RECIPES[self.selected];
        let (tx, ty) = self.target();
        let bench = tile(self.world.get(tx, ty)).bench;
        match self.plan() {
            Plan::Repair { id, cost } => {
                if affordable(&cost, &self.inv) {
                    for &(r, n) in &cost {
                        self.inv[r as usize] -= n;
                    }
                    self.world.heal(tx, ty);
                    let (cx, cy) = tile_center(tx, ty);
                    self.burst(cx, cy, 6, rgb(140, 230, 120));
                    self.say(format!("{} REPAIRED", tile(id).name), 45);
                } else if bench {
                    self.open_bench(); // can't mend it yet, but you can still use it
                } else {
                    self.say(Self::needs("REPAIR", &cost), 60);
                }
                return;
            }
            Plan::Nothing => {
                if bench {
                    self.open_bench();
                }
                return;
            }
            Plan::Build => {}
        }
        if !recipe.affordable(&self.inv) {
            self.say(Self::needs(recipe.name(), recipe.cost), 60);
            return;
        }
        let floor = tile(recipe.tile).is_floor();
        if floor {
            for &(r, n) in recipe.cost {
                self.inv[r as usize] -= n;
            }
            self.world.set_ground(tx, ty, recipe.tile);
            let (cx, cy) = tile_center(tx, ty);
            self.burst(cx, cy + 3.0, 4, tile(recipe.tile).debris);
            return;
        }
        if overlaps_tile(self.x, self.y, tx, ty) {
            // Step back out of the tile if we're only slightly inside it.
            let (dx, dy) = DIRS[self.face];
            for k in 1..=4 {
                let (nx, ny) = (self.x - (dx * k) as f32, self.y - (dy * k) as f32);
                if !overlaps_tile(nx, ny, tx, ty) && box_free(&self.world, nx, ny, false) {
                    (self.x, self.y) = (nx, ny);
                    break;
                }
            }
        }
        if overlaps_tile(self.x, self.y, tx, ty) || self.zombies.iter().any(|z| overlaps_tile(z.x, z.y, tx, ty)) {
            self.say("BLOCKED".into(), 30);
            return;
        }
        for &(r, n) in recipe.cost {
            self.inv[r as usize] -= n;
        }
        self.world.set(tx, ty, recipe.tile);
        if tile(recipe.tile).spawn {
            self.bed = Some((tx, ty));
            self.say("SPAWN POINT SET".into(), 75);
        }
        if tile(recipe.tile).light > 0 {
            self.lights.push((tx, ty));
        }
        if tile(recipe.tile).turret.is_some() {
            self.turrets.push(TurretState { tx, ty, cooldown: 30, fired: 0 });
        }
        let (cx, cy) = tile_center(tx, ty);
        self.burst(cx, cy + 4.0, 6, tile(recipe.tile).debris);
    }

    fn open_bench(&mut self) {
        self.bench = Some(0);
        self.note.1 = 0; // the bench shows its own messages in place of the description
    }

    /// The player's best tool of a kind, if they own one.
    fn best(&self, kind: Tool) -> Option<&'static ToolDef> {
        let tier = self.tools[kind as usize];
        TOOLS.iter().find(|t| t.kind == kind && t.tier == tier)
    }

    /// The player's tool for breaking this kind of tile, if they own one.
    fn tool_for(&self, def: &TileDef) -> Option<&'static ToolDef> {
        self.best(def.tool?)
    }

    fn owns(&self, t: &ToolDef) -> bool {
        self.tools[t.kind as usize] >= t.tier
    }

    /// Toolbench screen: A/D choose, B/Space/Enter make, Tab/Esc close.
    fn update_bench(&mut self, inp: &Input) {
        let Some(sel) = self.bench else { return };
        let n = TOOLS.len();
        let mut sel = sel;
        if inp.pressed(LEFT) || inp.pressed(UP) {
            sel = (sel + n - 1) % n;
        }
        if inp.pressed(RIGHT) || inp.pressed(DOWN) {
            sel = (sel + 1) % n;
        }
        self.bench = Some(sel);
        if inp.pressed(MENU_CLOSE) || inp.pressed(MENU) {
            self.bench = None;
        } else if inp.pressed(MENU_OK) && self.craft(sel) {
            self.bench = None;
        }
    }

    /// Make TOOLS[i] if you don't have it and can pay. Returns whether it was made.
    fn craft(&mut self, i: usize) -> bool {
        let t = &TOOLS[i];
        if self.owns(t) {
            self.say(format!("YOU HAVE THE {}", t.name), 60);
            return false;
        }
        if self.tools[t.kind as usize] + 1 < t.tier {
            let prev = TOOLS.iter().find(|p| p.kind == t.kind && p.tier + 1 == t.tier).map_or("", |p| p.name);
            self.say(format!("MAKE THE {} FIRST", prev), 60);
            return false;
        }
        if !affordable(t.cost, &self.inv) {
            let short: Vec<String> = t
                .cost
                .iter()
                .filter(|&&(r, n)| self.inv[r as usize] < n)
                .map(|&(r, n)| format!("{} {}", n - self.inv[r as usize], RESOURCES[r as usize].name))
                .collect();
            self.say(format!("NEED {} MORE", short.join(" ")), 60);
            return false;
        }
        for &(r, n) in t.cost {
            self.inv[r as usize] -= n;
        }
        self.tools[t.kind as usize] = t.tier;
        self.say(format!("{} MADE!", t.name), 90);
        true
    }

    /// Too close to a light for zombies to rise.
    fn lit(&self, x: f32, y: f32) -> bool {
        self.lights.iter().any(|&(tx, ty)| {
            let (cx, cy) = tile_center(tx, ty);
            (cx - x).powi(2) + (cy - y).powi(2) < LIGHT_SAFE * LIGHT_SAFE
        })
    }

    fn spawn_zombies(&mut self) {
        let (phase, day) = (self.phase(), self.day());
        if !(SPAWN_START..DAWN).contains(&phase) {
            return;
        }
        if self.spawn_night != day {
            self.spawn_night = day;
            self.spawned = 0;
        }
        // A tier-n night brings 1 + 2n zombies, trickling in over the first 60% of the night.
        let total = (1 + 2 * tier(self.x, self.y)).min(60);
        let window = (DAWN - SPAWN_START) * 6 / 10;
        let due = ((phase - SPAWN_START) * total / window + 1).min(total) as u32;
        while self.spawned < due {
            self.spawned += 1;
            for _ in 0..12 {
                let a = self.randf() * std::f32::consts::TAU;
                let d = (12.0 + self.randf() * 4.0) * 8.0;
                let (zx, zy) = (self.x + a.cos() * d, self.y + a.sin() * d);
                if box_free(&self.world, zx, zy, true) && !self.lit(zx, zy) {
                    let hp = 3 + (tier(zx, zy) / 4) as i32;
                    self.zombies.push(Zombie {
                        x: zx,
                        y: zy,
                        hp,
                        flash: 0,
                        cooldown: 0,
                        stuck: 0,
                        wander: (0.0, 0.0, 0),
                        left: false,
                    });
                    break;
                }
            }
        }
    }

    /// Dijkstra from the player's tile: open ground costs 1, breakable blocks BREAK_COST, and
    /// rocks/trees are impassable. Zombies walk downhill, so they go around obstacles and only
    /// claw through walls when that's shorter than the detour.
    fn build_flow(&mut self) {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        let (px, py) = (tile_of(self.x), tile_of(self.y));
        let (ox, oy) = (px - FLOW_R, py - FLOW_R);
        self.flow_origin = (ox, oy);
        self.flow.fill(u16::MAX);
        let start = (FLOW_R as usize) * FLOW_N + FLOW_R as usize;
        self.flow[start] = 0;
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((0u16, start as u32)));
        while let Some(Reverse((d, i))) = heap.pop() {
            let i = i as usize;
            if d > self.flow[i] {
                continue;
            }
            let (cx, cy) = ((i % FLOW_N) as i32, (i / FLOW_N) as i32);
            for (dx, dy) in DIRS {
                let (nx, ny) = (cx + dx, cy + dy);
                if nx < 0 || ny < 0 || nx >= FLOW_N as i32 || ny >= FLOW_N as i32 {
                    continue;
                }
                let def = tile(self.world.get(ox + nx, oy + ny));
                let cost = if !def.blocks_zombie {
                    1
                } else if def.zombies_break {
                    BREAK_COST
                } else {
                    continue;
                };
                let j = ny as usize * FLOW_N + nx as usize;
                if d + cost < self.flow[j] {
                    self.flow[j] = d + cost;
                    heap.push(Reverse((d + cost, j as u32)));
                }
            }
        }
    }

    fn flow_at(&self, tx: i32, ty: i32) -> u16 {
        let (x, y) = (tx - self.flow_origin.0, ty - self.flow_origin.1);
        if x < 0 || y < 0 || x >= FLOW_N as i32 || y >= FLOW_N as i32 {
            return u16::MAX;
        }
        self.flow[y as usize * FLOW_N + x as usize]
    }

    /// Direction toward the centre of the neighbouring tile closest to the player by path.
    fn flow_dir(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let (tx, ty) = (tile_of(x), tile_of(y));
        let here = self.flow_at(tx, ty);
        if here == u16::MAX || here <= 1 {
            return None; // off the map or next to the player: go straight for them
        }
        let (bx, by) = DIRS
            .iter()
            .map(|&(dx, dy)| (tx + dx, ty + dy))
            .min_by_key(|&(nx, ny)| self.flow_at(nx, ny))?;
        if self.flow_at(bx, by) >= here {
            return None;
        }
        let (cx, cy) = tile_center(bx, by);
        let (dx, dy) = (cx - x, cy - y);
        let d = (dx * dx + dy * dy).sqrt().max(0.01);
        Some((dx / d, dy / d))
    }

    fn update_zombies(&mut self) {
        if !self.zombies.is_empty() && self.tick % 15 == 0 {
            self.build_flow();
        }
        let steer: Vec<Option<(f32, f32)>> = self.zombies.iter().map(|z| self.flow_dir(z.x, z.y)).collect();
        let burning = self.ambient() >= 200;
        let mut puffs = Vec::new();
        let mut clawed: Vec<(i32, i32)> = Vec::new();
        for i in 0..self.zombies.len() {
            let r = self.randf();
            let r2 = self.randf();
            let z = &mut self.zombies[i];
            let speed = zombie_speed(z.x, z.y);
            z.flash = z.flash.saturating_sub(1);
            z.cooldown = z.cooldown.saturating_sub(1);
            if burning && self.tick % 15 == 0 {
                z.hp -= 1;
                z.flash = 4;
                puffs.push((z.x, z.y - 4.0, 3, rgb(255, 140, 30)));
            }
            let (ddx, ddy) = (self.x - z.x, self.y - z.y);
            let dist = (ddx * ddx + ddy * ddy).sqrt().max(0.01);
            let (mx, my) = if z.wander.2 > 0 {
                z.wander.2 -= 1;
                (z.wander.0, z.wander.1)
            } else {
                steer[i].unwrap_or((ddx / dist, ddy / dist))
            };
            let (ox, oy) = (z.x, z.y);
            let (bx, by) = slide(&self.world, &mut z.x, &mut z.y, mx * speed, my * speed, true);
            if mx.abs() > 0.1 {
                z.left = mx < 0.0;
            }
            // Blocked by something breakable (walls, doors): claw at it.
            let mut attacking = false;
            for (blocked, tx, ty) in [
                (bx, tile_of(z.x + mx.signum() * (HALF + 1.0)), tile_of(z.y)),
                (by, tile_of(z.x), tile_of(z.y + my.signum() * (HALF + 1.0))),
            ] {
                if blocked && tile(self.world.get(tx, ty)).zombies_break {
                    attacking = true;
                    if z.cooldown == 0 {
                        z.cooldown = 45;
                        clawed.push((tx, ty));
                    }
                    break;
                }
            }
            let moved = (z.x - ox).abs() + (z.y - oy).abs();
            if moved < speed * 0.3 && !attacking {
                z.stuck += 1;
                if z.stuck > 20 {
                    let a = r * std::f32::consts::TAU;
                    z.wander = (a.cos(), a.sin(), 20 + (r2 * 30.0) as u16);
                    z.stuck = 0;
                }
            } else {
                z.stuck = 0;
            }
            // Bite.
            if ddx.abs() < 6.0 && ddy.abs() < 6.0 && self.hurt == 0 && self.dead.is_none() {
                self.hp -= 1;
                self.hurt = 30;
                let (mut px, mut py) = (self.x, self.y);
                slide(&self.world, &mut px, &mut py, ddx / dist * 6.0, ddy / dist * 6.0, false);
                (self.x, self.y) = (px, py);
                puffs.push((self.x, self.y, 5, rgb(220, 40, 50)));
            }
        }
        for (tx, ty) in clawed {
            self.damage_tile(tx, ty, false);
        }
        // Keep zombies from stacking on one spot.
        for i in 0..self.zombies.len() {
            for j in i + 1..self.zombies.len() {
                let (dx, dy) = (self.zombies[j].x - self.zombies[i].x, self.zombies[j].y - self.zombies[i].y);
                let d2 = dx * dx + dy * dy;
                if d2 < 36.0 && d2 > 0.0001 {
                    let d = d2.sqrt();
                    let (px, py) = (dx / d * 0.3, dy / d * 0.3);
                    let w = &self.world;
                    let (a, b) = self.zombies.split_at_mut(j);
                    slide(w, &mut a[i].x, &mut a[i].y, -px, -py, true);
                    slide(w, &mut b[0].x, &mut b[0].y, px, py, true);
                }
            }
        }
        let (px, py) = (self.x, self.y);
        self.zombies.retain(|z| {
            let keep = z.hp > 0 && (z.x - px).abs() < 320.0 && (z.y - py).abs() < 320.0;
            if !keep && z.hp <= 0 {
                puffs.push((z.x, z.y, 10, rgb(110, 160, 70)));
            }
            keep
        });
        for (x, y, n, c) in puffs {
            self.burst(x, y, n, c);
        }
    }

    /// Turrets fire at the nearest zombie in range, spending ammo from the player's inventory.
    fn update_turrets(&mut self) {
        let w = &self.world;
        // Turrets in chunks that were paged out go idle; maintain() finds them again.
        self.turrets.retain(|t| w.peek(t.tx, t.ty).is_some_and(|id| tile(id).turret.is_some()));
        self.ammo_warn = self.ammo_warn.saturating_sub(1);
        for i in 0..self.turrets.len() {
            let (tx, ty) = (self.turrets[i].tx, self.turrets[i].ty);
            let def = tile(self.world.get(tx, ty));
            let Some(spec) = &def.turret else { continue };
            let ts = &mut self.turrets[i];
            ts.fired = ts.fired.saturating_sub(1);
            if ts.cooldown > 0 {
                ts.cooldown -= 1;
                continue;
            }
            let (cx, cy) = tile_center(tx, ty);
            let r2 = spec.range * spec.range;
            let w = &self.world;
            let target = self
                .zombies
                .iter()
                .filter(|z| z.hp > 0)
                .map(|z| (z.x, z.y, (z.x - cx).powi(2) + (z.y - cy).powi(2)))
                .filter(|t| t.2 <= r2)
                // Flat shots need a clear line to the zombie's tile.
                .filter(|t| spec.arc || first_blocked(w, (cx, cy), (t.0, t.1), (tx, ty)).is_none())
                .min_by(|a, b| a.2.total_cmp(&b.2));
            let Some((zx, zy, _)) = target else { continue };
            let (res, n) = spec.ammo;
            if self.inv[res as usize] < n {
                if self.ammo_warn == 0 {
                    self.ammo_warn = 300;
                    self.say(format!("{} NEEDS {}", def.name, RESOURCES[res as usize].name), 75);
                }
                continue;
            }
            self.inv[res as usize] -= n;
            // Lead the shot: zombies walk toward the player.
            let (dx, dy) = (self.x - zx, self.y - zy);
            let d = (dx * dx + dy * dy).sqrt().max(0.01);
            let lead = (zombie_speed(zx, zy) * spec.flight as f32).min(d);
            let ts = &mut self.turrets[i];
            ts.cooldown = spec.reload;
            ts.fired = 20;
            self.projectiles.push(Projectile {
                x0: cx,
                y0: cy,
                x1: zx + dx / d * lead,
                y1: zy + dy / d * lead,
                t: 0,
                dur: spec.flight,
                damage: spec.damage,
                splash: spec.splash,
                arc: spec.arc,
                from: (tx, ty),
            });
            self.burst(cx, cy - 10.0, 3, rgb(190, 110, 60));
        }
    }

    fn update_projectiles(&mut self) {
        let mut impacts = Vec::new();
        let mut thuds = Vec::new();
        let w = &self.world;
        for p in &mut self.projectiles {
            let (ox, oy, _) = p.pos();
            p.t += 1;
            let (nx, ny, _) = p.pos();
            if !p.arc {
                if let Some((bx, by)) = first_blocked(w, (ox, oy), (nx, ny), p.from) {
                    thuds.push((bx, by));
                    p.t = u16::MAX; // stuck in a wall
                    continue;
                }
            }
            if p.t >= p.dur {
                impacts.push((p.x1, p.y1, p.damage, p.splash));
            }
        }
        self.projectiles.retain(|p| p.t < p.dur);
        for (x, y) in thuds {
            self.burst(x, y, 4, rgb(190, 110, 60));
        }
        for (x, y, damage, splash) in impacts {
            for z in &mut self.zombies {
                if z.hp > 0 && (z.x - x).powi(2) + (z.y - y).powi(2) <= splash * splash {
                    z.hp -= damage;
                    z.flash = 6;
                    if z.hp <= 0 {
                        self.kills += 1;
                    }
                }
            }
            self.burst(x, y, 10, rgb(170, 170, 190));
            self.burst(x, y, 5, rgb(120, 100, 80));
        }
    }

    fn update_particles(&mut self) {
        for p in &mut self.particles {
            p.x += p.vx;
            p.y += p.vy;
            p.vy += 0.15;
            p.life -= 1;
        }
        self.particles.retain(|p| p.life > 0);
    }

    // ---------------------------------------------------------------- drawing

    /// Turrets show their "fired" sprite right after a shot and while out of ammo.
    fn state_sprite(&self, t: Tile, tx: i32, ty: i32, normal: usize) -> usize {
        let Some(spec) = &tile(t).turret else { return normal };
        let fired = self.turrets.iter().any(|s| s.tx == tx && s.ty == ty && s.fired > 0);
        if fired || self.inv[spec.ammo.0 as usize] < spec.ammo.1 { spec.fired } else { normal }
    }

    fn draw_tile_object(&self, cv: &mut Canvas, t: Tile, tx: i32, ty: i32, sx: i32, sy: i32) {
        let def = tile(t);
        let h = hash(tx, ty, self.world.seed);
        let pick = |v: &[usize]| v[(h >> 6) as usize % v.len()];
        match def.look {
            Look::Ground | Look::Floor(_) => return, // floors are drawn with the ground
            Look::Small(v, flip) => {
                cv.sprite(self.state_sprite(t, tx, ty, pick(v)), sx, sy, flip && h & 128 != 0, None)
            }
            Look::Big(v, w, bh, flip) => {
                let id = self.state_sprite(t, tx, ty, pick(v));
                cv.sprite_big(id, w, bh, sx - (w * 8 - 8) / 2, sy - (bh - 1) * 8, flip && h & 128 != 0)
            }
        }
        let d = self.world.damage(tx, ty);
        if d > 0 && def.hp > 0 {
            draw_cracks(cv, sx, sy, (d as u32 * 4 / def.hp as u32).clamp(1, 3));
        }
    }

    fn draw_zombie(&self, cv: &mut Canvas, i: usize, camx: i32, camy: i32) {
        let z = &self.zombies[i];
        let (sx, sy) = (z.x.round() as i32 - 4 - camx, z.y.round() as i32 - 4 - camy);
        cv.darken(sx + 1, sy + 7, 6, 1);
        let frame = if (self.tick / 10 + i as u64) % 2 == 0 { spr::ZOMBIE0 } else { spr::ZOMBIE1 };
        let flash = (z.flash > 0).then_some(rgb(255, 255, 255));
        cv.sprite(frame, sx, sy, z.left, flash);
    }

    pub fn render(&self, cv: &mut Canvas) {
        let camx = self.x.round() as i32 - W / 2;
        let camy = self.y.round() as i32 - H / 2;
        let seed = self.world.seed;
        let (tx0, ty0) = (camx.div_euclid(8), camy.div_euclid(8));
        let (tx1, ty1) = (tx0 + W / 8 + 1, ty0 + H / 8 + 2); // +1 row below: tall sprites overhang upward

        // Ground: floors where they've been laid, grass elsewhere.
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let h = hash(tx, ty, seed);
                let (sx, sy) = (tx * 8 - camx, ty * 8 - camy);
                let floor = tile(self.world.ground(tx, ty));
                if let Look::Floor(v) = floor.look {
                    cv.sprite(v[(h >> 6) as usize % v.len()], sx, sy, false, None);
                    let d = self.world.damage(tx, ty);
                    if d > 0 && self.world.get(tx, ty) == GRASS {
                        draw_cracks(cv, sx, sy, (d as u32 * 4 / floor.hp.max(1) as u32).clamp(1, 3));
                    }
                    continue;
                }
                let g = match h % 24 {
                    0 => spr::GRASS1,
                    1 => spr::GRASS2,
                    2 => spr::GRASS3,
                    3 => spr::GRASS4,
                    _ => spr::GRASS0,
                };
                cv.sprite(g, tx * 8 - camx, ty * 8 - camy, h & 32 != 0, None);
            }
        }

        // Objects and creatures, row by row, so whatever is further south is drawn on top.
        let mut ents: Vec<(f32, usize)> = self.zombies.iter().enumerate().map(|(i, z)| (z.y, i)).collect();
        if self.dead.is_none() {
            ents.push((self.y, usize::MAX));
        }
        ents.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut next = 0;
        let draw_until = |cv: &mut Canvas, row: i32, next: &mut usize| {
            while *next < ents.len() && tile_of(ents[*next].0) <= row {
                match ents[*next].1 {
                    usize::MAX => self.draw_player(cv, camx, camy),
                    i => self.draw_zombie(cv, i, camx, camy),
                }
                *next += 1;
            }
        };
        draw_until(cv, ty0 - 1, &mut next);
        for ty in ty0..=ty1 {
            for tx in tx0 - 1..=tx1 {
                let t = self.world.get(tx, ty);
                if t != GRASS {
                    self.draw_tile_object(cv, t, tx, ty, tx * 8 - camx, ty * 8 - camy);
                }
            }
            draw_until(cv, ty, &mut next);
        }
        draw_until(cv, i32::MAX, &mut next);
        // Show the player through tall things standing in front of them.
        if self.dead.is_none() {
            let row = tile_of(self.y) + 1;
            let hidden = [tile_of(self.x - HALF), tile_of(self.x + HALF - 0.01)]
                .iter()
                .any(|&tx| matches!(tile(self.world.get(tx, row)).look, Look::Big(_, _, h, _) if h > 1));
            if hidden {
                let (sx, sy) = (self.x.round() as i32 - 4 - camx, self.y.round() as i32 - 4 - camy);
                cv.sprite_ghost(spr::PLAYER, sx, sy, self.face == 2, rgb(255, 240, 200));
            }
        }
        for p in &self.particles {
            cv.pixel(p.x.round() as i32 - camx, p.y.round() as i32 - camy, p.c);
        }
        for p in &self.projectiles {
            let (x, y, _) = p.pos();
            cv.darken(x.round() as i32 - 1 - camx, y.round() as i32 - camy, 3, 1);
        }

        // Lighting: ambient plus the player's lantern and any lights on screen (which flicker).
        let amb = self.ambient();
        if amb < 256 {
            let mut lights = Vec::new();
            if self.dead.is_none() {
                lights.push((self.x.round() as i32 - camx, self.y.round() as i32 - camy, 34));
            }
            for &(tx, ty) in &self.lights {
                let r = tile(self.world.get(tx, ty)).light as i32;
                let (sx, sy) = (tx * 8 + 4 - camx, ty * 8 + 1 - camy);
                if sx > -r && sy > -r && sx < W + r && sy < H + r {
                    let flicker = ((self.tick / 5) as u32 ^ hash(tx, ty, seed)) % 3;
                    lights.push((sx, sy, r + flicker as i32 - 1));
                }
            }
            cv.light(amb, &lights);
            if amb < 160 {
                // Glowing eyes in the dark.
                for z in &self.zombies {
                    let (sx, sy) = (z.x.round() as i32 - 4 - camx, z.y.round() as i32 - 4 - camy);
                    let eye = rgb(255, 60, 40);
                    let (a, b) = if z.left { (2, 4) } else { (3, 5) };
                    cv.pixel(sx + a, sy + 3, eye);
                    cv.pixel(sx + b, sy + 3, eye);
                }
            }
        }

        // Projectiles in flight stay visible at night: rocks, or bolts drawn along their path.
        for p in &self.projectiles {
            let (x, y, z) = p.pos();
            let (rx, ry) = (x.round() as i32 - 1 - camx, (y - z).round() as i32 - 1 - camy);
            if p.arc {
                cv.fill(rx, ry, 2, 2, rgb(200, 205, 215));
                cv.pixel(rx + 1, ry + 1, rgb(110, 100, 130));
            } else {
                let (dx, dy) = (p.x1 - p.x0, p.y1 - p.y0);
                let d = (dx * dx + dy * dy).sqrt().max(0.01);
                for k in 0..4 {
                    let (bx, by) = (x - dx / d * k as f32, y - z - dy / d * k as f32);
                    let c = if k == 0 { rgb(220, 225, 235) } else { rgb(160, 90, 50) };
                    cv.pixel(bx.round() as i32 - camx, by.round() as i32 - camy, c);
                }
            }
        }

        // Target marker, plus turret range when looking at (or about to build) one.
        if self.dead.is_none() && !self.menu {
            let (tx, ty) = self.target();
            let t = self.world.get(tx, ty);
            let placing = RECIPES[self.selected].tile;
            let range = match (&tile(t).turret, &tile(placing).turret) {
                (Some(spec), _) => Some(spec.range),
                (None, Some(spec)) if t == GRASS => Some(spec.range),
                _ => None,
            };
            if let (Some(r), true) = (range, self.tick.saturating_sub(self.aim.1) < 90) {
                let (cx, cy) = (tx * 8 + 4 - camx, ty * 8 + 4 - camy);
                for i in 0..256 {
                    let a = i as f32 / 256.0 * std::f32::consts::TAU;
                    if (i + (self.tick / 2) as i32) % 8 < 5 {
                        cv.pixel(cx + (a.cos() * r) as i32, cy + (a.sin() * r) as i32, rgb(255, 230, 140));
                    }
                }
            }
            let (sx, sy) = (tx * 8 - camx, ty * 8 - camy);
            let c = if matches!(self.plan(), Plan::Repair { .. }) { rgb(140, 230, 120) } else { rgb(255, 255, 220) };
            for (x, y) in [(0, 0), (1, 0), (0, 1), (7, 0), (6, 0), (7, 1), (0, 7), (1, 7), (0, 6), (7, 7), (6, 7), (7, 6)] {
                cv.pixel(sx + x, sy + y, c);
            }
        }
        match &self.pause {
            Some(m) if m.title => self.draw_pause(cv, m), // title screen: no HUD
            Some(m) => {
                self.draw_hud(cv);
                self.draw_pause(cv, m);
            }
            None => self.draw_hud(cv),
        }
    }

    /// Title screen at launch, pause menu in game: same list, different heading.
    fn draw_pause(&self, cv: &mut Canvas, m: &PauseMenu) {
        cv.darken(0, 0, W, H);
        if m.title {
            cv.darken(0, 0, W, H);
            cv.text_centered(14, "NIGHTMASON", rgb(255, 220, 90), 3);
            let sub = if self.tick > 0 { format!("DAY {}", self.day()) } else { "A NEW WORLD".into() };
            cv.text_centered(34, &sub, rgb(170, 170, 190), 1);
        } else {
            cv.text_centered(16, "PAUSED", rgb(255, 255, 255), 2);
        }
        let first = match (m.title, self.tick > 0) {
            (false, _) => "RESUME",
            (true, true) => "CONTINUE",
            (true, false) => "PLAY",
        };
        let new = if m.confirm { "DELETE THIS WORLD?" } else { "NEW WORLD" };
        for (i, label) in [first, new, "SAVE AND QUIT"].iter().enumerate() {
            let y = 48 + i as i32 * 9;
            let c = match (i == m.sel, i == 1 && m.confirm) {
                (true, true) => rgb(255, 110, 90),
                (true, false) => rgb(255, 230, 120),
                _ => rgb(150, 150, 170),
            };
            let text = if i == m.sel { format!("> {label} <") } else { label.to_string() };
            cv.text_centered(y, &text, c, 1);
        }
        let hint = if m.confirm && m.sel == 1 { "ENTER AGAIN TO DELETE" } else { "W/S PICK  ENTER OK" };
        cv.text_centered(H - 9, hint, rgb(120, 120, 140), 1);
    }

    fn draw_player(&self, cv: &mut Canvas, camx: i32, camy: i32) {
        let bob = if self.walking && (self.tick / 6) % 2 == 0 { -1 } else { 0 };
        let (sx, sy) = (self.x.round() as i32 - 4 - camx, self.y.round() as i32 - 4 - camy);
        cv.darken(sx + 1, sy + 7, 6, 1);
        if self.hurt > 0 && self.hurt % 6 < 3 {
            cv.sprite(spr::PLAYER, sx, sy + bob, self.face == 2, Some(rgb(255, 80, 80)));
        } else {
            cv.sprite(spr::PLAYER, sx, sy + bob, self.face == 2, None);
        }
        if self.punch > 4 {
            // Fist thrown toward the facing direction.
            let (dx, dy) = DIRS[self.face];
            let reach = 5 + (8 - self.punch as i32);
            let (fx, fy) = (sx + 3 + dx * reach, sy + 4 + dy * reach);
            cv.fill(fx, fy, 2, 2, rgb(238, 195, 154));
            cv.pixel(fx - dx * 2, fy - dy * 2, rgb(255, 255, 255));
        }
    }

    /// Draw "<icon><n>" cost entries starting at x; returns the end x.
    fn draw_cost(&self, cv: &mut Canvas, mut x: i32, y: i32, cost: &[(Res, u32)]) -> i32 {
        for &(r, n) in cost {
            let enough = self.inv[r as usize] >= n;
            cv.sprite(RESOURCES[r as usize].icon, x, y - 2, false, None);
            let s = n.to_string();
            cv.text(x + 9, y, &s, if enough { rgb(255, 255, 255) } else { rgb(255, 90, 90) }, 1);
            x += 9 + s.len() as i32 * 4 + 2;
        }
        x
    }

    fn draw_hud(&self, cv: &mut Canvas) {
        let white = rgb(255, 255, 255);
        // Hearts.
        const HEART: [u8; 5] = [0b01010, 0b11111, 0b11111, 0b01110, 0b00100];
        for i in 0..MAX_HP / 2 {
            let fill = (self.hp - i * 2).clamp(0, 2);
            for (r, bits) in HEART.iter().enumerate() {
                for b in 0..5 {
                    if bits >> (4 - b) & 1 == 1 {
                        let lit = fill == 2 || (fill == 1 && b < 3);
                        let c = if lit { rgb(230, 40, 60) } else { rgb(60, 20, 30) };
                        let (x, y) = (2 + i * 7 + b, 2 + r as i32);
                        cv.pixel(x + 1, y + 1, 0);
                        cv.pixel(x, y, c);
                    }
                }
            }
        }

        // Tools you own, after the hearts.
        let mut x = 2 + MAX_HP / 2 * 7 + 2;
        for t in TOOLS.iter().filter(|t| self.tools[t.kind as usize] == t.tier) {
            cv.sprite(t.icon, x, 1, false, None);
            x += 9;
        }

        // Inventory, right-aligned along the top: one row per resource.
        for (i, res) in RESOURCES.iter().enumerate() {
            let s = self.inv[i].to_string();
            let sw = s.len() as i32 * 4;
            let y = i as i32 * 10;
            cv.darken(W - sw - 13, y, sw + 13, 10);
            cv.sprite(res.icon, W - sw - 11, y + 1, false, None);
            cv.text(W - sw - 1, y + 3, &s, white, 1);
        }

        // Day / night clock.
        let night = (SPAWN_START..DAWN).contains(&self.phase());
        let (label, c) = if night {
            (format!("NIGHT {}", self.day()), rgb(140, 160, 255))
        } else {
            (format!("DAY {}", self.day()), rgb(255, 220, 90))
        };
        cv.text_centered(2, &label, c, 1);
        let danger = tier(self.x, self.y);
        if danger > 1 && self.dead.is_none() {
            cv.text_centered(13, &format!("DANGER {danger}"), rgb(230, 90, 80), 1);
        }
        let bw = 30;
        let bx = (W - bw) / 2;
        cv.fill(bx, 9, bw, 2, rgb(30, 30, 50));
        let done = (self.phase() * bw as u64 / DAY_LEN) as i32;
        cv.fill(bx, 9, done, 2, c);
        let night_x = bx + (SPAWN_START * bw as u64 / DAY_LEN) as i32;
        cv.pixel(night_x, 8, rgb(140, 160, 255));

        // Bottom right: what B does at the target. The selected item and its cost; or what a
        // repair there costs; or the selected item crossed out when it can't go there.
        let rec = &RECIPES[self.selected];
        let cost_w = |c: &[(Res, u32)]| -> i32 { c.iter().map(|&(_, n)| 11 + n.to_string().len() as i32 * 4).sum() };
        match self.plan() {
            Plan::Build => {
                let bx = W - cost_w(rec.cost) - 12;
                cv.darken(bx - 2, H - 11, cost_w(rec.cost) + 14, 11);
                cv.sprite(rec.icon(), bx, H - 10, false, None);
                self.draw_cost(cv, bx + 10, H - 7, rec.cost);
            }
            Plan::Repair { id, cost } => {
                let icon = RECIPES.iter().find(|r| r.tile == id).map_or(rec.icon(), |r| r.icon());
                let w = cost_w(&cost);
                let bx = W - w - 12;
                let lx = bx - 4 * 6 - 2; // "REPAIR" is 6 glyphs of 4 px
                cv.darken(lx - 2, H - 11, W - lx + 2, 11);
                cv.text(lx, H - 7, "REPAIR", rgb(140, 230, 120), 1);
                cv.sprite(icon, bx, H - 10, false, None);
                self.draw_cost(cv, bx + 10, H - 7, &cost);
            }
            Plan::Nothing => {
                let bx = W - 10;
                cv.darken(bx - 2, H - 11, 12, 11);
                cv.sprite(rec.icon(), bx, H - 10, false, None);
                cv.darken(bx, H - 10, 8, 8);
                let red = rgb(230, 60, 60);
                for i in 0..8 {
                    cv.pixel(bx + i, H - 10 + i, red);
                    cv.pixel(bx + 7 - i, H - 10 + i, red);
                }
            }
        }

        if self.banner.1 > 0 && self.dead.is_none() && self.pause.is_none() {
            cv.text_centered(24, &self.banner.0, white, 2);
        }
        if self.tick < 300 && self.dead.is_none() && !self.menu && self.pause.is_none() {
            cv.text(2, H - 14, "SPACE PUNCH  B BUILD", rgb(230, 230, 230), 1);
            cv.text(2, H - 7, "TAB BUILD MENU", rgb(230, 230, 230), 1);
        } else if self.note.1 > 0 && self.pause.is_none() {
            cv.text(2, H - 7, &self.note.0, rgb(255, 200, 120), 1);
        }
        if self.esc > 0 {
            cv.text_centered(H - 22, "HOLD ESC TO QUIT", white, 1);
        }
        if self.menu {
            self.draw_menu(cv);
        }
        if let Some(sel) = self.bench {
            self.draw_bench(cv, sel);
        }
        if let Some(t) = self.dead {
            cv.darken(0, 0, W, H);
            cv.text_centered(18, "YOU DIED", rgb(230, 50, 60), 2);
            let n = self.nights;
            cv.text_centered(36, &format!("SURVIVED {} NIGHT{}", n, if n == 1 { "" } else { "S" }), rgb(220, 220, 220), 1);
            cv.text_centered(44, &format!("ZOMBIES PUNCHED OUT: {}", self.kills), rgb(220, 220, 220), 1);
            if t > 45 {
                cv.text_centered(60, "ENTER TO RESPAWN", rgb(255, 230, 120), 1);
            }
        }
    }

    /// Build menu: a full-screen grid of RECIPES, with the selected one described underneath.
    fn draw_menu(&self, cv: &mut Canvas) {
        let dim = rgb(150, 150, 170);
        cv.fill(0, 0, W, H, rgb(20, 18, 30));

        // Category tabs, Q and E either side.
        let cat = RECIPES[self.selected].cat;
        let widths: Vec<i32> = CATEGORIES.iter().map(|c| c.len() as i32 * 4 + 5).collect();
        let mut x = (W - widths.iter().sum::<i32>() - 2 * (CATEGORIES.len() as i32 - 1)) / 2;
        cv.text(x - 9, 3, "Q", dim, 1);
        for (c, (name, w)) in CATEGORIES.iter().zip(&widths).enumerate() {
            let on = c == cat as usize;
            cv.fill(x, 1, *w, 9, if on { rgb(70, 60, 100) } else { rgb(36, 32, 52) });
            if on {
                cv.fill(x, 9, *w, 1, rgb(255, 220, 90));
            }
            cv.text(x + 3, 3, name, if on { rgb(255, 220, 90) } else { dim }, 1);
            x += w + 2;
        }
        cv.text(x + 4, 3, "E", dim, 1);

        // This category's grid, scrolled so the selected row is visible.
        let items = self.menu_items();
        let pos = items.iter().position(|&i| i == self.selected).unwrap_or(0);
        let rows = items.len().div_ceil(MENU_COLS);
        let sel_row = pos / MENU_COLS;
        let first_row = sel_row.saturating_sub(MENU_ROWS - 1).min(rows.saturating_sub(MENU_ROWS));
        let x0 = (W - MENU_COLS as i32 * MENU_CELL) / 2;
        let y0 = 12;
        for (p, &i) in items.iter().enumerate().skip(first_row * MENU_COLS).take(MENU_ROWS * MENU_COLS) {
            let rec = &RECIPES[i];
            let (col, row) = ((p % MENU_COLS) as i32, (p / MENU_COLS - first_row) as i32);
            let (cx, cy) = (x0 + col * MENU_CELL, y0 + row * MENU_CELL);
            draw_cell(cv, cx, cy, i == self.selected, rec.art(), !rec.affordable(&self.inv));
            if i < 9 {
                cv.text(cx + 2, cy + 2, &(i + 1).to_string(), dim, 1);
            }
        }
        if first_row > 0 {
            cv.text(W - 5, y0 + 2, "↑", dim, 1);
        }
        if first_row + MENU_ROWS < rows {
            cv.text(W - 5, y0 + MENU_ROWS as i32 * MENU_CELL - 7, "↓", dim, 1);
        }

        let rec = &RECIPES[self.selected];
        self.draw_details(cv, rec.name(), Some(rec.cost), rec.desc);
    }

    /// Bottom strip of the build menu and toolbench: name, cost (None = owned) and a line of text.
    fn draw_details(&self, cv: &mut Canvas, name: &str, cost: Option<&[(Res, u32)]>, desc: &str) {
        let y = 12 + MENU_ROWS as i32 * MENU_CELL + 2;
        cv.fill(0, y - 1, W, 1, rgb(60, 54, 84));
        let ok = cost.is_none_or(|c| affordable(c, &self.inv));
        cv.text(4, y + 2, name, if ok { rgb(255, 255, 255) } else { rgb(255, 120, 110) }, 1);
        match cost {
            Some(cost) => {
                let w: i32 = cost.iter().map(|&(_, n)| 11 + n.to_string().len() as i32 * 4).sum();
                self.draw_cost(cv, W - w - 2, y + 2, cost);
            }
            None => cv.text(W - 22, y + 2, "OWNED", rgb(120, 220, 120), 1),
        }
        cv.text(4, y + 9, desc, rgb(150, 150, 170), 1);
    }

    /// Toolbench: the tools you can make, in the same grid style as the build menu.
    fn draw_bench(&self, cv: &mut Canvas, sel: usize) {
        let dim = rgb(150, 150, 170);
        cv.fill(0, 0, W, H, rgb(20, 18, 30));
        cv.text(4, 3, "TOOLBENCH", rgb(255, 220, 90), 1);
        cv.text(W - 69, 3, "B MAKE  TAB CLOSE", dim, 1);
        let x0 = (W - MENU_COLS as i32 * MENU_CELL) / 2;
        for (i, t) in TOOLS.iter().enumerate().take(MENU_COLS * MENU_ROWS) {
            let (cx, cy) = (x0 + (i % MENU_COLS) as i32 * MENU_CELL, 12 + (i / MENU_COLS) as i32 * MENU_CELL);
            let owned = self.owns(t);
            let locked = self.tools[t.kind as usize] + 1 < t.tier;
            draw_cell(cv, cx, cy, i == sel, (t.icon, 1, 1), !owned && (locked || !affordable(t.cost, &self.inv)));
            if owned {
                cv.fill(cx + 2, cy + 2, 3, 3, rgb(120, 220, 120));
            }
        }
        let t = &TOOLS[sel];
        let desc = if self.note.1 > 0 { self.note.0.as_str() } else { t.desc };
        self.draw_details(cv, t.name, (!self.owns(t)).then_some(t.cost), desc);
    }
}

/// One build-menu / toolbench cell: art standing on the bottom (up to 2x2 blocks fits), a yellow
/// frame when selected, dimmed when you can't afford it.
fn draw_cell(cv: &mut Canvas, cx: i32, cy: i32, sel: bool, (id, bw, bh): (usize, i32, i32), dimmed: bool) {
    cv.fill(cx + 1, cy + 1, MENU_CELL - 2, MENU_CELL - 2, if sel { rgb(70, 60, 100) } else { rgb(36, 32, 52) });
    if sel {
        let c = rgb(255, 220, 90);
        cv.fill(cx, cy, MENU_CELL, 1, c);
        cv.fill(cx, cy + MENU_CELL - 1, MENU_CELL, 1, c);
        cv.fill(cx, cy, 1, MENU_CELL, c);
        cv.fill(cx + MENU_CELL - 1, cy, 1, MENU_CELL, c);
    }
    cv.sprite_big(id, bw, bh, cx + (MENU_CELL - bw * 8) / 2, cy + MENU_CELL - 2 - bh * 8, false);
    if dimmed {
        cv.darken(cx + 1, cy + 1, MENU_CELL - 2, MENU_CELL - 2);
    }
}

fn draw_cracks(cv: &mut Canvas, sx: i32, sy: i32, level: u32) {
    const CRACKS: [&[(i32, i32)]; 3] = [
        &[(3, 2), (4, 3), (4, 4)],
        &[(3, 2), (4, 3), (4, 4), (5, 5), (2, 5), (3, 4), (6, 2)],
        &[(3, 2), (4, 3), (4, 4), (5, 5), (2, 5), (3, 4), (6, 2), (1, 3), (5, 6), (6, 4), (2, 1)],
    ];
    for &(x, y) in CRACKS[(level - 1) as usize] {
        cv.pixel(sx + x, sy + y, rgb(30, 25, 35));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{BALLISTA, CATAPULT, WALL};

    /// Cleared ground with a turret at (0,0), a zombie 5 tiles east, and optionally a wall
    /// between them. Ammo: 10 of everything.
    fn range(turret: Tile, wall: bool) -> Game {
        let mut g = Game::new(7, None);
        for y in -8..=8 {
            for x in -8..=8 {
                g.world.set(x, y, GRASS);
            }
        }
        g.world.set(0, 0, turret);
        if wall {
            g.world.set(2, 0, WALL);
        }
        (g.x, g.y) = tile_center(0, 6);
        g.maintain();
        g.turrets[0].cooldown = 0;
        let (x, y) = tile_center(5, 0);
        g.zombies.push(Zombie { x, y, hp: 3, flash: 0, cooldown: 0, stuck: 0, wander: (0.0, 0.0, 0), left: false });
        g.inv = [10; RES_COUNT];
        g
    }

    fn fly(g: &mut Game) {
        for _ in 0..60 {
            g.update_projectiles();
        }
    }

    /// Run a whole night of spawning around the player at the origin; returns zombies risen.
    fn night_spawns(torches: bool) -> usize {
        let mut g = Game::new(7, None);
        if torches {
            // Twelve torches on a 14-tile ring cover the whole 12-16 tile spawn band.
            for i in 0..12 {
                let a = i as f32 / 12.0 * std::f32::consts::TAU;
                g.world.set((a.cos() * 14.0) as i32, (a.sin() * 14.0) as i32, crate::content::TORCH);
            }
            g.maintain();
        }
        for t in SPAWN_START..DAWN {
            g.tick = t;
            g.spawn_zombies();
        }
        g.zombies.len()
    }

    #[test]
    fn floors_go_under_built_things_but_not_natural_ones() {
        use crate::content::{TOOLBENCH, TORCH, TREE, WOOD_FLOOR};
        let mut g = Game::new(7, None);
        g.inv = [10; RES_COUNT];
        g.selected = RECIPES.iter().position(|r| r.tile == WOOD_FLOOR).unwrap();
        (g.x, g.y) = tile_center(0, 0);
        g.face = DIRS.iter().position(|&d| d == (1, 0)).unwrap();
        let lay = |g: &mut Game, t: Tile| {
            g.world.set(1, 0, t);
            g.world.set_ground(1, 0, GRASS);
            g.do_build();
            (g.world.get(1, 0), g.world.ground(1, 0))
        };
        assert_eq!(lay(&mut g, TORCH), (TORCH, WOOD_FLOOR));
        assert_eq!(lay(&mut g, TOOLBENCH), (TOOLBENCH, WOOD_FLOOR));
        assert!(g.bench.is_none(), "laying the floor doesn't open the bench");
        assert_eq!(lay(&mut g, TREE), (TREE, GRASS));
        // Laying a floor under a cracked torch leaves it cracked, and the floor stays when it breaks.
        g.world.set(1, 0, TORCH);
        g.world.set_ground(1, 0, GRASS);
        g.world.hit(1, 0, 1);
        g.world.set_ground(1, 0, WOOD_FLOOR);
        assert_eq!(g.world.damage(1, 0), 1);
        while g.world.get(1, 0) != GRASS {
            g.damage_tile(1, 0, true);
        }
        assert_eq!(g.world.ground(1, 0), WOOD_FLOOR);
    }

    #[test]
    fn b_repairs_damaged_buildings_for_a_share_of_their_cost() {
        use crate::content::{TREE, WALL, WOOD_FLOOR};
        let mut g = Game::new(7, None);
        (g.x, g.y) = tile_center(0, 0);
        g.face = DIRS.iter().position(|&d| d == (1, 0)).unwrap();
        g.selected = RECIPES.iter().position(|r| r.tile == WALL).unwrap();
        let wall = tile(WALL);
        g.world.set(1, 0, WALL);
        g.world.set_ground(1, 0, GRASS);
        assert!(matches!(g.plan(), Plan::Nothing), "a wall can't go on a wall");
        g.world.hit(1, 0, wall.hp / 2);
        let Plan::Repair { id, cost } = g.plan() else { panic!("a cracked wall should be repairable") };
        assert_eq!(id, WALL);
        assert!(matches!(cost[..], [(Res::Stone, 1)]));
        g.inv = [0; RES_COUNT];
        g.do_build();
        assert!(g.world.damage(1, 0) > 0, "no stone, no repair");
        g.inv[Res::Stone as usize] = 1;
        g.do_build();
        assert_eq!((g.world.damage(1, 0), g.inv[Res::Stone as usize]), (0, 0));
        // A cracked floor with nothing on it is repairable too; natural things never are.
        g.world.set(1, 0, GRASS);
        g.world.set_ground(1, 0, WOOD_FLOOR);
        g.world.hit(1, 0, 1);
        assert!(matches!(g.plan(), Plan::Repair { id: WOOD_FLOOR, .. }));
        g.world.set(1, 0, TREE);
        g.world.hit(1, 0, 1);
        assert!(matches!(g.plan(), Plan::Nothing));
    }

    #[test]
    fn torches_stop_zombies_rising_nearby() {
        assert!(night_spawns(false) > 0);
        assert_eq!(night_spawns(true), 0);
    }

    /// Punches to break (tx, ty) and what it dropped.
    fn break_it(g: &mut Game, tx: i32, ty: i32) -> (u32, [u32; RES_COUNT]) {
        let before = g.inv;
        let mut punches = 0;
        while g.world.get(tx, ty) != GRASS {
            g.damage_tile(tx, ty, true);
            punches += 1;
        }
        (punches, std::array::from_fn(|i| g.inv[i] - before[i]))
    }

    fn with_tools(tools: bool) -> Game {
        let mut g = Game::new(7, None);
        g.inv = [150, 150, 0];
        if tools {
            for (i, t) in TOOLS.iter().enumerate() {
                if t.tier == 1 {
                    assert!(g.craft(i));
                }
            }
            assert_eq!(g.inv, [0; RES_COUNT], "each stone tool costs 50 wood and 50 stone");
        }
        for (x, t) in [(0, crate::content::ROCK), (1, crate::content::TREE), (2, crate::content::WALL)] {
            g.world.set(x, 0, t);
        }
        g
    }

    #[test]
    fn tools_speed_up_and_add_to_natural_drops() {
        let (stone, wood) = (Res::Stone as usize, Res::Wood as usize);
        let mut g = with_tools(false);
        assert_eq!(break_it(&mut g, 0, 0), (3, [2, 0, 0]), "rock by hand");
        assert_eq!(break_it(&mut g, 1, 0), (4, [0, 3, 0]), "tree by hand");
        let mut g = with_tools(true);
        let (p, d) = break_it(&mut g, 0, 0);
        assert_eq!((p, d[stone]), (2, 3), "rock with a pickaxe");
        let (p, d) = break_it(&mut g, 1, 0);
        assert_eq!((p, d[wood]), (2, 4), "tree with an axe");
        // Faster on your own walls, but no bonus refund (no free stone from wall loops).
        let (p, d) = break_it(&mut g, 2, 0);
        assert_eq!((p, d[stone]), (2, 1));
    }

    #[test]
    fn iron_tools_need_the_stone_tool_and_iron() {
        use crate::content::{IRON_ORE, ROCK, TREE};
        let iron_pick = TOOLS.iter().position(|t| t.name == "IRON PICKAXE").unwrap();
        let mut g = Game::new(7, None);
        g.inv = [500, 500, 500];
        assert!(!g.craft(iron_pick), "needs the stone pickaxe first");
        // Bare hands can't mine ore at all.
        g.world.set(0, 0, IRON_ORE);
        for _ in 0..20 {
            g.damage_tile(0, 0, true);
        }
        assert_eq!((g.world.get(0, 0), g.inv[Res::Iron as usize]), (IRON_ORE, 500));
        let mut g = with_tools(true);
        g.inv = [20, 20, 39];
        assert!(!g.craft(iron_pick), "39 iron isn't enough");
        g.inv = [20, 20, 40];
        assert!(g.craft(iron_pick));
        assert_eq!(g.inv, [0, 0, 0]);
        assert_eq!(g.tools[crate::content::Tool::Pick as usize], 2);
        for (x, t) in [(0, ROCK), (1, IRON_ORE), (2, TREE)] {
            g.world.set(x, 0, t);
        }
        assert_eq!(break_it(&mut g, 0, 0), (1, [4, 0, 0]), "rock with an iron pickaxe");
        assert_eq!(break_it(&mut g, 1, 0), (2, [0, 0, 3]), "ore with an iron pickaxe");
        assert_eq!(break_it(&mut g, 2, 0), (2, [0, 4, 0]), "tree still uses the stone axe");
    }

    #[test]
    fn swords_hit_zombies_harder() {
        let hits_to_kill = |sword: u8| {
            let mut g = Game::new(7, None);
            for y in -8..=8 {
                for x in -8..=8 {
                    g.world.set(x, y, GRASS);
                }
            }
            g.tools[Tool::Sword as usize] = sword;
            g.zombies.push(Zombie { x: 4.0, y: 4.0, hp: 6, flash: 0, cooldown: 0, stuck: 0, wander: (0.0, 0.0, 0), left: false });
            (g.x, g.y, g.face) = (g.zombies[0].x - 7.0, g.zombies[0].y, 3); // facing it, close by
            let mut n = 0;
            while g.zombies[0].hp > 0 {
                (g.x, g.y) = (g.zombies[0].x - 7.0, g.zombies[0].y); // follow the knockback
                g.do_punch();
                n += 1;
            }
            n
        };
        assert_eq!((hits_to_kill(0), hits_to_kill(1), hits_to_kill(2)), (6, 3, 2));
    }

    #[test]
    fn tools_are_made_once() {
        let mut g = with_tools(true);
        g.inv = [500; RES_COUNT];
        assert!(!g.craft(0), "already owned");
        assert_eq!(g.inv, [500; RES_COUNT]);
    }

    #[test]
    fn ballista_shoots_on_a_clear_line() {
        let mut g = range(BALLISTA, false);
        g.update_turrets();
        assert_eq!(g.projectiles.len(), 1);
        assert_eq!(g.inv[Res::Wood as usize], 9);
        fly(&mut g);
        assert!(g.zombies[0].hp <= 0, "bolt should hit");
    }

    #[test]
    fn ballista_holds_fire_behind_a_wall() {
        let mut g = range(BALLISTA, true);
        g.update_turrets();
        assert!(g.projectiles.is_empty());
        assert_eq!(g.inv[Res::Wood as usize], 10, "no ammo spent");
    }

    #[test]
    fn catapult_lobs_over_a_wall() {
        let mut g = range(CATAPULT, true);
        g.update_turrets();
        assert_eq!(g.projectiles.len(), 1);
        assert_eq!(g.inv[Res::Stone as usize], 9);
        fly(&mut g);
        assert!(g.zombies[0].hp < 3);
    }

    #[test]
    fn bolt_stops_on_a_wall_built_in_its_path() {
        let mut g = range(BALLISTA, false);
        g.update_turrets();
        g.world.set(2, 0, WALL);
        fly(&mut g);
        assert!(g.projectiles.is_empty());
        assert_eq!(g.zombies[0].hp, 3);
    }
}
