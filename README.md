# Nightmason

A tiny top-down survival-craft game for the M5Stack Cardputer Zero. Punch rocks for stone and trees
for wood, build cobblestone walls, doors and catapult towers, and survive the nights. Every night, more zombies come.

Written in Rust with no dependencies: one ~385 KB static binary that draws straight to `/dev/fb0` and
reads the keyboard from evdev. It uses about 0.5 MB of RAM and 6-10% CPU at a locked 30 fps.

## Controls

| Key | Action |
| --- | --- |
| W A S D, arrow keys, or `;` `,` `.` `/` | Move (tap to turn without stepping) |
| Space, J, or Enter | Punch: rocks, trees, your own buildings, zombies |
| B, K, or E | Build the selected item on the marked tile |
| Tab or Q | Build menu: category tabs over a full-screen grid (Q/E switch tab, WASD choose, B or Space confirm, Tab or Esc close) |
| 1-9 | Select a build item directly |
| P or Esc | Pause menu: resume, new world (asks to confirm), save and quit |
| Hold Esc | Quit (saves) |

The game opens on a title screen with the same menu: Continue (or Play for a new world), New
World, and Save and Quit.

## Rules

- Rocks take 3 punches and drop 2 stone. Trees take 4 punches and drop 3 wood.
- Build items: **wall** (1 stone, 12 zombie hits) and **door** (2 wood, 8 zombie hits). You can walk
  through doors; zombies can't. Punching your own buildings breaks them for a partial refund.
- **Catapult tower** (5 stone, 3 wood): automatically lobs a rock at the nearest zombie within
  7 tiles every ~1.7 s. Each shot costs 1 stone from your inventory, so keep mining. Rocks fly
  in an arc over walls, lead their target, and deal 2 splash damage. With no stone left it sits
  empty and warns you. Looking at a catapult, or getting ready to place one, shows its range
  for a few seconds.
- **Ballista tower** (2 stone, 5 wood): shoots a bolt at the nearest zombie within 9 tiles every
  ~1.3 s for 1 wood a shot. Bolts fly flat and fast and deal 3 damage, so a ballista is the
  harder hitter, but it can't shoot over walls. It only fires at zombies it can see in a straight
  line, and a bolt stops on anything zombies can't walk through (walls, doors, rocks, trees). Build
  it into your wall line, not behind it.
