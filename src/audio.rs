//! Sound: a tiny sfxr-style synth. Each sound effect is a few `Voice`s of parameters (content.rs
//! `SOUNDS`), rendered live, so sounds cost a few dozen bytes each and no sample data.
//!
//! Output is 16-bit mono PCM piped into one long-lived `pw-cat` (PipeWire owns the codec on the
//! Cardputer Zero). A mixer thread renders blocks paced by the wall clock, a little ahead of
//! real time, so latency stays near LEAD + pw-cat's latency. No PipeWire, no sound; the game
//! never waits on audio. NM_AUDIO=off disables it; NM_AUDIO=<file> writes the raw PCM there.
//!
//! Music uses the same voices: a `Song` (generated into music.rs by tools/pack_music.py) is
//! stepped through inside the mixer, sample-accurately, and each note starts its instrument's
//! voices at the note's pitch.
use std::io::Write;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

pub const RATE: u32 = 24_000;
/// Samples rendered ahead of real time (50 ms).
const LEAD: u64 = RATE as u64 / 20;
/// Samples per block (10 ms).
const BLOCK: usize = RATE as usize / 100;
const MAX_VOICES: usize = 24;
const MASTER: f32 = 0.8;

#[derive(Clone, Copy, PartialEq)]
pub enum Wave {
    Square,
    Saw,
    Tri,
    #[allow(dead_code)] // no sound uses it yet
    Sine,
    /// White noise, re-rolled `freq` times a second: low freq gives rumble, high gives hiss.
    Noise,
}

/// One synth voice. Times are seconds, frequencies Hz, slides octaves per second.
pub struct Voice {
    pub wave: Wave,
    pub freq: f32,
    /// Pitch glide (negative falls).
    pub slide: f32,
    /// At `.0` seconds, multiply the pitch by `.1` (1 = no jump): two-note blips.
    pub jump: (f32, f32),
    /// Square wave duty cycle (0.5 = square, smaller = thinner/nasal).
    pub duty: f32,
    /// Vibrato (rate Hz, depth as a fraction of pitch).
    pub vib: (f32, f32),
    /// Start this long after the sound is triggered (layers, arpeggios).
    pub delay: f32,
    /// Envelope: ramp up, stay at full, then fall to silence.
    pub attack: f32,
    pub hold: f32,
    pub decay: f32,
    /// One-pole low-pass cutoff (0 = off) and its sweep.
    pub lp: f32,
    pub lp_slide: f32,
    /// One-pole high-pass cutoff (0 = off).
    pub hp: f32,
    pub vol: f32,
}

/// Defaults for new voices: `Voice { wave: Wave::Noise, decay: 0.1, ..VOICE }`.
pub const VOICE: Voice = Voice {
    wave: Wave::Square, freq: 440.0, slide: 0.0, jump: (0.0, 1.0), duty: 0.5, vib: (0.0, 0.0),
    delay: 0.0, attack: 0.0, hold: 0.0, decay: 0.1, lp: 0.0, lp_slide: 0.0, hp: 0.0, vol: 0.5,
};

/// A sound effect: its voices start together (offset by their `delay`).
pub type Sfx = &'static [Voice];

pub fn same(a: Sfx, b: Sfx) -> bool {
    std::ptr::eq(a, b)
}

impl Voice {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn length(&self) -> f32 {
        self.delay + self.attack + self.hold + self.decay
    }
}

struct Playing {
    v: &'static Voice,
    gain: f32,
    /// Pitch multiplier (music notes; 1 for sound effects).
    ratio: f32,
    /// Extra hold in seconds (gated music notes last as long as the note).
    extra: f32,
    /// Part of the music (its volume is the music volume, not the sound volume).
    music: bool,
    /// Samples since the sound was triggered.
    n: u32,
    phase: f32,
    noise: f32,
    rng: u32,
    lp: f32,
    hp: f32,
    hp_in: f32,
}

