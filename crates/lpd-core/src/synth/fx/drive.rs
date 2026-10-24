//! DRIVE (spec 5, K19): a 2× oversampled waveshaper with a tone low-pass.

use crate::synth::params::{DriveFx, DriveType};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

const TAPS: usize = 31;

#[derive(Clone, Copy)]
struct Fir {
    hist: [f32; TAPS],
    pos: usize,
}

impl Fir {
    const fn new() -> Fir {
        Fir {
            hist: [0.0; TAPS],
            pos: 0,
        }
    }

    #[inline]
    fn push(&mut self, x: f32, coef: &[f32; TAPS]) -> f32 {
        self.hist[self.pos] = x;
        let mut acc = 0.0;
        let mut j = self.pos;
        for c in coef {
            acc += c * self.hist[j];
            j = if j == 0 { TAPS - 1 } else { j - 1 };
        }
        self.pos = if self.pos + 1 == TAPS {
            0
        } else {
            self.pos + 1
        };
        acc
    }
}

/// Windowed-sinc low-pass at a quarter of the (2×) rate, Blackman window, unity DC gain.
fn halfband() -> [f32; TAPS] {
    let m = (TAPS - 1) as f32 / 2.0;
    let mut h = [0.0f32; TAPS];
    for (n, v) in h.iter_mut().enumerate() {
        let x = n as f32 - m;
        let sinc = if x == 0.0 {
            1.0
        } else {
            (PI * 0.5 * x).sin() / (PI * 0.5 * x)
        };
        let w = 0.42 - 0.5 * (TAU * n as f32 / (TAPS - 1) as f32).cos()
            + 0.08 * (2.0 * TAU * n as f32 / (TAPS - 1) as f32).cos();
        *v = 0.5 * sinc * w;
    }
    let sum: f32 = h.iter().sum();
    h.iter_mut().for_each(|v| *v /= sum);
    h
}

#[inline]
fn shape(kind: DriveType, v: f32) -> f32 {
    match kind {
        DriveType::Soft => v.tanh(),
        DriveType::Tube => (v + 0.3).tanh() - 0.3f32.tanh(),
        DriveType::Hard => v.clamp(-1.0, 1.0),
        DriveType::Fold => (v * FRAC_PI_2).sin(),
    }
}

pub struct Drive {
    rate: f32,
    coef: [f32; TAPS],
    up: [Fir; 2],
    down: [Fir; 2],
    lp: [f32; 2],
    /// DC blocker state (x1, y1) per side, for TUBE.
    dc: [(f32, f32); 2],
}

impl Drive {
    pub(super) fn pending(&self) -> bool {
        self.up
            .iter()
            .chain(&self.down)
            .any(|f| f.hist.iter().any(|v| v.abs() >= super::QUIET_IN))
            || self.lp.iter().any(|v| v.abs() >= super::QUIET_IN)
            || self
                .dc
                .iter()
                .any(|(x, y)| x.abs().max(y.abs()) >= super::QUIET_IN)
    }

    pub fn new(rate: f32) -> Drive {
        Drive {
            rate,
            coef: halfband(),
            up: [Fir::new(); 2],
            down: [Fir::new(); 2],
            lp: [0.0; 2],
            dc: [(0.0, 0.0); 2],
        }
    }

    pub fn clear(&mut self) {
        self.up = [Fir::new(); 2];
        self.down = [Fir::new(); 2];
        self.lp = [0.0; 2];
        self.dc = [(0.0, 0.0); 2];
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], p: &DriveFx) {
        let g = 10f32.powf(36.0 * p.amount.clamp(0.0, 1.0) / 20.0);
        let makeup = 1.0 / g.sqrt();
        let a = 1.0 - (-TAU * p.tone.clamp(500.0, 20_000.0) / (2.0 * self.rate)).exp();
        let mix = p.mix.clamp(0.0, 1.0);
        for (ch, buf) in [l, r].into_iter().enumerate() {
            for x in buf.iter_mut() {
                let mut out = 0.0;
                for k in 0..2 {
                    let u = self.up[ch].push(if k == 0 { 2.0 * *x } else { 0.0 }, &self.coef);
                    self.lp[ch] += (shape(p.kind, g * u) - self.lp[ch]) * a;
                    let mut wet = self.lp[ch] * makeup;
                    if p.kind == DriveType::Tube {
                        let (x1, y1) = self.dc[ch];
                        let y = wet - x1 + 0.995 * y1;
                        self.dc[ch] = (wet, y);
                        wet = y;
                    }
                    let mixed = u + (wet - u) * mix;
                    let d = self.down[ch].push(mixed, &self.coef);
                    if k == 1 {
                        out = d;
                    }
                }
                *x = out;
            }
        }
    }
}
