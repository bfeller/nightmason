//! Nightmason: punch rocks, build walls, survive the nights.
//! Renders straight to /dev/fb0, reads the keyboard from evdev and plays sound through pw-cat;
//! no dependencies.
mod audio;
mod content;
mod game;
mod gfx;
mod input;
mod music;
mod save;
mod settings;
mod sprites;
mod world;

use std::fs::OpenOptions;
use std::os::unix::fs::FileExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" {
    fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
}
extern "C" fn on_signal(_: i32) {
    STOP.store(true, Ordering::Relaxed);
}

fn env<T: std::str::FromStr>(k: &str) -> Option<T> {
    std::env::var(k).ok()?.parse().ok()
}

fn main() {
    unsafe {
        signal(2, on_signal); // SIGINT
        signal(15, on_signal); // SIGTERM
    }
    // NM_FB / NM_INPUT / NM_SAVE / NM_SEED / NM_TICK / NM_INV / NM_FRAMES / NM_AUDIO are test hooks.
    let fb_path = std::env::var("NM_FB").unwrap_or_else(|_| "/dev/fb0".into());
    let fb = match OpenOptions::new().write(true).create(fb_path != "/dev/fb0").open(&fb_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("nightmason: cannot open {fb_path}: {e}");
            std::process::exit(1);
        }
    };
    let mut input = input::Input::open();
    let seed = env("NM_SEED").unwrap_or_else(|| {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u32).unwrap_or(1)
    });
    let store = save::default_dir().and_then(|d| match save::Store::open(d.clone()) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("nightmason: cannot use save dir {}: {e} (progress won't be kept)", d.display());
            None
        }
    });
    let mut game = game::Game::open(store, seed); // NM_SEED only applies to a new world
    game.set_settings(settings::Settings::load(save::default_dir()));
    if let Some(t) = env("NM_TICK") {
        game.tick = t;
    }
    if let Ok(v) = std::env::var("NM_INV") {
        for (slot, n) in game.inv.iter_mut().zip(v.split(',')) {
            *slot = n.parse().unwrap_or(0);
        }
    }
    let max_frames: u64 = env("NM_FRAMES").unwrap_or(u64::MAX);

    let mut audio = audio::Audio::open();
    let mut song: Option<&'static audio::Song> = None;
    let mut volume = (u8::MAX, u8::MAX);
    let mut canvas = gfx::Canvas::new();
    let mut out = vec![0u8; gfx::FB_W * gfx::FB_H * 2];
    let frame = Duration::from_micros(1_000_000 / 30);
    let mut next = Instant::now();
    for _ in 0..max_frames {
        if STOP.load(Ordering::Relaxed) {
            break;
        }
        input.poll();
        if !game.update(&input) {
            break;
        }
        input.end_frame();
        for (s, gain) in game.sounds.drain(..) {
            audio.play(s, gain);
        }
        if (game.settings.sound, game.settings.music) != volume {
            volume = (game.settings.sound, game.settings.music);
            audio.volume(settings::Settings::gain(volume.0), settings::Settings::gain(volume.1));
        }
        let want = game.music();
        if want.map(|s| s as *const _) != song.map(|s| s as *const _) {
            song = want;
            audio.music(song);
        }
        game.render(&mut canvas);
        canvas.present(&mut out);
        let _ = fb.write_at(&out, 0);
        next += frame;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
    game.save();
    if max_frames == u64::MAX {
        out.fill(0);
        let _ = fb.write_at(&out, 0);
    }
}
