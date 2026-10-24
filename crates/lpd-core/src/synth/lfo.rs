//! LFOs (spec §3.8) at control rate.

use super::osc::next_bipolar;
use super::params::{LfoMode, LfoParams, LfoShape, Polarity};
use std::f32::consts::{PI, TAU};

/// The deterministic shapes at `phase` 0–1, in −1…1 (the random shapes live in `Lfo`).
pub fn shape_value(shape: LfoShape, phase: f32) -> f32 {
    let p = phase.rem_euclid(1.0);
    match shape {
        LfoShape::Sine => (TAU * p).sin(),
        LfoShape::Triangle => {
            if p < 0.25 {
                4.0 * p
            } else if p < 0.75 {
                2.0 - 4.0 * p
            } else {
                4.0 * p - 4.0
            }
        }
        LfoShape::SawUp => 2.0 * p - 1.0,
        LfoShape::SawDown => 1.0 - 2.0 * p,
        LfoShape::Square => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        LfoShape::SampleHold | LfoShape::SmoothRandom => 0.0,
    }
}

/// The LFO's rate in Hz: `rate` (already modulated) or, with SYNC on, the tempo division.
pub fn rate_hz(p: &LfoParams, rate: f32, bpm: f32) -> f32 {
    if p.sync {
        1.0 / p.sync_div.seconds(bpm)
    } else {
        rate
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Lfo {
    pub phase: f32,
    /// The last output (polarity and fade-in applied).
    pub value: f32,
    held: f32,
    from: f32,
    to: f32,
    age: f32,
    done: bool,
    rng: u32,
}

impl Lfo {
    pub fn new(seed: u32) -> Lfo {
        let mut rng = seed.max(1);
        let held = next_bipolar(&mut rng);
        let to = next_bipolar(&mut rng);
        Lfo {
            phase: 0.0,
            value: 0.0,
            held,
            from: 0.0,
            to,
            age: 0.0,
            done: false,
            rng,
        }
    }

    /// Note-on (TRIG, ONE-SHOT): start at PHASE and fade in over DELAY.
    pub fn restart(&mut self, p: &LfoParams) {
        self.phase = p.phase.clamp(0.0, 1.0);
        self.age = 0.0;
        self.done = false;
    }

    /// Advance one tick at `hz` and return the output.
    pub fn tick(&mut self, p: &LfoParams, hz: f32, dt: f32) -> f32 {
        if !self.done {
            let next = self.phase + hz.max(0.0) * dt;
            if next >= 1.0 {
                if p.mode == LfoMode::OneShot {
                    self.phase = 1.0;
                    self.done = true;
                } else {
                    self.phase = next.fract();
                    self.held = next_bipolar(&mut self.rng);
                    self.from = self.to;
                    self.to = next_bipolar(&mut self.rng);
                }
            } else {
                self.phase = next;
            }
        }
        let raw = match p.shape {
            LfoShape::SampleHold => self.held,
            LfoShape::SmoothRandom => {
                self.from + (self.to - self.from) * (0.5 - 0.5 * (PI * self.phase).cos())
            }
            shape => shape_value(shape, if self.done { 0.999_999 } else { self.phase }),
        };
        let fade = if p.mode != LfoMode::Free && p.delay > 0.0 {
            (self.age / p.delay).min(1.0)
        } else {
            1.0
        };
        self.age += dt;
        let out = if p.polarity == Polarity::Uni {
            0.5 * (raw + 1.0)
        } else {
            raw
        };
        self.value = out * fade;
        self.value
    }
}
