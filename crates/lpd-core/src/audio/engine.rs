use super::taps::{self, TapSender};
use super::{Levels, LowPass, PlayParams, Sample, Taps, Voice};
use crate::instrument::{BasicKeys, Instrument, KeysSound};
use rtrb::{Consumer, Producer, RingBuffer};
use std::f32::consts::{FRAC_PI_4, SQRT_2};
use std::sync::Arc;

pub const MAX_VOICES: usize = 32;
pub const AUDITION_PAD: u16 = u16::MAX;
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
    Audition {
        sample: Arc<Sample>,
    },
    StopAudition,
    /// Keys instrument (Task 15).
    KeysNoteOn {
        note: u8,
        velocity: u8,
    },
    KeysNoteOff {
        note: u8,
    },
    /// −2..2 semitones.
    KeysPitch(f32),
    /// 0..1.
    KeysMod(f32),
    KeysSound(KeysSound),
    KeysAllOff,
    /// Linear gain of the keys, and where they go: a mixer channel (before its low-pass, pan,
    /// gain, mute and solo; D5: not its pitch) or, with `None`, the master bus.
    KeysRoute {
        gain: f32,
        group: Option<u8>,
    },
    /// Swap the instrument. The old one goes back through the trash queue (never dropped here).
    SetInstrument(Box<dyn Instrument>),
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
    auditions: [Voice; 2],
    audition_age: u64,
    audition_attack: [usize; 2],
    audition_l: Vec<f32>,
    audition_r: Vec<f32>,
    taps: Option<TapSender>,
    instrument: Box<dyn Instrument>,
    keys_gain: f32,
    keys_group: Option<usize>,
    keys_l: Vec<f32>,
    keys_r: Vec<f32>,
    trash: rtrb::Producer<Box<dyn Instrument>>,
    trash_rx: Option<Consumer<Box<dyn Instrument>>>,
}

impl Engine {
    pub fn new(sample_rate: u32) -> (Engine, Consumer<Event>) {
        Self::build(sample_rate, None)
    }

    /// Like `new`, plus lock-free taps on the output for the telemetry display.
    pub fn with_taps(sample_rate: u32) -> (Engine, Consumer<Event>, Taps) {
        let (sender, taps) = taps::channel();
        let (engine, rx) = Self::build(sample_rate, Some(sender));
        (engine, rx, taps)
    }

    fn build(sample_rate: u32, taps: Option<TapSender>) -> (Engine, Consumer<Event>) {
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
        let (trash, trash_rx) = RingBuffer::new(4);
        let engine = Engine {
            rate: sample_rate as f32,
            voices: (0..SLOTS).map(|_| Voice::idle()).collect(),
            groups,
            master_gain: 1.0,
            next_age: 0,
            events,
            auditions: [Voice::idle(), Voice::idle()],
            audition_age: 0,
            audition_attack: [0; 2],
            audition_l: vec![0.0; MAX_BLOCK],
            audition_r: vec![0.0; MAX_BLOCK],
            taps,
            instrument: Box::new(BasicKeys::new(sample_rate)),
            keys_gain: super::db_to_gain(-6.0),
            keys_group: None,
            keys_l: vec![0.0; MAX_BLOCK],
            keys_r: vec![0.0; MAX_BLOCK],
            trash,
            trash_rx: Some(trash_rx),
        };
        (engine, rx)
    }

