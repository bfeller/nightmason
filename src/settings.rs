//! Player settings: volumes and key bindings, kept in `settings.dat` in the save directory
//! (`key values` lines, like world.dat; starting a new world leaves it alone).
//!
//! Each action has keys the player chooses plus `locked` keys that always work and can't be
//! rebound (the arrows, Enter, Tab, Esc), so the menus can never become unreachable.
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Punch,
    Build,
    Menu,
    Pause,
}

pub struct ActionDef {
    pub action: Action,
    /// Shown on the settings page.
    pub name: &'static str,
    /// Saved under `key.<id>`.
    pub id: &'static str,
    pub default: &'static [u16],
    pub locked: &'static [u16],
}

// Linux key codes. The Cardputer's ; . , / keys double as arrows.
pub const K_ESC: u16 = 1;
pub const K_TAB: u16 = 15;
pub const K_ENTER: u16 = 28;
pub const K_Q: u16 = 16;
pub const K_E: u16 = 18;

/// In `Action` order.
pub const ACTIONS: &[ActionDef] = &[
    ActionDef { action: Action::Up, name: "UP", id: "up", default: &[17], locked: &[103, 39] },
    ActionDef { action: Action::Down, name: "DOWN", id: "down", default: &[31], locked: &[108, 52] },
    ActionDef { action: Action::Left, name: "LEFT", id: "left", default: &[30], locked: &[105, 51] },
    ActionDef { action: Action::Right, name: "RIGHT", id: "right", default: &[32], locked: &[106, 53] },
    ActionDef { action: Action::Punch, name: "PUNCH", id: "punch", default: &[57, 36], locked: &[K_ENTER] },
    ActionDef { action: Action::Build, name: "BUILD", id: "build", default: &[48, 37, 18], locked: &[] },
    ActionDef { action: Action::Menu, name: "BUILD MENU", id: "menu", default: &[K_Q], locked: &[K_TAB] },
    ActionDef { action: Action::Pause, name: "PAUSE", id: "pause", default: &[25], locked: &[K_ESC] },
];
const N: usize = 8;

/// Volumes run 0..=MAX_VOL.
pub const MAX_VOL: u8 = 10;

/// Keys that can't be bound: every locked key, and 1-9/0 (they pick build items).
pub fn reserved(k: u16) -> bool {
    (2..=11).contains(&k) || ACTIONS.iter().any(|a| a.locked.contains(&k))
}

/// A key's name in the 3x5 font.
pub fn key_name(k: u16) -> String {
    const LETTERS: &[(u16, &str)] = &[
        (1, "ESC"), (12, "-"), (14, "BKSP"), (15, "TAB"), (28, "ENTER"), (29, "CTRL"), (39, "SEMI"),
        (40, "QUOTE"), (42, "SHIFT"), (51, ","), (52, "."), (53, "/"), (54, "RSHIFT"), (56, "ALT"),
        (57, "SPACE"), (103, "UP"), (105, "LEFT"), (106, "RIGHT"), (108, "DOWN"), (111, "DEL"),
    ];
    const ROWS: [(u16, &str); 4] = [(2, "1234567890"), (16, "QWERTYUIOP"), (30, "ASDFGHJKL"), (44, "ZXCVBNM")];
    if let Some((_, n)) = LETTERS.iter().find(|(c, _)| *c == k) {
        return n.to_string();
    }
    for (start, row) in ROWS {
        if let Some(c) = row.chars().nth(k.wrapping_sub(start) as usize).filter(|_| k >= start) {
            return c.to_string();
        }
    }
    format!("#{k}")
}

pub struct Settings {
    pub sound: u8,
    pub music: u8,
    /// Chosen keys per action, in `ACTIONS` order (locked keys come on top).
    pub binds: [Vec<u16>; N],
    path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            sound: MAX_VOL,
            music: MAX_VOL,
            binds: std::array::from_fn(|i| ACTIONS[i].default.to_vec()),
            path: None,
        }
    }
}

impl Settings {
    /// Load from `dir/settings.dat` (defaults for anything missing); saves go back there.
    pub fn load(dir: Option<PathBuf>) -> Settings {
        let path = dir.map(|d| d.join("settings.dat"));
        let mut s = Settings { path: path.clone(), ..Settings::default() };
        let Some(text) = path.and_then(|p| std::fs::read_to_string(p).ok()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once(' ') else { continue };
            let nums: Vec<u16> = v.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            match k {
                "sound" => s.sound = nums.first().map_or(MAX_VOL, |&n| (n as u8).min(MAX_VOL)),
                "music" => s.music = nums.first().map_or(MAX_VOL, |&n| (n as u8).min(MAX_VOL)),
                _ => {
                    let Some(id) = k.strip_prefix("key.") else { continue };
                    let Some(i) = ACTIONS.iter().position(|a| a.id == id) else { continue };
                    s.binds[i] = nums.into_iter().filter(|&n| n < 256 && !reserved(n)).collect();
                }
            }
        }
        s
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else { return Ok(()) };
        let mut out = format!("sound {}\nmusic {}\n", self.sound, self.music);
        for (a, keys) in ACTIONS.iter().zip(&self.binds) {
            let keys: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
            out += &format!("key.{} {}\n", a.id, keys.join(" "));
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, out)?;
        std::fs::rename(tmp, path)
    }

