//! REVERB (spec 5): an 8-line feedback delay network with Hadamard mixing.

use super::line::DelayLine;
use crate::synth::params::ReverbFx;
use std::f32::consts::TAU;

const BASE_MS: [f32; 8] = [29.7, 37.1, 41.1, 43.7, 53.0, 59.9, 67.7, 73.9];
const SIGNS: [f32; 8] = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
const MOD_S: f32 = 0.000_25;
const MOD_HZ: f32 = 0.5;

fn hadamard8(x: &mut [f32; 8]) {
    let mut h = 1;
    while h < 8 {
        for i in (0..8).step_by(h * 2) {
            for j in i..i + h {
                let (a, b) = (x[j], x[j + h]);
                x[j] = a + b;
                x[j + h] = a - b;
            }
        }
        h *= 2;
    }
    x.iter_mut().for_each(|v| *v *= 0.353_553_4);
}

pub struct Reverb {
    rate: f32,
    lines: [DelayLine; 8],
    pre: DelayLine,
    lp: [f32; 8],
    phase: f32,
}

impl Reverb {
    pub(super) fn pending(&self) -> bool {
        self.lines.iter().any(DelayLine::pending)
            || self.pre.pending()
            || self.lp.iter().any(|v| v.abs() >= super::QUIET_IN)
    }

    pub fn new(rate: f32) -> Reverb {
        let lines = std::array::from_fn(|i| {
            DelayLine::new((BASE_MS[i] / 1000.0 * rate + MOD_S * rate) as usize + 8)
        });
        Reverb {
            rate,
            lines,
            pre: DelayLine::new((0.2 * rate) as usize + 8),
            lp: [0.0; 8],
            phase: 0.0,
        }
    }

    pub fn clear(&mut self) {
        self.lines.iter_mut().for_each(DelayLine::clear);
        self.pre.clear();
        self.lp = [0.0; 8];
        self.phase = 0.0;
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], p: &ReverbFx) {
        let scale = 0.35 + 0.65 * p.size.clamp(0.0, 1.0);
        let decay = p.decay.clamp(0.2, 20.0);
        let mut lens = [0.0f32; 8];
        let mut gains = [0.0f32; 8];
        for i in 0..8 {
            lens[i] = BASE_MS[i] / 1000.0 * scale * self.rate;
            gains[i] = 10f32.powf(-3.0 * (lens[i] / self.rate) / decay);
        }
        let damp = p.damp.clamp(0.0, 1.0);
        let a = if damp <= 0.0 {
            1.0
        } else {
            1.0 - (-TAU * 20_000.0 * (1_500.0f32 / 20_000.0).powf(damp) / self.rate).exp()
        };
        let pre = p.pre_delay.clamp(0.0, 0.2) * self.rate;
        let mix = p.mix.clamp(0.0, 1.0);
        let depth = MOD_S * self.rate;
        for i in 0..l.len().min(r.len()) {
            let x = 0.5 * (l[i] + r[i]);
            let input = if pre >= 1.0 { self.pre.read(pre) } else { x };
            self.pre.write(x);
            let mut o = [0.0f32; 8];
            for k in 0..8 {
                let wobble = if k < 4 {
                    depth * (TAU * (self.phase + k as f32 * 0.25)).sin()
                } else {
                    0.0
                };
                let y = self.lines[k].read(lens[k] + wobble);
                self.lp[k] += (y - self.lp[k]) * a;
                o[k] = self.lp[k] * gains[k];
            }
            let wet_l = 0.5 * (self.lp[0] + self.lp[2] + self.lp[4] + self.lp[6]);
            let wet_r = 0.5 * (self.lp[1] + self.lp[3] + self.lp[5] + self.lp[7]);
            hadamard8(&mut o);
            for k in 0..8 {
                self.lines[k].write(input * 0.35 * SIGNS[k] + o[k]);
            }
            l[i] = (1.0 - mix) * l[i] + mix * wet_l;
            r[i] = (1.0 - mix) * r[i] + mix * wet_r;
            self.phase += MOD_HZ / self.rate;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
}
