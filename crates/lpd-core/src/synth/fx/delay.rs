//! DELAY (spec 5): stereo or ping-pong echo with a damped feedback path.

use super::line::DelayLine;
use crate::synth::params::DelayFx;
use std::f32::consts::TAU;

pub struct Delay {
    rate: f32,
    lines: [DelayLine; 2],
    /// Current delay in samples (glides towards the target).
    time: f32,
    lp: [f32; 2],
    /// Start at the target time (after `new` and `clear`).
    fresh: bool,
}

impl Delay {
    pub(super) fn pending(&self) -> bool {
        self.lines.iter().any(DelayLine::pending)
            || self.lp.iter().any(|v| v.abs() >= super::QUIET_IN)
    }

    pub fn new(rate: f32) -> Delay {
        let len = (2.0 * rate) as usize + 8;
        Delay {
            rate,
            lines: [DelayLine::new(len), DelayLine::new(len)],
            time: 0.0,
            lp: [0.0; 2],
            fresh: true,
        }
    }

    pub fn clear(&mut self) {
        self.lines.iter_mut().for_each(DelayLine::clear);
        self.lp = [0.0; 2];
        self.fresh = true;
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], p: &DelayFx, bpm: f32) {
        let seconds = if p.sync {
            p.sync_div.seconds(bpm).min(2.0)
        } else {
            p.time.clamp(0.001, 2.0)
        };
        let target = seconds * self.rate;
        if self.fresh {
            self.time = target;
            self.fresh = false;
        }
        let a = 1.0 - (-TAU * p.tone.clamp(500.0, 20_000.0) / self.rate).exp();
        let (fb, mix) = (p.feedback.clamp(0.0, 0.95), p.mix.clamp(0.0, 1.0));
        for i in 0..l.len().min(r.len()) {
            self.time += (target - self.time) * 0.0005;
            let (xl, xr) = (l[i], r[i]);
            let yl = self.lines[0].read(self.time);
            let yr = self.lines[1].read(self.time);
            self.lp[0] += (yl - self.lp[0]) * a;
            self.lp[1] += (yr - self.lp[1]) * a;
            if p.ping_pong {
                self.lines[0].write(0.5 * (xl + xr) + fb * self.lp[1]);
                self.lines[1].write(fb * self.lp[0]);
            } else {
                self.lines[0].write(xl + fb * self.lp[0]);
                self.lines[1].write(xr + fb * self.lp[1]);
            }
            l[i] = (1.0 - mix) * xl + mix * yl;
            r[i] = (1.0 - mix) * xr + mix * yr;
        }
    }
}