impl Playing {
    /// Next sample, or None once the voice has finished.
    fn next(&mut self) -> Option<f32> {
        let v = self.v;
        let dt = 1.0 / RATE as f32;
        let t = self.n as f32 * dt - v.delay;
        self.n += 1;
        if t < 0.0 {
            return Some(0.0);
        }
        let hold = v.hold + self.extra;
        let env = if t < v.attack {
            t / v.attack
        } else if t < v.attack + hold {
            1.0
        } else {
            let d = (t - v.attack - hold) / v.decay.max(dt);
            if d >= 1.0 {
                return None;
            }
            (1.0 - d) * (1.0 - d)
        };
        let mut f = v.freq * self.ratio * (v.slide * t).exp2();
        if t >= v.jump.0 {
            f *= v.jump.1;
        }
        if v.vib.1 != 0.0 {
            f *= 1.0 + v.vib.1 * (std::f32::consts::TAU * v.vib.0 * t).sin();
        }
        self.phase += f.clamp(1.0, RATE as f32 / 2.0) * dt;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 17;
            self.rng ^= self.rng << 5;
            self.noise = (self.rng >> 8) as f32 / (1u32 << 23) as f32 - 1.0;
        }
        let p = self.phase;
        let mut x = match v.wave {
            Wave::Square => if p < v.duty { 0.6 } else { -0.6 },
            Wave::Saw => 1.2 * p - 0.6,
            Wave::Tri => if p < 0.5 { 4.0 * p - 1.0 } else { 3.0 - 4.0 * p },
            Wave::Sine => (std::f32::consts::TAU * p).sin(),
            Wave::Noise => self.noise,
        };
        if v.lp > 0.0 {
            let c = (v.lp * (v.lp_slide * t).exp2()).clamp(20.0, RATE as f32 * 0.45);
            self.lp += (1.0 - (-std::f32::consts::TAU * c * dt).exp()) * (x - self.lp);
            x = self.lp;
        }
        if v.hp > 0.0 {
            let rc = 1.0 / (std::f32::consts::TAU * v.hp);
            self.hp = rc / (rc + dt) * (self.hp + x - self.hp_in);
            self.hp_in = x;
            x = self.hp;
        }
        Some(x * env * v.vol * self.gain)
    }
}

/// One voice of a music instrument.
pub struct InstVoice {
    pub v: Voice,
    /// Ignore the note's pitch (drums); otherwise `v.freq` is the pitch when the note is A4.
    pub fixed: bool,
    /// Hold for as long as the note lasts (then `decay` is the release); otherwise a one-shot.
    pub gate: bool,
}
pub type Inst = &'static [InstVoice];

pub struct Note {
    /// Step within the pattern (16th notes).
    pub step: u8,
    /// MIDI note number (69 = A4).
    pub key: u8,
    /// Length in steps.
    pub len: u8,
    /// Velocity, 1-255.
    pub vel: u8,
}

pub struct Track {
    pub inst: Inst,
    pub vol: f32,
}

pub struct Song {
    pub bpm: f32,
    /// Steps per pattern.
    pub steps: u8,
    pub volume: f32,
    pub tracks: &'static [Track],
    /// Each pattern's notes, one list per track, sorted by step.
    pub patterns: &'static [&'static [&'static [Note]]],
    /// Patterns in play order; the song loops.
    pub order: &'static [u8],
}

struct Music {
    song: &'static Song,
    /// Mixer clock when the song started, and steps triggered since.
    start: u64,
    step: u64,
}

/// Mixes the voices that are playing.
pub struct Mixer {
    playing: Vec<Playing>,
    seed: u32,
    music: Option<Music>,
    /// Samples rendered so far.
    clock: u64,
    /// Sound-effect and music volumes (gains, 1 = full).
    sfx_vol: f32,
    music_vol: f32,
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer { playing: Vec::new(), seed: 0x9e37_79b9, music: None, clock: 0, sfx_vol: 1.0, music_vol: 1.0 }
    }

    pub fn start(&mut self, s: Sfx, gain: f32) {
        for v in s {
            self.voice(v, gain, 1.0, 0.0, false);
        }
    }

