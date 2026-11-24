# Nightmason: notes for coding sessions

Rust survival-craft game for the Cardputer Zero (320x170 RGB565 fb, stride 640; keyboard evdev).
Sibling of ~/projects/lookout and ~/projects/beamhound, but pure Rust with no crates.

- Never compile on the device (it has about 350 MB RAM). `make cross` builds static aarch64-musl
  with rust-lld; `make deploy` installs into /usr/share/APPLaunch and restarts APPLaunch.
- Manual on-device run: `systemctl --user stop APPLaunch`, run the binary, `cat /dev/fb0` for a
  screenshot (tools/fbshot.py), then restart APPLaunch. With NM_FRAMES set, the last frame stays on
  screen.
- Host testing: tools/drive.py (FIFO key script + screenshots). Test hooks are in the README.
- Art: edit SPRITES in tools/pack_sprites.py, then run `make sprites`. That regenerates
  src/sprites.rs; don't hand-edit it.
- Content is data-driven in src/content.rs (TILES indexed by u8 tile id, RESOURCES, RECIPES). Add
  things there rather than special-casing them in game.rs. Chunks are 16x16.
- Two map layers: tiles (`World::get/set`, what stands on a tile) and ground (`ground/set_ground`,
  floors with `Look::Floor`, drawn in the ground pass). Floors never block and are saved in their
  own `g.*` region files.
- Tools are permanent per-line upgrades (`Game::tools`, best tier owned per `Tool` kind), made at a
  toolbench and applied automatically via each TileDef's `tool`. Yield bonuses must never apply to
  buildable tiles (`content::buildable`), or build-and-break loops would mint resources.
- `world::generate` defines every chunk nobody has edited, so changing it reshapes the untouched
  parts of existing saves (edited chunks are stored whole and don't change). `tier_at` (danger
  tier by distance) drives both zombie toughness and how much rock is iron ore.
- The world is persistent (src/save.rs). Tile ids are stored on disk, so append new TILES entries
  and never renumber or reuse old ids. world.dat is `key values` lines: add keys freely, and bump
  VERSION only for incompatible changes. Edited chunks are written out and evicted like any other,
  and turrets are re-discovered from loaded chunks in Game::maintain.
- Rendering is row-ordered: ground, then for each row its tile objects followed by the creatures
  whose feet are in that row. Look::Big sprites (trees, catapult tower) overhang the rows above. Turrets: a `turret` spec on a TileDef plus runtime TurretState/Projectile in game.rs.
