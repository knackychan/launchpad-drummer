//! Voice filters (spec §3.5, plan K9): a Moog-style ladder for LP 24 and a topology-preserving
//! state-variable filter for LP 12, HP 12, BP 12 and NOTCH. Two instances per voice (stereo).

use super::params::FilterType;
use std::f32::consts::PI;

/// The ladder's input is scaled by this before its saturating stage (and the output back), so
/// ordinary levels stay clean and loud ones saturate like the original.
pub const LADDER_HEADROOM: f32 = 0.5;

/// Prewarped integrator gain for `cutoff` (clamped to 20 Hz … min(20 kHz, 0.45 × rate)).
#[inline]
pub fn g_for(cutoff: f32, rate: f32) -> f32 {
    let top = 20_000.0f32.min(0.45 * rate);
    (PI * cutoff.clamp(20.0, top) / rate).tan()
}

pub fn ladder_k(reso: f32) -> f32 {
    4.1 * reso.clamp(0.0, 1.0)
}

pub fn svf_q(reso: f32) -> f32 {
    0.5 * 50f32.powf(reso.clamp(0.0, 1.0))
}

/// DRIVE 0–1 → pre-gain 0–24 dB.
pub fn drive_gain(drive: f32) -> f32 {
    10f32.powf(24.0 * drive.clamp(0.0, 1.0) / 20.0)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ladder {
    s: [f32; 4],
}

impl Ladder {
    #[inline]
    pub fn process(&mut self, x: f32, g: f32, k: f32) -> f32 {
        let x = x * LADDER_HEADROOM;
        let gg = g / (1.0 + g);
        let g2 = gg * gg;
        let g4 = g2 * g2;
        let sum = (1.0 - gg) * (gg * g2 * self.s[0] + g2 * self.s[1] + gg * self.s[2] + self.s[3]);
        let y4 = (g4 * x + sum) / (1.0 + k * g4);
        let mut input = (x - k * y4).tanh();
        for s in &mut self.s {
            let v = (input - *s) * gg;
            let y = v + *s;
            *s = y + v;
            input = y;
        }
        input * (1.0 + k).sqrt() / LADDER_HEADROOM
    }

    pub fn reset(&mut self) {
        self.s = [0.0; 4];
    }

    pub fn is_finite(&self) -> bool {
        self.s.iter().all(|v| v.is_finite())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    s1: f32,
    s2: f32,
}

impl Svf {
    #[inline]
    pub fn process(&mut self, x: f32, g: f32, q: f32, kind: FilterType) -> f32 {
        let r2 = 1.0 / q;
        let hp = (x - (r2 + g) * self.s1 - self.s2) / (1.0 + r2 * g + g * g);
        let bp = g * hp + self.s1;
        self.s1 = g * hp + bp;
        let lp = g * bp + self.s2;
        self.s2 = g * bp + lp;
        match kind {
            FilterType::Hp12 => hp,
            FilterType::Bp12 => bp * r2,
            FilterType::Notch => x - r2 * bp,
            FilterType::Lp12 | FilterType::Lp24 => lp,
        }
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    pub fn is_finite(&self) -> bool {
        self.s1.is_finite() && self.s2.is_finite()
    }
}

/// A voice's stereo filter. The voice passes coefficients once per control tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct VoiceFilter {
    ladder: [Ladder; 2],
    svf: [Svf; 2],
    kind: FilterType,
}

impl VoiceFilter {
    /// Tests only: make the next output non-finite.
    #[doc(hidden)]
    pub fn debug_poison(&mut self) {
        self.ladder = [Ladder { s: [f32::NAN; 4] }; 2];
        self.svf = [Svf {
            s1: f32::NAN,
            s2: f32::NAN,
        }; 2];
    }

    /// A new type starts from silence (the other topology's state means nothing to it).
    pub fn set_kind(&mut self, kind: FilterType) {
        if kind != self.kind {
            self.reset();
            self.kind = kind;
        }
    }

    pub fn kind(&self) -> FilterType {
        self.kind
    }

    pub fn reset(&mut self) {
        self.ladder = [Ladder::default(); 2];
        self.svf = [Svf::default(); 2];
    }

    /// `res` = `ladder_k(reso)` for LP 24, else `svf_q(reso)`; `dg` = `drive_gain(drive)`.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32, g: f32, res: f32, dg: f32) -> (f32, f32) {
        let makeup = 1.0 / dg.sqrt();
        match self.kind {
            FilterType::Lp24 => (
                self.ladder[0].process(l * dg, g, res) * makeup,
                self.ladder[1].process(r * dg, g, res) * makeup,
            ),
            kind => {
                let (l, r) = if dg > 1.0001 {
                    ((l * dg).tanh(), (r * dg).tanh())
                } else {
                    (l, r)
                };
                (
                    self.svf[0].process(l, g, res, kind) * makeup,
                    self.svf[1].process(r, g, res, kind) * makeup,
                )
            }
        }
    }

    pub fn is_finite(&self) -> bool {
        self.ladder.iter().all(Ladder::is_finite) && self.svf.iter().all(Svf::is_finite)
    }
}

/// Linear magnitude response in dB at `freqs` (analog prototype); DRIVE shifts it by 12·DRIVE dB.
/// For the telemetry thread (allocates).
pub fn response_db(
    kind: FilterType,
    cutoff: f32,
    reso: f32,
    drive: f32,
    freqs: &[f32],
) -> Vec<f32> {
    let offset = 12.0 * drive.clamp(0.0, 1.0);
    freqs
        .iter()
        .map(|&f| {
            let w = f / cutoff.max(1.0);
            let mag = match kind {
                FilterType::Lp24 => {
                    let k = ladder_k(reso);
                    let a = 1.0 - w * w;
                    (1.0 + k).sqrt() / (a * a - 4.0 * w * w + k).hypot(4.0 * w * a)
                }
                FilterType::Lp12 | FilterType::Hp12 | FilterType::Bp12 | FilterType::Notch => {
                    let q = svf_q(reso);
                    let den = ((1.0 - w * w).powi(2) + (w / q).powi(2)).sqrt();
                    match kind {
                        FilterType::Hp12 => w * w / den,
                        FilterType::Bp12 => (w / q) / den,
                        FilterType::Notch => (1.0 - w * w).abs() / den,
                        _ => 1.0 / den,
                    }
                }
            };
            (20.0 * mag.max(1e-6).log10() + offset).max(-120.0)
        })
        .collect()
}
