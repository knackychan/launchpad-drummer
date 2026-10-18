use super::Sample;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayParams {
    /// Linear gain.
    pub gain: f32,
    pub pitch_semitones: f32,
    /// Playback range in source frames: start_frame <= end_frame <= sample.frames().
    pub start_frame: usize,
    pub end_frame: usize,
    pub choke_group: Option<u8>,
    /// 0..8
    pub mix_group: u8,
    /// Hold mode: releasing the pad fades the voice out.
    pub hold: bool,
}

/// Fade before the end of the playback range.
const END_FADE_MS: f64 = 2.0;

pub struct Voice {
    pub pad: u16,
    pub params: PlayParams,
    /// Trigger order; lower = older.
    pub age: u64,
    sample: Option<Arc<Sample>>,
    pos: f64,
    step: f64,
    end_fade_frames: f64,
    env: f32,
    env_step: f32,
}

impl Voice {
    pub fn idle() -> Voice {
        Voice {
            pad: 0,
            params: PlayParams {
                gain: 0.0,
                pitch_semitones: 0.0,
                start_frame: 0,
                end_frame: 0,
                choke_group: None,
                mix_group: 0,
                hold: false,
            },
            age: 0,
            sample: None,
            pos: 0.0,
            step: 0.0,
            end_fade_frames: 1.0,
            env: 0.0,
            env_step: 0.0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.sample.is_some()
    }

    pub fn is_fading(&self) -> bool {
        self.env_step < 0.0
    }

    /// Start playing. `rate_ratio` = sample rate / output rate × pitch factor.
    pub fn start(
        &mut self,
        pad: u16,
        sample: Arc<Sample>,
        params: PlayParams,
        rate_ratio: f64,
        age: u64,
    ) {
        self.end_fade_frames = (END_FADE_MS / 1000.0 * sample.rate as f64).max(1.0);
        self.pad = pad;
        self.params = params;
        self.age = age;
        self.pos = params.start_frame as f64;
        self.step = rate_ratio;
        self.env = 1.0;
        self.env_step = 0.0;
        self.sample = Some(sample);
    }

    /// Fade to silence over `frames` output frames, then stop. A slower fade never
    /// replaces a faster one already running.
    pub fn fade_out(&mut self, frames: usize) {
        if !self.is_active() {
            return;
        }
        let step = -self.env / frames.max(1) as f32;
        if step < self.env_step {
            self.env_step = step;
        }
    }

    /// Mix this voice into `l`/`r`. Returns false once the voice has finished (it is then idle).
    pub fn render_add(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let Some(sample) = self.sample.as_ref() else {
            return false;
        };
        let end = self.params.end_frame as f64;
        let mut finished = false;
        for (ol, or) in l.iter_mut().zip(r.iter_mut()) {
            if self.pos >= end || self.env <= 0.0 {
                finished = true;
                break;
            }
            let i = self.pos.floor();
            let t = (self.pos - i) as f32;
            let i = i as isize;
            let (al, ar) = sample.frame(i - 1);
            let (bl, br) = sample.frame(i);
            let (cl, cr) = sample.frame(i + 1);
            let (dl, dr) = sample.frame(i + 2);
            let tail = ((end - self.pos) / self.end_fade_frames).min(1.0) as f32;
            let g = self.params.gain * self.env * tail;
            *ol += hermite(al, bl, cl, dl, t) * g;
            *or += hermite(ar, br, cr, dr, t) * g;
            self.pos += self.step;
            self.env = (self.env + self.env_step).max(0.0);
        }
        if finished {
            self.sample = None;
            self.env_step = 0.0;
        }
        !finished
    }
}

/// 4-point, 3rd-order Hermite interpolation between x0 and x1 at t in [0, 1).
#[inline]
fn hermite(xm1: f32, x0: f32, x1: f32, x2: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}