    fn voice(&mut self, v: &'static Voice, gain: f32, ratio: f32, extra: f32, music: bool) {
        if self.playing.len() >= MAX_VOICES {
            self.playing.remove(0); // oldest goes first
        }
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.playing.push(Playing {
            v, gain, ratio, extra, music, n: 0, phase: 0.0, noise: 0.0, rng: self.seed | 1, lp: 0.0, hp: 0.0, hp_in: 0.0,
        });
    }

    /// Start a song from the top (None stops it; notes already sounding finish their release).
    pub fn music(&mut self, song: Option<&'static Song>) {
        self.music = song.filter(|s| !s.order.is_empty() && s.steps > 0).map(|song| Music { song, start: self.clock, step: 0 });
    }

    /// Start the notes of every step that's due by now.
    fn sequence(&mut self) {
        let Some(m) = &mut self.music else { return };
        let song = m.song;
        let step_samples = RATE as f64 * 60.0 / song.bpm as f64 / 4.0;
        let due = m.start + (m.step as f64 * step_samples).round() as u64;
        if self.clock < due {
            return;
        }
        let steps = song.steps as u64;
        let pat = song.patterns[song.order[(m.step / steps % song.order.len() as u64) as usize] as usize];
        let at = (m.step % steps) as u8;
        m.step += 1;
        let step_secs = step_samples as f32 / RATE as f32;
        for (track, notes) in song.tracks.iter().zip(pat.iter()) {
            for n in notes.iter().filter(|n| n.step == at) {
                let ratio = ((n.key as f32 - 69.0) / 12.0).exp2();
                let gain = track.vol * n.vel as f32 / 255.0 * song.volume;
                for iv in track.inst {
                    let extra = if iv.gate { n.len as f32 * step_secs } else { 0.0 };
                    self.voice(&iv.v, gain, if iv.fixed { 1.0 } else { ratio }, extra, true);
                }
            }
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn idle(&self) -> bool {
        self.playing.is_empty()
    }

    pub fn render(&mut self, out: &mut [i16]) {
        for s in out.iter_mut() {
            self.sequence();
            self.clock += 1;
            let mut x = 0.0;
            let (sv, mv) = (self.sfx_vol, self.music_vol);
            self.playing.retain_mut(|p| p.next().map(|y| x += y * if p.music { mv } else { sv }).is_some());
            // Soft clip so stacked sounds squash instead of wrapping.
            *s = ((x * MASTER).tanh() * 32_000.0) as i16;
        }
    }
}

enum Msg {
    Sfx(Sfx, f32),
    Music(Option<&'static Song>),
    Volume(f32, f32),
}

/// Handle the game keeps; sounds are sent to the mixer thread.
pub struct Audio {
    tx: Option<Sender<Msg>>,
}

extern "C" {
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
}
const F_SETPIPE_SZ: i32 = 1031;

impl Audio {
    pub fn open() -> Audio {
        let target = std::env::var("NM_AUDIO").unwrap_or_default();
        let out: Box<dyn Write + Send> = match target.as_str() {
            "off" | "0" => return Audio { tx: None },
            "" => {
                let child = std::process::Command::new("pw-cat")
                    .args(["--playback", "--raw", "--format", "s16", "--channels", "1"])
                    .args(["--rate", &RATE.to_string(), "--latency", "20ms", "-"])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                let Ok(mut child) = child else { return Audio { tx: None } };
                let stdin = child.stdin.take().expect("piped stdin");
                // A small pipe caps how far audio can queue up if pw-cat's clock runs slow.
                unsafe {
                    use std::os::fd::AsRawFd;
                    fcntl(stdin.as_raw_fd(), F_SETPIPE_SZ, 4096);
                }
                Box::new(stdin)
            }
            path => match std::fs::File::create(path) {
                Ok(f) => Box::new(f),
                Err(e) => {
                    eprintln!("nightmason: NM_AUDIO {path}: {e}");
                    return Audio { tx: None };
                }
            },
        };
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new().name("audio".into()).spawn(move || run(rx, out));
        Audio { tx: spawned.ok().map(|_| tx) }
    }

    pub fn play(&mut self, s: Sfx, gain: f32) {
        self.send(Msg::Sfx(s, gain));
    }

    /// Start a song from the top, or stop the music with None.
    pub fn music(&mut self, song: Option<&'static Song>) {
        self.send(Msg::Music(song));
    }

    /// Sound-effect and music volumes, as gains (applies to what's already playing too).
    pub fn volume(&mut self, sfx: f32, music: f32) {
        self.send(Msg::Volume(sfx, music));
    }

    fn send(&mut self, m: Msg) {
        if let Some(tx) = &self.tx {
            if tx.send(m).is_err() {
                self.tx = None; // the output went away
            }
        }
    }
}

fn run(rx: Receiver<Msg>, mut out: Box<dyn Write + Send>) {
    let mut mix = Mixer::new();
    let mut buf = vec![0i16; BLOCK];
    let mut bytes = vec![0u8; BLOCK * 2];
    let start = Instant::now();
    let mut written: u64 = 0;
    loop {
        loop {
            match rx.try_recv() {
                Ok(Msg::Sfx(s, g)) => mix.start(s, g),
                Ok(Msg::Music(song)) => mix.music(song),
                Ok(Msg::Volume(s, m)) => (mix.sfx_vol, mix.music_vol) = (s, m),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        let due = (start.elapsed().as_secs_f64() * RATE as f64) as u64 + LEAD;
        if written >= due {
            std::thread::sleep(Duration::from_millis(4));
            continue;
        }
        // Fell far behind (stalled, suspended): skip ahead rather than play catch-up.
        if due - written > RATE as u64 / 4 {
            written = due - LEAD;
        }
        mix.render(&mut buf);
        for (b, s) in bytes.chunks_exact_mut(2).zip(&buf) {
            b.copy_from_slice(&s.to_le_bytes());
        }
        if out.write_all(&bytes).is_err() {
            return; // pw-cat gone: play on silently
        }
        written += BLOCK as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::SOUNDS;

    #[test]
    fn title_music_plays_its_notes_and_stops_cleanly() {
        let song = crate::content::TITLE_MUSIC;
        let mut m = Mixer::new();
        m.music(Some(song));
        // One full loop of the arrangement, plus a little.
        let secs = song.order.len() as f32 * song.steps as f32 * 15.0 / song.bpm;
        let mut buf = vec![0i16; (secs * RATE as f32) as usize + RATE as usize];
        m.render(&mut buf);
        let peak = buf.iter().map(|x| x.unsigned_abs()).max().unwrap();
        assert!(peak > 3000, "title music is nearly silent (peak {peak})");
        let quiet = buf.chunks(RATE as usize).filter(|c| c.iter().all(|x| x.unsigned_abs() < 200)).count();
        assert_eq!(quiet, 0, "a whole second of the title music is silent");
        assert!(m.playing.len() <= MAX_VOICES);
        // Stopped: what's sounding finishes and nothing new starts.
        m.music(None);
        let mut tail = vec![0i16; RATE as usize * 3];
        m.render(&mut tail);
        assert!(m.idle(), "voices still sounding 3 s after the music stopped");
    }

    #[test]
    fn every_sound_is_audible_short_and_finishes() {
        for &(name, s) in SOUNDS {
            let len = s.iter().map(|v| v.length()).fold(0.0, f32::max);
            assert!(len > 0.0 && len <= 1.5, "{name}: {len}s long");
            let mut m = Mixer::new();
            m.start(s, 1.0);
            let mut buf = vec![0i16; (len * RATE as f32) as usize + BLOCK];
            m.render(&mut buf);
            assert!(m.idle(), "{name} still playing after {len}s");
            let peak = buf.iter().map(|x| x.unsigned_abs()).max().unwrap();
            assert!(peak > 3000, "{name} is nearly silent (peak {peak})");
        }
    }
}
