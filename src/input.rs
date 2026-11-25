//! Raw evdev keyboard reader (no deps). Tracks held keys and keys pressed this frame.
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;

const O_NONBLOCK: i32 = 0o4000;
const EV_SIZE: usize = 24; // struct input_event on 64-bit Linux

pub struct Input {
    f: Option<File>,
    carry: Vec<u8>,
    down: [bool; 256],
    hit: [bool; 256],
}

impl Input {
    pub fn open() -> Input {
        let env = std::env::var("NM_INPUT").ok();
        let paths: Vec<&str> = match &env {
            Some(p) => vec![p.as_str()],
            None => vec![
                "/dev/input/by-path/platform-3f804000.i2c-event",
                "/dev/input/cardputer-zero-internal",
                "/dev/input/event3",
            ],
        };
        let f = paths
            .iter()
            .find_map(|p| OpenOptions::new().read(true).custom_flags(O_NONBLOCK).open(p).ok());
        Input { f, carry: Vec::new(), down: [false; 256], hit: [false; 256] }
    }

    pub fn poll(&mut self) {
        let Some(f) = &mut self.f else { return };
        let mut buf = [0u8; EV_SIZE * 16];
        while let Ok(n) = f.read(&mut buf) {
            if n == 0 {
                break;
            }
            self.carry.extend_from_slice(&buf[..n]);
        }
        let whole = self.carry.len() / EV_SIZE * EV_SIZE;
        for ev in self.carry[..whole].chunks_exact(EV_SIZE) {
            let ty = u16::from_le_bytes([ev[16], ev[17]]);
            let code = u16::from_le_bytes([ev[18], ev[19]]) as usize;
            let val = i32::from_le_bytes([ev[20], ev[21], ev[22], ev[23]]);
            if ty != 1 || code >= 256 {
                continue;
            }
            self.down[code] = val != 0;
            if val == 1 {
                self.hit[code] = true;
            }
        }
        self.carry.drain(..whole);
    }

    pub fn end_frame(&mut self) {
        self.hit = [false; 256];
    }
    /// A fake input with `key` just pressed (tests).
    #[cfg(test)]
    pub fn tap(key: u16) -> Input {
        let mut i = Input { f: None, carry: Vec::new(), down: [false; 256], hit: [false; 256] };
        (i.down[key as usize], i.hit[key as usize]) = (true, true);
        i
    }

    pub fn held(&self, keys: &[u16]) -> bool {
        keys.iter().any(|&k| self.down[k as usize])
    }
    pub fn pressed(&self, keys: &[u16]) -> bool {
        keys.iter().any(|&k| self.hit[k as usize])
    }
}
