use super::{LowPass, PlayParams, Sample, Voice};
use rtrb::{Consumer, Producer, RingBuffer};
use std::f32::consts::{FRAC_PI_4, SQRT_2};
use std::sync::Arc;

pub const MAX_VOICES: usize = 32;
/// Extra slots so voices that are fading out do not count against MAX_VOICES.
const SLOTS: usize = MAX_VOICES + 8;
pub const GROUPS: usize = 8;
/// Largest block rendered in one pass; longer buffers are split.
pub const MAX_BLOCK: usize = 1024;
const RESTART_FADE_MS: f32 = 3.0;
const CHOKE_FADE_MS: f32 = 5.0;
const RELEASE_FADE_MS: f32 = 10.0;
const STEAL_FADE_MS: f32 = 3.0;
const STOP_FADE_MS: f32 = 5.0;
/// Soft clipper is linear below this level.
const CLIP_KNEE: f32 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupParams {
    /// Linear gain.
    pub gain: f32,
    /// -1 = left, 0 = centre, 1 = right.
    pub pan: f32,
    pub pitch_semitones: f32,
    pub lowpass_hz: f32,
    pub mute: bool,
    pub solo: bool,
}

impl Default for GroupParams {
    fn default() -> Self {
        GroupParams {
            gain: 1.0,
            pan: 0.0,
            pitch_semitones: 0.0,
            lowpass_hz: 20_000.0,
            mute: false,
            solo: false,
        }
    }
}

#[derive(Debug)]
pub enum Command {
    /// `pad` identifies the pad across banks (bank * 64 + y * 8 + x).
    Trigger {
        pad: u16,
        sample: Arc<Sample>,
        params: PlayParams,
    },
    Release {
        pad: u16,
    },
    StopAll,
    SetGroup {
        group: u8,
        params: GroupParams,
    },
    SetMasterGain(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    VoiceStarted { pad: u16 },
    VoiceEnded { pad: u16 },
}

struct GroupState {
    params: GroupParams,
    lp_l: LowPass,
    lp_r: LowPass,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
}

/// Owned by the audio thread. All buffers are allocated in `new`; `handle` and
/// `render` never allocate.
pub struct Engine {
    rate: f32,
    voices: Vec<Voice>,
    groups: Vec<GroupState>,
    master_gain: f32,
    next_age: u64,
    events: Producer<Event>,
}

impl Engine {
    pub fn new(sample_rate: u32) -> (Engine, Consumer<Event>) {
        let (events, rx) = RingBuffer::new(1024);
        let groups = (0..GROUPS)
            .map(|_| GroupState {
                params: GroupParams::default(),
                lp_l: LowPass::new(),
                lp_r: LowPass::new(),
                buf_l: vec![0.0; MAX_BLOCK],
                buf_r: vec![0.0; MAX_BLOCK],
            })
            .collect();
        let engine = Engine {
            rate: sample_rate as f32,
            voices: (0..SLOTS).map(|_| Voice::idle()).collect(),
            groups,
            master_gain: 1.0,
            next_age: 0,
            events,
        };
        (engine, rx)
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate as u32
    }