- Walking straight into a corner slides you into line with the tile ahead (corner assist).
- A day lasts about 3 minutes: 90 s of daylight, then dusk, about 75 s of night, then dawn.
- Zombies get more dangerous the farther you are from the world spawn, not as the days go by.
  Every 40 tiles is one danger tier (shown under the clock once you're past tier 1). A tier-*n* night
  brings `1 + 2n` zombies (up to 60), and they're faster each tier up to tier 10 and tougher every
  4th tier. Home stays as safe as night 1. They trickle in from off-screen, path-find around rocks
  and trees, and claw through walls, doors and catapults only when that's shorter than walking
  around. Daylight burns them away. Zombies only exist around you: they rise 12-16 tiles away
  (never near a torch) and are dropped past 40 tiles, and nothing attacks a base you're away from.
- **Torch** (1 wood, 1 stone): lights about 3 tiles around it at night, and no zombie rises
  within 8 tiles of one. Zombies that are already out still walk through the light, so
  torches thin out a night rather than stopping one. Punch it to pick it up.
- **Iron ore**: rocks with rusty specks. There's none within 40 tiles of spawn (danger tier 1); past
  that about 6% of rocks are ore at tier 2, rising ~6% per tier to about 40% at tier 7 and beyond,
  so the iron is out where the zombies are tougher. You need at least a stone pickaxe to mine it
  (fists just clink off); it drops 1 iron, plus the pickaxe's bonus.
- **Toolbench** (10 wood, 5 stone): face it and press B to make tools. A/D picks, B makes,
  Tab/Esc closes. Tools are permanent (you keep them when you die), and there's nothing to switch
  between: your best tool of the right kind is used automatically on whatever you punch, and it
  shows next to your hearts.

  | Tool | Cost | Effect |
  | --- | --- | --- |
  | Stone pickaxe | 50 wood, 50 stone | rocks, walls and towers break 2x faster; +1 from rocks and ore; lets you mine iron ore |
  | Stone axe | 50 wood, 50 stone | trees and wooden things break 2x faster; +1 wood from trees |
  | Iron pickaxe | 40 iron, 20 wood, 20 stone (needs the stone pickaxe) | 4x faster; +2 from rocks and ore |
  | Iron axe | 40 iron, 20 wood, 20 stone (needs the stone axe) | 4x faster; +2 wood from trees |
  | Stone sword | 50 wood, 50 stone | punches hit zombies for 2 instead of 1 |
  | Iron sword | 40 iron, 20 wood, 20 stone (needs the stone sword) | punches hit zombies for 3 |

  Extra drops only come from things that grow in the world, never from breaking your own buildings.
- **Wood floor** (1 wood): laid on the ground under everything else. Anyone can walk on it, and
  you can build walls, beds and towers on top. You can also lay one under something you've
  already built (a torch, a wall, a toolbench), but not under trees or rocks. Punch an empty floor twice to take it up for
  1 wood. Floors live on their own ground layer, so they aren't lost when you build over them.
- **Bed** (4 wood): building one makes it your respawn point (the newest bed wins). You can stand
  on it; zombies can't, and they'll claw it apart (6 hits), which resets your spawn.
- **Repairs**: face a cracked building (or a cracked floor with nothing on it) and press B to mend
  it fully. It costs the building's price scaled by how damaged it is, rounded up, and the
  bottom-right box shows REPAIR and that cost while the target marker turns green. At a cracked
  toolbench you can't afford to fix, B opens the bench instead. When the selected item can't go on
  the tile you're facing (or is still locked), the box shows it crossed out with no cost.
- **Iron wall** (2 iron, 1 stone, 36 zombie hits) and **iron door** (2 iron, 2 wood, 24 zombie
  hits) unlock once you've made any iron tool; until then they show locked in the build menu.
  You can build them outright, or upgrade in place: face a wall (or door) with either the plain or
  the iron version selected, and the box shows UPGRADE and the difference in price (2 iron) with a
  gold target marker. B swaps it for the iron one at full strength, keeping any floor under it.
  A cracked building is repaired first, and the next B upgrades it.
- You have 5 hearts and regenerate during the day. If you die you respawn at your bed, or at the
  world spawn if you have none, with your inventory; everything you built stays.

## Saving

The world is persistent. It autosaves every 20 s and again on quit (hold Esc, SIGINT or SIGTERM)
to `~/.local/share/nightmason/` (override with `NM_SAVE=<dir>`; `NM_SAVE=` disables saving).

- `world.dat`: a short text file with the seed, clock, player, inventory, stats and the zombies
  that are out.
- `r.<x>.<y>` (tiles) and `g.<x>.<y>` (ground layer: floors): region files of 8x8 chunks. Each has a 64-byte presence header followed by 64 fixed
  256-byte slots, written sparsely, so a chunk is one `pread`/`pwrite`. Only chunks you've
  changed are stored. Everything else regenerates from the seed.

## Building

```sh
make cross     # static aarch64-musl binary (Rust's bundled lld, no extra toolchain)
make deploy    # install into the APPLaunch launcher on the device (DEVICE=cardputer)
make           # host build for headless testing
make sprites   # repack the art (needs the source sheets, see Art below)
```

Needs `rustup target add aarch64-unknown-linux-musl`.

## Layout

- `src/world.rs`: infinite world in 16x16 chunks, 1 byte per tile (256 B per chunk, up to 255 tile
  kinds), plus a ground layer for floors that's only allocated in chunks that have one. Chunks within 2 chunks of the player are loaded. Past 3 they're written out if edited
  and dropped, so at most 49 chunks (about 13 KB) are ever resident. `get` pages a chunk in on
  demand, so it's correct anywhere.
- `src/save.rs`: region files and `world.dat`.
- `src/content.rs`: the data tables. `TILES` defines each tile's look, collision for players and
  zombies, durability, punches to break, whether zombies claw through it, and drops. `RESOURCES`
  defines resources and their HUD icons. `RECIPES` defines build-menu entries, their category tab (`CATEGORIES`) and costs. `TOOLS`
  defines what the toolbench makes (speed and yield bonus per tool line), and each tile's `tool`
  says which tool line helps break it.
  Adding a build item takes a sprite, a tile id plus `TILES` entry, and one `RECIPES` line. The
  menu, HUD, costs and drops all follow from the tables.
  Giving a tile a `turret` spec (lobbed or flat, range, reload, ammo, damage, splash, flight
  time, fired sprite) makes it shoot, so a new tower type is just another table entry.
- `src/game.rs`: player, punching and building, the day/night cycle, the HUD and the build menu.
  Zombies follow a Dijkstra flow field (49x49 tiles around the player, rebuilt twice a second).
  Turrets fire arcing projectiles.
- `src/gfx.rs`: a 160x85 canvas (20x10 tiles of 8x8) scaled 2x to the 320x170 panel, plus
  palette sprites, a 3x5 font, and banded lantern lighting.
- `src/input.rs`: raw evdev reader.
- `assets/sprites.bin`: every sprite packed as one palette-index byte per pixel (41 8x8 blocks, 34 colours,
  2 KB). Sprites can span several tiles (the 16x16 trees), generated by `tools/pack_sprites.py` from `assets/src/`.

## Testing without the device

Environment hooks: `NM_FB` (write frames to a file instead of `/dev/fb0`), `NM_INPUT` (evdev
source), `NM_SAVE` (save dir), `NM_SEED` (new worlds only), `NM_TICK` (start time, e.g. 3300 = night 1), `NM_INV` (e.g. `20,10,5` = stone,wood,iron), `NM_FRAMES` (exit after
N frames). `tools/drive.py` feeds scripted key presses through a FIFO and saves screenshots;
`tools/fbshot.py` turns a framebuffer dump into a PNG. drive.py saves to `<workdir>/save`, so
running it again with the same workdir continues that world. The game starts on the title
screen, so scripts usually begin with `tap enter`. `cargo test` checks chunk and floor paging, turret line of sight, torch spawn blocking,
tool effects, swords, and iron ore placement and gating.

```sh
NM_SEED=7 NM_TICK=2900 NM_INV=20,20 python3 tools/drive.py target/release/nightmason /tmp/nm \
  "wait 5; tap enter; tap s; tap b; tap w; tap b; wait 300; shot night"
```

## Art

The repo ships only what the game needs: `assets/sprites.bin` (packed sprites, built into the
binary) and `assets/icon_100.png` (launcher icon). The source sheets aren't redistributed here. To
change the art and run `make sprites`, download them from the links below into `assets/src/`
(git-ignored) under these names:

| File in `assets/src/` | From |
| --- | --- |
| `TinyFarm_Tiles.png`, `TinyFarm_MapDecorations.png`, `TinyFarm_Structures.png` | "Tiny Ranch" by gvituri: https://gvituri.itch.io/tiny-ranch |
| `characters_8x8.png` | "8x8 Character and Sprite Sheet", OpenGameArt: https://opengameart.org/content/8x8-character-and-sprite-sheet |
| `catapult.png`, `ballista.png`, `bed.png`, `floor.png`, `torch.png`, `tools.png`, `ore.png` | Generated by the `tools/draw_*.py` scripts (`make sprites` runs them) |

Credits:

- Characters (player, zombies): "8x8 Character and Sprite Sheet", OpenGameArt
  (https://opengameart.org/content/8x8-character-and-sprite-sheet)
- Tiles, rocks, trees, walls, doors and wood: "Tiny Ranch" by gvituri (https://gvituri.itch.io/tiny-ranch)
- Catapult tower: hand-drawn in the Tiny Ranch palette on a Tiny Ranch wall block
  (`tools/draw_catapult.py`), composed from a PixelLab sketch
- Ballista tower: hand-drawn in the Tiny Ranch palette on a Tiny Ranch wall block
  (`tools/draw_ballista.py`)
- Bed: hand-drawn in the Tiny Ranch palette (`tools/draw_bed.py`)
- Wood floor: hand-drawn in the Tiny Ranch palette (`tools/draw_floor.py`)
- Torch: hand-drawn in the Tiny Ranch palette (`tools/draw_torch.py`)
- Toolbench, pickaxes, axes and swords: hand-drawn in the Tiny Ranch palette (`tools/draw_tools.py`)
- Iron ore: the Tiny Ranch rocks with hand-added rust specks (`tools/draw_ore.py`)
- Launcher icon (`assets/icon_100.png`): made from the character and Tiny Ranch wall sprites above
