//! The FX chain after the voices (spec 5): DRIVE → CHORUS → DELAY → REVERB.

mod chorus;
mod delay;
mod drive;
mod line;
mod reverb;

pub use chorus::Chorus;
pub use delay::Delay;
pub use drive::Drive;
pub use reverb::Reverb;

use crate::synth::params::FxParams;

const QUIET_IN: f32 = 1e-6;
/// −100 dB.
const QUIET_TAIL: f32 = 1e-5;

fn peak(l: &[f32], r: &[f32]) -> f32 {
    l.iter().chain(r).fold(0.0, |m, v| m.max(v.abs()))
}

pub struct FxChain {
    pub drive: Drive,
    pub chorus: Chorus,
    pub delay: Delay,
    pub reverb: Reverb,
    was_on: [bool; 4],
    tail: [f32; 4],
    idle: [bool; 4],
}

impl FxChain {
    pub fn new(rate: f32) -> FxChain {
        FxChain {
            drive: Drive::new(rate),
            chorus: Chorus::new(rate),
            delay: Delay::new(rate),
            reverb: Reverb::new(rate),
            was_on: [false; 4],
            tail: [0.0; 4],
            idle: [false; 4],
        }
    }

    pub fn clear(&mut self) {
        self.drive.clear();
        self.chorus.clear();
        self.delay.clear();
        self.reverb.clear();
        self.tail = [0.0; 4];
    }

    /// Units skipped in the last block (DRIVE, CHORUS, DELAY, REVERB).
    pub fn idle(&self) -> [bool; 4] {
        self.idle
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams, bpm: f32) {
        let on = [p.drive.on, p.chorus.on, p.delay.on, p.reverb.on];
        for unit in 0..4 {
            if !on[unit] {
                self.was_on[unit] = false;
                self.idle[unit] = false;
                continue;
            }
            if !self.was_on[unit] {
                match unit {
                    0 => self.drive.clear(),
                    1 => self.chorus.clear(),
                    2 => self.delay.clear(),
                    _ => self.reverb.clear(),
                }
                self.was_on[unit] = true;
                self.tail[unit] = 0.0;
            }
            // Output can be silent while a delay still holds an echo (plan K22).
            let pending = match unit {
                0 => self.drive.pending(),
                1 => self.chorus.pending(),
                2 => self.delay.pending(),
                _ => self.reverb.pending(),
            };
            if peak(l, r) < QUIET_IN && self.tail[unit] < QUIET_TAIL && !pending {
                self.idle[unit] = true;
                continue;
            }
            self.idle[unit] = false;
            match unit {
                0 => self.drive.process(l, r, &p.drive),
                1 => self.chorus.process(l, r, &p.chorus),
                2 => self.delay.process(l, r, &p.delay, bpm),
                _ => self.reverb.process(l, r, &p.reverb),
            }
            self.tail[unit] = peak(l, r);
        }
    }
}