    /// The consumer of replaced instruments; the host takes it once and drops what arrives on its
    /// own thread. Without it a replaced instrument is leaked rather than freed on the audio thread.
    pub fn take_trash(&mut self) -> Option<Consumer<Box<dyn Instrument>>> {
        self.trash_rx.take()
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
                for v in &mut self.auditions {
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
            Command::KeysNoteOn { note, velocity } => self.instrument.note_on(note, velocity),
            Command::KeysNoteOff { note } => self.instrument.note_off(note),
            Command::KeysPitch(semitones) => self.instrument.pitch_bend(semitones),
            Command::KeysMod(depth) => self.instrument.modulation(depth),
            Command::KeysSound(sound) => self.instrument.set_sound(sound),
            Command::KeysAllOff => self.instrument.all_notes_off(),
            Command::KeysRoute { gain, group } => {
                self.keys_gain = gain;
                self.keys_group = group.map(|g| (g as usize).min(GROUPS - 1));
            }
            Command::SetInstrument(new) => {
                let mut old = std::mem::replace(&mut self.instrument, new);
                old.all_notes_off();
                if let Err(rtrb::PushError::Full(old)) = self.trash.push(old) {
                    // Nobody is collecting: leak it rather than free it on the audio thread.
                    std::mem::forget(old);
                }
            }
            Command::StopAudition => {
                let frames = self.frames_for(STOP_FADE_MS);
                for v in &mut self.auditions {
                    v.fade_out(frames);
                }
            }
            Command::Audition { sample } => {
                let frames = self.frames_for(RESTART_FADE_MS);
                for v in &mut self.auditions {
                    v.fade_out(frames);
                }
                let slot = self
                    .auditions
                    .iter()
                    .position(|v| !v.is_active())
                    .unwrap_or_else(|| {
                        if self.auditions[0].age <= self.auditions[1].age {
                            0
                        } else {
                            1
                        }
                    });
                let params = PlayParams {
                    gain: 1.0,
                    pitch_semitones: 0.0,
                    start_frame: 0,
                    end_frame: sample.frames(),
                    choke_group: None,
                    mix_group: 0,
                    hold: false,
                };
                let ratio = sample.rate as f64 / self.rate as f64;
                self.auditions[slot].start(AUDITION_PAD, sample, params, ratio, self.audition_age);
                self.audition_age = self.audition_age.wrapping_add(1);
                self.audition_attack[slot] = 0;
            }
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
        self.keys_l[..n].fill(0.0);
        self.keys_r[..n].fill(0.0);
        self.instrument
            .render_add(&mut self.keys_l[..n], &mut self.keys_r[..n]);
        if let Some(gi) = self.keys_group {
            // Through the channel: its low-pass, pan, gain, mute, solo and level tap apply.
            let g = &mut self.groups[gi];
            for i in 0..n {
                g.buf_l[i] += self.keys_l[i] * self.keys_gain;
                g.buf_r[i] += self.keys_r[i] * self.keys_gain;
            }
        }
        let any_solo = self.groups.iter().any(|g| g.params.solo);
        out.fill(0.0);
        let mut levels = Levels::default();
        for (gi, g) in self.groups.iter_mut().enumerate() {
            let p = g.params;
            if p.mute || (any_solo && !p.solo) {
                continue;
            }
            let angle = (p.pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
            let gl = angle.cos() * SQRT_2 * p.gain;
            let gr = angle.sin() * SQRT_2 * p.gain;
            let mut peak = 0.0f32;
            for i in 0..n {
                let l = g.lp_l.process(g.buf_l[i]) * gl;
                let r = g.lp_r.process(g.buf_r[i]) * gr;
                out[2 * i] += l;
                out[2 * i + 1] += r;
                peak = peak.max(l.abs()).max(r.abs());
            }
            levels.groups[gi] = peak;
        }
        let attack_frames = self.frames_for(RESTART_FADE_MS).max(1);
        for (slot, voice) in self.auditions.iter_mut().enumerate() {
            if !voice.is_active() {
                continue;
            }
            self.audition_l[..n].fill(0.0);
            self.audition_r[..n].fill(0.0);
            voice.render_add(&mut self.audition_l[..n], &mut self.audition_r[..n]);
            for i in 0..n {
                let gain =
                    self.audition_attack[slot].min(attack_frames) as f32 / attack_frames as f32;
                self.audition_attack[slot] = (self.audition_attack[slot] + 1).min(attack_frames);
                out[2 * i] += self.audition_l[i] * gain;
                out[2 * i + 1] += self.audition_r[i] * gain;
            }
        }
        if self.keys_group.is_none() {
            // Straight to the master bus, like auditions: before master gain and the clipper.
            for i in 0..n {
                out[2 * i] += self.keys_l[i] * self.keys_gain;
                out[2 * i + 1] += self.keys_r[i] * self.keys_gain;
            }
        }
        for s in out.iter_mut() {
            *s = soft_clip(*s * self.master_gain);
        }
        if let Some(taps) = &mut self.taps {
            // A full queue drops the rest of the block; the reader catches up next time.
            let mut scope_open = true;
            for frame in out.chunks_exact(2) {
                levels.master[0] = levels.master[0].max(frame[0].abs());
                levels.master[1] = levels.master[1].max(frame[1].abs());
                if scope_open {
                    scope_open = taps.scope.push((frame[0] + frame[1]) * 0.5).is_ok();
                }
            }
            let _ = taps.levels.push(levels);
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