    /// Every key that triggers `a`: the chosen ones, then the locked ones.
    pub fn keys(&self, a: Action) -> Vec<u16> {
        let i = a as usize;
        self.binds[i].iter().chain(ACTIONS[i].locked).copied().collect()
    }

    /// The key to name in on-screen hints for `a`.
    pub fn hint(&self, a: Action) -> String {
        key_name(self.keys(a).first().copied().unwrap_or(0))
    }

    /// Make `key` the key for `a`. If another action had it, that action gets `a`'s old key
    /// instead (a swap). Returns false for keys that can't be bound.
    pub fn bind(&mut self, a: Action, key: u16) -> bool {
        if reserved(key) {
            return false;
        }
        let i = a as usize;
        let old = self.binds[i].first().copied();
        for (j, b) in self.binds.iter_mut().enumerate() {
            if j != i && b.contains(&key) {
                b.retain(|&k| k != key);
                if let Some(o) = old.filter(|o| !b.contains(o)) {
                    b.insert(0, o);
                }
            }
        }
        self.binds[i] = vec![key];
        true
    }

    pub fn reset_keys(&mut self) {
        self.binds = std::array::from_fn(|i| ACTIONS[i].default.to_vec());
    }

    /// Volume as a gain; squared so each step sounds about even.
    pub fn gain(v: u8) -> f32 {
        let x = v as f32 / MAX_VOL as f32;
        x * x
    }
}

/// Every key list the game reads, rebuilt whenever the bindings change.
pub struct Keys {
    pub up: Vec<u16>,
    pub down: Vec<u16>,
    pub left: Vec<u16>,
    pub right: Vec<u16>,
    pub punch: Vec<u16>,
    pub build: Vec<u16>,
    pub menu: Vec<u16>,
    pub pause: Vec<u16>,
    /// Confirm in menus: punch or build, except Q/E (they switch build-menu tabs).
    pub ok: Vec<u16>,
    /// Close the build menu: the build-menu key or Esc, except Q/E.
    pub close: Vec<u16>,
}

impl Keys {
    pub fn new(s: &Settings) -> Keys {
        let tabs = |v: Vec<u16>| v.into_iter().filter(|k| *k != K_Q && *k != K_E).collect::<Vec<_>>();
        let ok = tabs([s.keys(Action::Punch), s.keys(Action::Build)].concat());
        Keys {
            up: s.keys(Action::Up),
            down: s.keys(Action::Down),
            left: s.keys(Action::Left),
            right: s.keys(Action::Right),
            punch: s.keys(Action::Punch),
            build: s.keys(Action::Build),
            menu: s.keys(Action::Menu),
            pause: s.keys(Action::Pause),
            ok,
            close: tabs([s.keys(Action::Menu), vec![K_ESC]].concat()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_original_controls() {
        let k = Keys::new(&Settings::default());
        assert_eq!(k.up, [17, 103, 39]);
        assert_eq!(k.punch, [57, 36, 28]);
        assert_eq!(k.build, [48, 37, 18]);
        assert_eq!(k.menu, [16, 15]);
        assert_eq!(k.pause, [25, 1]);
        assert_eq!(k.ok, [57, 36, 28, 48, 37]);
        assert_eq!(k.close, [15, 1]);
    }

    #[test]
    fn binding_a_taken_key_swaps_and_reserved_keys_are_refused() {
        let mut s = Settings::default();
        assert!(s.bind(Action::Punch, 48)); // B, the build key
        assert_eq!(s.binds[Action::Punch as usize], [48]);
        assert_eq!(s.binds[Action::Build as usize], [57, 37, 18]); // gets Space back
        assert!(!s.bind(Action::Up, 28)); // Enter is locked to punch
        assert!(!s.bind(Action::Up, 3)); // number keys pick build items
        assert_eq!(s.keys(Action::Up), [17, 103, 39]);
    }

    #[test]
    fn settings_survive_a_save_and_load() {
        let dir = std::env::temp_dir().join(format!("nm-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Settings::load(Some(dir.clone()));
        (s.sound, s.music) = (3, 7);
        s.bind(Action::Pause, 19);
        s.save().unwrap();
        let t = Settings::load(Some(dir.clone()));
        assert_eq!((t.sound, t.music), (3, 7));
        assert_eq!(t.keys(Action::Pause), [19, 1]);
        assert_eq!(t.binds[Action::Up as usize], [17]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn every_key_name_draws_in_the_font() {
        for k in 1..256u16 {
            let n = key_name(k);
            assert!(n.chars().all(|c| c.is_ascii_alphanumeric() || "-,./#".contains(c)), "{k}: {n}");
        }
    }
}