    /// Voices currently producing sound, including ones fading out.
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.is_active()).count()
    }

    fn frames_for(&self, ms: f32) -> usize {
        (ms / 1000.0 * self.rate) as usize
    }

    pub fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Trigger {
                pad,
                sample,
                params,
            } => self.trigger(pad, sample, params),
            Command::Release { pad } => {
                let frames = self.frames_for(RELEASE_FADE_MS);
                for v in &mut self.voices {
                    if v.is_active() && v.pad == pad && v.params.hold {
                        v.fade_out(frames);
                    }
                }
            }
            Command::StopAll => {
                let frames = self.frames_for(STOP_FADE_MS);
                for v in &mut self.voices {
                    v.fade_out(frames);
                }
            }
            Command::SetGroup { group, params } => {
                if let Some(g) = self.groups.get_mut(group as usize) {
                    g.params = params;
                    g.lp_l.set_cutoff(params.lowpass_hz, self.rate);
                    g.lp_r.set_cutoff(params.lowpass_hz, self.rate);
                }
            }
            Command::SetMasterGain(gain) => self.master_gain = gain,
        }
    }

    fn trigger(&mut self, pad: u16, sample: Arc<Sample>, params: PlayParams) {
        let restart = self.frames_for(RESTART_FADE_MS);
        let choke = self.frames_for(CHOKE_FADE_MS);
        let steal = self.frames_for(STEAL_FADE_MS);
        for v in self.voices.iter_mut().filter(|v| v.is_active()) {
            if v.pad == pad {
                v.fade_out(restart);
            } else if params.choke_group.is_some() && v.params.choke_group == params.choke_group {
                v.fade_out(choke);
            }
        }
        let sounding = self
            .voices
            .iter()
            .filter(|v| v.is_active() && !v.is_fading())
            .count();
        if sounding >= MAX_VOICES {
            if let Some(oldest) = self
                .voices
                .iter_mut()
                .filter(|v| v.is_active() && !v.is_fading())
                .min_by_key(|v| v.age)
            {
                oldest.fade_out(steal);
            }
        }
        let slot = match self.voices.iter().position(|v| !v.is_active()) {
            Some(i) => i,
            None => {
                // Every slot is busy fading: cut the oldest immediately.
                let i = self
                    .voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.age)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let _ = self.events.push(Event::VoiceEnded {
                    pad: self.voices[i].pad,
                });
                i
            }
        };
        let group = (params.mix_group as usize).min(GROUPS - 1);
        let semitones = (params.pitch_semitones + self.groups[group].params.pitch_semitones) as f64;
        let ratio = sample.rate as f64 / self.rate as f64 * 2f64.powf(semitones / 12.0);
        let age = self.next_age;
        self.next_age += 1;
        self.voices[slot].start(pad, sample, params, ratio, age);
        let _ = self.events.push(Event::VoiceStarted { pad });
    }

    /// Render interleaved stereo into `out` (length = 2 × frames).
    pub fn render(&mut self, out: &mut [f32]) {
        for chunk in out.chunks_mut(MAX_BLOCK * 2) {
            self.render_block(chunk);
        }
    }

    fn render_block(&mut self, out: &mut [f32]) {
        let n = out.len() / 2;
        for g in &mut self.groups {
            g.buf_l[..n].fill(0.0);
            g.buf_r[..n].fill(0.0);
        }
        for v in &mut self.voices {
            if !v.is_active() {
                continue;
            }
            let pad = v.pad;
            let g = &mut self.groups[(v.params.mix_group as usize).min(GROUPS - 1)];
            if !v.render_add(&mut g.buf_l[..n], &mut g.buf_r[..n]) {
                let _ = self.events.push(Event::VoiceEnded { pad });
            }
        }
        let any_solo = self.groups.iter().any(|g| g.params.solo);
        out.fill(0.0);
        for g in &mut self.groups {
            let p = g.params;
            if p.mute || (any_solo && !p.solo) {
                continue;
            }
            let angle = (p.pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
            let gl = angle.cos() * SQRT_2 * p.gain;
            let gr = angle.sin() * SQRT_2 * p.gain;
            for i in 0..n {
                out[2 * i] += g.lp_l.process(g.buf_l[i]) * gl;
                out[2 * i + 1] += g.lp_r.process(g.buf_r[i]) * gr;
            }
        }
        for s in out.iter_mut() {
            *s = soft_clip(*s * self.master_gain);
        }
    }
}

/// Linear up to ±CLIP_KNEE, tanh knee above; output magnitude never exceeds 1.0.
#[inline]
fn soft_clip(x: f32) -> f32 {
    let a = x.abs();
    if a <= CLIP_KNEE {
        x
    } else {
        let range = 1.0 - CLIP_KNEE;
        (CLIP_KNEE + range * ((a - CLIP_KNEE) / range).tanh()).copysign(x)
    }
}
