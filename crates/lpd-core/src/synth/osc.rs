//! Wavetable oscillator with unison and warp, the sub oscillator and noise (spec §3.2, §3.4).

use super::params::{NoiseColor, SubShape, Warp, UNISON_MAX};
use super::wavetable::Wavetable;
use std::f32::consts::{FRAC_PI_4, SQRT_2, TAU};

/// xorshift32; the state must not be 0.
#[inline]
pub fn next_u32(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

/// Uniform in [0, 1).
#[inline]
pub fn next_unit(state: &mut u32) -> f32 {
    (next_u32(state) >> 8) as f32 / (1u32 << 24) as f32
}

/// Uniform in [−1, 1).
#[inline]
pub fn next_bipolar(state: &mut u32) -> f32 {
    2.0 * next_unit(state) - 1.0
}

/// Band-limited step correction for naive saw and square waves.
#[inline]
pub fn polyblep(mut t: f32, dt: f32) -> f32 {
    if t < dt {
        t /= dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// Index of the centre copy (for an even count, the inner copy below the note).
pub fn centre(n: usize) -> usize {
    (n.clamp(1, UNISON_MAX) - 1) / 2
}

/// Pitch offsets of the copies in cents, spread evenly over ±100·detune².
pub fn unison_cents(n: usize, detune: f32) -> [f32; UNISON_MAX] {
    let n = n.clamp(1, UNISON_MAX);
    let spread = 100.0 * detune * detune;
    let mut out = [0.0; UNISON_MAX];
    if n > 1 {
        for (i, v) in out.iter_mut().enumerate().take(n) {
            *v = spread * (2.0 * i as f32 / (n - 1) as f32 - 1.0);
        }
    }
    out
}

/// Copy gains: the centre 1, the others BLEND, scaled to unit power.
pub fn unison_gains(n: usize, blend: f32) -> [f32; UNISON_MAX] {
    let n = n.clamp(1, UNISON_MAX);
    let c = centre(n);
    let mut g = [0.0; UNISON_MAX];
    let mut power = 0.0;
    for (i, v) in g.iter_mut().enumerate().take(n) {
        *v = if i == c { 1.0 } else { blend.clamp(0.0, 1.0) };
        power += *v * *v;
    }
    let norm = 1.0 / power.sqrt().max(1e-6);
    g.iter_mut().take(n).for_each(|v| *v *= norm);
    g
}

/// Copy pans −1…1 spread evenly across ±WIDTH (0 for a single copy).
pub fn unison_pans(n: usize, width: f32) -> [f32; UNISON_MAX] {
    let n = n.clamp(1, UNISON_MAX);
    let mut out = [0.0; UNISON_MAX];
    if n > 1 {
        for (i, v) in out.iter_mut().enumerate().take(n) {
            *v = width.clamp(0.0, 1.0) * (2.0 * i as f32 / (n - 1) as f32 - 1.0);
        }
    }
    out
}

/// Constant-power pan gains, 1.0 on both sides at the centre.
#[inline]
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
    (a.cos() * SQRT_2, a.sin() * SQRT_2)
}

/// A warp mode with its per-tick constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarpShape {
    pub mode: Warp,
    pub amt: f32,
    ratio: f32,
    exponent: f32,
    width: f32,
}

impl WarpShape {
    pub fn new(mode: Warp, amt: f32) -> WarpShape {
        let amt = amt.clamp(0.0, 1.0);
        WarpShape {
            mode,
            amt,
            ratio: 16f32.powf(amt),
            exponent: 1.0 + 4.0 * amt,
            width: 1.0 - 0.98 * amt,
        }
    }

    /// The phase a frame is read at; `None` = the silent part of a PWM cycle.
    #[inline]
    pub fn phase(&self, p: f32) -> Option<f32> {
        match self.mode {
            Warp::Sync => Some((p * self.ratio).fract()),
            Warp::Bend => Some(p.powf(self.exponent)),
            Warp::Pwm => (p < self.width).then(|| p / self.width),
            Warp::Off | Warp::Fm | Warp::Rm => Some(p),
        }
    }

    /// Frequency multiplier for the band-limit choice (SYNC reads faster).
    pub fn freq_factor(&self) -> f32 {
        if self.mode == Warp::Sync {
            self.ratio
        } else {
            1.0
        }
    }
}

/// One control tick of oscillator settings (computed by the voice, read every sample).
#[derive(Clone, Copy)]
pub struct OscTick {
    pub table: &'static Wavetable,
    pub level: usize,
    pub pos: f32,
    pub n: usize,
    pub inc: [f32; UNISON_MAX],
    pub gain_l: [f32; UNISON_MAX],
    pub gain_r: [f32; UNISON_MAX],
    pub centre: usize,
    pub warp: WarpShape,
}

impl OscTick {
    /// `hz` is the note frequency with every pitch offset applied; `level` is the 0–1 knob.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        table: &'static Wavetable,
        rate: f32,
        hz: f32,
        pos: f32,
        unison: usize,
        detune: f32,
        blend: f32,
        width: f32,
        pan: f32,
        level: f32,
        warp: WarpShape,
    ) -> OscTick {
        let n = unison.clamp(1, UNISON_MAX);
        let cents = unison_cents(n, detune);
        let gains = unison_gains(n, blend);
        let pans = unison_pans(n, width);
        let lvl = level.clamp(0.0, 1.0);
        let lvl = lvl * lvl;
        let mut t = OscTick {
            table,
            level: 0,
            pos,
            n,
            inc: [0.0; UNISON_MAX],
            gain_l: [0.0; UNISON_MAX],
            gain_r: [0.0; UNISON_MAX],
            centre: centre(n),
            warp,
        };
        let mut top = hz;
        for i in 0..n {
            let f = hz * (cents[i] / 1200.0).exp2();
            top = top.max(f);
            t.inc[i] = (f / rate).min(0.5);
            let (gl, gr) = pan_gains(pan + pans[i]);
            t.gain_l[i] = gains[i] * lvl * gl;
            t.gain_r[i] = gains[i] * lvl * gr;
        }
        t.level = Wavetable::level_for(top * warp.freq_factor(), rate);
        t
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OscState {
    pub phase: [f32; UNISON_MAX],
    /// The centre copy's output at the previous sample (the other oscillator's FM / RM source).
    pub last_centre: f32,
}

impl OscState {
    /// A fresh note: every copy starts at PHASE RAND × random(0…1).
    pub fn reset(&mut self, phase_rand: f32, rng: &mut u32) {
        for p in &mut self.phase {
            *p = phase_rand.clamp(0.0, 1.0) * next_unit(rng);
        }
        self.last_centre = 0.0;
    }

    /// One stereo sample. `modulator` is the other oscillator's previous centre sample.
    #[inline]
    pub fn sample(&mut self, t: &OscTick, modulator: f32) -> (f32, f32) {
        let (mut l, mut r, mut centre_out) = (0.0, 0.0, 0.0);
        for i in 0..t.n {
            let mut p = self.phase[i];
            if t.warp.mode == Warp::Fm {
                p = (p + t.warp.amt * 8.0 / TAU * modulator).rem_euclid(1.0);
            }
            let mut v = match t.warp.phase(p) {
                Some(q) => t.table.read(t.level, t.pos, q),
                None => 0.0,
            };
            if t.warp.mode == Warp::Rm {
                v += (v * modulator - v) * t.warp.amt;
            }
            if i == t.centre {
                centre_out = v;
            }
            l += v * t.gain_l[i];
            r += v * t.gain_r[i];
            let next = self.phase[i] + t.inc[i];
            self.phase[i] = if next >= 1.0 { next - 1.0 } else { next };
        }
        self.last_centre = centre_out;
        (l, r)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SubState {
    pub phase: f32,
}

impl SubState {
    #[inline]
    pub fn sample(&mut self, shape: SubShape, inc: f32) -> f32 {
        let p = self.phase;
        let v = match shape {
            SubShape::Sine => (TAU * p).sin(),
            SubShape::Triangle => 1.0 - 4.0 * (p - 0.5).abs(),
            SubShape::Saw => 2.0 * p - 1.0 - polyblep(p, inc),
            SubShape::Square => {
                let raw = if p < 0.5 { 1.0 } else { -1.0 };
                raw + polyblep(p, inc) - polyblep((p + 0.5).fract(), inc)
            }
        };
        let next = p + inc;
        self.phase = if next >= 1.0 { next - 1.0 } else { next };
        v
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NoiseState {
    rng: u32,
    b: [f32; 3],
}

impl NoiseState {
    pub fn new(seed: u32) -> NoiseState {
        NoiseState {
            rng: seed.max(1),
            b: [0.0; 3],
        }
    }

    #[inline]
    pub fn sample(&mut self, color: NoiseColor) -> f32 {
        let white = next_bipolar(&mut self.rng);
        match color {
            NoiseColor::White => white,
            NoiseColor::Pink => {
                // Paul Kellet's economy pink filter (±0.5 dB above 10 Hz).
                self.b[0] = 0.997_65 * self.b[0] + white * 0.099_046;
                self.b[1] = 0.963 * self.b[1] + white * 0.296_516_4;
                self.b[2] = 0.57 * self.b[2] + white * 1.052_691_3;
                (self.b[0] + self.b[1] + self.b[2] + white * 0.1848) * 0.25
            }
        }
    }
}
