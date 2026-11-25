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
  toolbench and applied automatically via each TileDef's `tool` (`min_tier` gates breaking it at
  all, e.g. ore needs a pickaxe). The sword line's `speed` is punch damage on zombies. Yield bonuses must never apply to
  buildable tiles (`content::buildable`), or build-and-break loops would mint resources.
- B at the target is decided by `Game::plan` (repair, then upgrade, then build), which the HUD
  box and target marker also read. Upgrades are data: a Recipe's `upgrades` names the tile it
  replaces for the cost difference, and `tier` locks it until you own a tool of that tier. A
  building's drops must never exceed its recipe cost (a test checks every recipe).
- `world::generate` defines every chunk nobody has edited, so changing it reshapes the untouched
  parts of existing saves (edited chunks are stored whole and don't change). `tier_at` (danger
  tier by distance) drives both zombie toughness and how much rock is iron ore (tier 2+) and
  silver ore (tier 3+).
- The world is persistent (src/save.rs). Tile ids are stored on disk, so append new TILES entries
  and never renumber or reuse old ids. world.dat is `key values` lines: add keys freely, and bump
  VERSION only for incompatible changes. Edited chunks are written out and evicted like any other,
  and turrets are re-discovered from loaded chunks in Game::maintain.
- Rendering is row-ordered: ground, then for each row its tile objects followed by the creatures
  whose feet are in that row. Look::Big sprites (trees, catapult tower) overhang the rows above. Turrets: a `turret` spec on a TileDef plus runtime TurretState/Projectile in game.rs.
- Controls: game.rs never hard-codes action keys; it reads `Game::keys` (settings.rs `Keys`, rebuilt
  from the player's bindings). Each action in `settings::ACTIONS` has rebindable default keys plus
  locked ones (arrows, Enter, Tab, Esc) that keep the menus reachable. Settings (volumes, binds)
  live in settings.dat beside the world, which `Store::wipe` leaves alone.
- Sound: src/audio.rs synthesizes sfxr-style voices and pipes 24 kHz s16 mono into `pw-cat`
  (PipeWire owns the ES8389 codec; there's no hardware synth). Sounds are data (`content::sfx`,
  listed in `SOUNDS`); game.rs queues them in `Game::sounds` via `sound`/`sound_at` and main.rs
  plays them. Keep new sounds short (a test caps them at 1.5 s). tools/synth.js is a JS copy of
  the synth (f32 rounding included) shared by the sound and music designer pages: change it
  together with audio.rs. Designs the user saves land in tools/sfx_designs.json; new sounds there
  need a `SOUNDS` entry and a hook.
- Music: tools/music_designer.html saves tools/music_designs.json, and `make music`
  (tools/pack_music.py) generates src/music.rs from it; don't hand-edit that. Only the title
  screen plays music (`content::TITLE_MUSIC`, via `Game::music`). The JSON has
  instruments (Voices plus `fixed`: ignore the note, `gate`: hold for the note's length; a pitched
  voice's freq is its pitch at A4) and songs (bpm, steps per pattern, tracks with an instrument,
  patterns of {s step, n MIDI note, l length in steps, v velocity} per track, and `order`).
