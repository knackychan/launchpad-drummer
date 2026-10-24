//! CHORUS (spec 5): two modulated taps per side.

use super::line::DelayLine;
use crate::synth::params::ChorusFx;
use std::f32::consts::TAU;

pub struct Chorus {
    rate: f32,
    lines: [DelayLine; 2],
    phase: f32,
}

impl Chorus {
    pub(super) fn pending(&self) -> bool {
        self.lines.iter().any(DelayLine::pending)
    }

    pub fn new(rate: f32) -> Chorus {
        let len = (0.040 * rate) as usize + 4;
        Chorus {
            rate,
            lines: [DelayLine::new(len), DelayLine::new(len)],
            phase: 0.0,
        }
    }

    pub fn clear(&mut self) {
        self.lines.iter_mut().for_each(DelayLine::clear);
        self.phase = 0.0;
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], p: &ChorusFx) {
        let base = p.delay.clamp(0.005, 0.03) * self.rate;
        let swing = p.depth.clamp(0.0, 1.0) * 0.005 * self.rate;
        let inc = p.rate.clamp(0.05, 5.0) / self.rate;
        let (fb, mix) = (p.feedback.clamp(0.0, 0.9), p.mix.clamp(0.0, 1.0));
        let tap = |ph: f32| base + swing * (0.5 + 0.5 * (TAU * ph).sin());
        for i in 0..l.len().min(r.len()) {
            for (ch, x) in [&mut l[i], &mut r[i]].into_iter().enumerate() {
                let off = if ch == 0 { 0.0 } else { 0.25 };
                let line = &mut self.lines[ch];
                let wet = 0.5
                    * (line.read(tap(self.phase + off)) + line.read(tap(self.phase + off + 0.5)));
                line.write(*x + fb * wet);
                *x = (1.0 - mix) * *x + mix * wet;
            }
            self.phase += inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
}
