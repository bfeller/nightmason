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
| Tab or Q | Build menu: a full-screen grid (WASD to choose, B or Space to confirm) |
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
- Walking straight into a corner slides you into line with the tile ahead (corner assist).
- A day lasts about 3 minutes: 90 s of daylight, then dusk, about 75 s of night, then dawn.
- Zombies get more dangerous the farther you are from the world spawn, not as the days go by.
  Every 40 tiles is one danger tier (shown under the clock once you're past tier 1). A tier-*n* night
  brings `1 + 2n` zombies (up to 60), and they're faster each tier up to tier 10 and tougher every
  4th tier. Home stays as safe as night 1. They trickle in from off-screen, path-find around rocks
  and trees, and claw through walls, doors and catapults only when that's shorter than walking
  around. Daylight burns them away.
- You have 5 hearts and regenerate during the day. If you die you respawn at the world spawn with
  your inventory; everything you built stays.

## Saving

The world is persistent. It autosaves every 20 s and again on quit (hold Esc, SIGINT or SIGTERM)
to `~/.local/share/nightmason/` (override with `NM_SAVE=<dir>`; `NM_SAVE=` disables saving).

- `world.dat`: a short text file with the seed, clock, player, inventory, stats and the zombies
  that are out.
- `r.<x>.<y>`: region files of 8x8 chunks. Each has a 64-byte presence header followed by 64 fixed
  256-byte slots, written sparsely, so a chunk is one `pread`/`pwrite`. Only chunks you've
  changed are stored. Everything else regenerates from the seed.

## Building

```sh
make cross     # static aarch64-musl binary (Rust's bundled lld, no extra toolchain)
make deploy    # install into the APPLaunch launcher on the device (DEVICE=cardputer)
make           # host build for headless testing
make sprites   # repack the art after editing tools/pack_sprites.py
```

Needs `rustup target add aarch64-unknown-linux-musl`.

## Layout

- `src/world.rs`: infinite world in 16x16 chunks, 1 byte per tile (256 B per chunk, up to 255 tile
  kinds). Chunks within 2 chunks of the player are loaded. Past 3 they're written out if edited
  and dropped, so at most 49 chunks (about 13 KB) are ever resident. `get` pages a chunk in on
  demand, so it's correct anywhere.
- `src/save.rs`: region files and `world.dat`.
- `src/content.rs`: the data tables. `TILES` defines each tile's look, collision for players and
  zombies, durability, punches to break, whether zombies claw through it, and drops. `RESOURCES`
  defines resources and their HUD icons. `RECIPES` defines build-menu entries and their costs.
  Adding a build item takes a sprite, a tile id plus `TILES` entry, and one `RECIPES` line. The
  menu, HUD, costs and drops all follow from the tables.
  Giving a tile a `turret` spec (range, reload, ammo, damage, splash, flight time, fired sprite)
  makes it shoot, so a new tower type is just another table entry.
- `src/game.rs`: player, punching and building, the day/night cycle, the HUD and the build menu.
  Zombies follow a Dijkstra flow field (49x49 tiles around the player, rebuilt twice a second).
  Turrets fire arcing projectiles.
- `src/gfx.rs`: a 160x85 canvas (20x10 tiles of 8x8) scaled 2x to the 320x170 panel, plus
  palette sprites, a 3x5 font, and banded lantern lighting.
- `src/input.rs`: raw evdev reader.
- `assets/sprites.bin`: every sprite packed as one palette-index byte per pixel (21 8x8 blocks, 28 colours,
  1.4 KB). Sprites can span several tiles (the 16x16 trees), generated by `tools/pack_sprites.py` from `assets/src/`.

## Testing without the device

Environment hooks: `NM_FB` (write frames to a file instead of `/dev/fb0`), `NM_INPUT` (evdev
source), `NM_SAVE` (save dir), `NM_SEED` (new worlds only), `NM_TICK` (start time, e.g. 3300 = night 1), `NM_INV` (e.g. `20,10` = stone,wood), `NM_FRAMES` (exit after
N frames). `tools/drive.py` feeds scripted key presses through a FIFO and saves screenshots;
`tools/fbshot.py` turns a framebuffer dump into a PNG. drive.py saves to `<workdir>/save`, so
running it again with the same workdir continues that world. The game starts on the title
screen, so scripts usually begin with `tap enter`. `cargo test` checks chunk paging.

```sh
NM_SEED=7 NM_TICK=2900 NM_INV=20,20 python3 tools/drive.py target/release/nightmason /tmp/nm \
  "wait 5; tap enter; tap s; tap b; tap w; tap b; wait 300; shot night"
```

## Art credits

- Characters: "8x8 Character and Sprite Sheet", OpenGameArt
  (https://opengameart.org/content/8x8-character-and-sprite-sheet)
- Catapult tower: hand-drawn in the Tiny Ranch palette (`tools/draw_catapult.py`), composed from
  a PixelLab sketch
- Tiles, rocks, trees, walls and doors: "Tiny Ranch" by gvituri (https://gvituri.itch.io/tiny-ranch)
