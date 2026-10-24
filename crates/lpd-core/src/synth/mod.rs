//! The keys synthesizer (keys synth spec): a semi-modular wavetable synth behind `Instrument`.

pub mod env;
pub mod filter;
pub mod fx;
pub mod lfo;
pub mod matrix;
pub mod monitor;
pub mod osc;
pub mod params;
pub mod voice;
pub mod wavetable;

use crate::audio::{fader_gain, MAX_BLOCK};
use crate::instrument::{Instrument, KeysSound};
use env::TICK;
use lfo::{rate_hz, Lfo};
use matrix::{accumulate_global, base_norms, live_norm, Sources};
use monitor::SynthMonitor;
use osc::next_u32;
use params::*;
use voice::{Rendered, Voice, VoiceCtx};

pub const VOICES: usize = 16;
const FADE_SLOTS: usize = 4;
const STEAL_FADE_S: f32 = 0.003;
const LOAD_FADE_S: f32 = 0.005;
const SMOOTH_S: f32 = 0.010;
/// The voice sum × this before the FX (−12 dB headroom, as the old synth had).
const HEADROOM: f32 = 0.25;
const DEFERRED: usize = 16;

/// Held keys for MONO / LEGATO (last-note priority).
#[derive(Clone, Copy, Default)]
struct NoteStack {
    notes: [u8; 16],
    len: usize,
}

impl NoteStack {
    fn push(&mut self, n: u8) {
        self.remove(n);
        if self.len == self.notes.len() {
            self.notes.copy_within(1.., 0);
            self.len -= 1;
        }
        self.notes[self.len] = n;
        self.len += 1;
    }

    fn remove(&mut self, n: u8) {
        if let Some(i) = self.notes[..self.len].iter().position(|&x| x == n) {
            self.notes.copy_within(i + 1..self.len, i);
            self.len -= 1;
        }
    }

    fn top(&self) -> Option<u8> {
        self.len.checked_sub(1).map(|i| self.notes[i])
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn clear(&mut self) {
        self.len = 0;
    }
}

pub struct Synth {
    rate: f32,
    params: SynthParams,
    /// Knob positions of the destinations (targets) and their smoothed values.
    base: [f32; DESTS],
    smooth: [f32; DESTS],
    voices: [Voice; VOICES],
    /// Stolen voices fading out over 3 ms: (voice, gain).
    fades: [(Voice, f32); FADE_SLOTS],
    stack: NoteStack,
    pedal: bool,
    wheel: f32,
    bend: f32,
    tempo: f32,
    lfo: [Lfo; LFOS],
    free_lfo: [f32; LFOS],
    last_note: Option<f32>,
    pending: Option<SynthParams>,
    deferred: [Option<(u8, u8)>; DEFERRED],
    load_fade: usize,
    load_fade_len: usize,
    bus_l: Vec<f32>,
    bus_r: Vec<f32>,
    volume: f32,
    vol_k: f32,
    monitor: SynthMonitor,
    fx: fx::FxChain,
    next_age: u64,
    resets: u32,
}

impl Synth {
    /// Allocates everything the audio thread will use. Call it off the audio thread.
    pub fn new(sample_rate: u32) -> Synth {
        let rate = sample_rate.max(1) as f32;
        let params = SynthParams::default();
        let base = base_norms(&params);
        let mut seed = 0x1234_5679u32;
        let voices = std::array::from_fn(|_| Voice::idle(next_u32(&mut seed)));
        let fades = std::array::from_fn(|_| (Voice::idle(next_u32(&mut seed)), 0.0));
        Synth {
            rate,
            params,
            base,
            smooth: base,
            voices,
            fades,
            stack: NoteStack::default(),
            pedal: false,
            wheel: 0.0,
            bend: 0.0,
            tempo: 120.0,
            lfo: [Lfo::new(11), Lfo::new(23)],
            free_lfo: [0.0; LFOS],
            last_note: None,
            pending: None,
            deferred: [None; DEFERRED],
            load_fade: 0,
            load_fade_len: ((LOAD_FADE_S * rate) as usize).max(1),
            bus_l: vec![0.0; MAX_BLOCK],
            bus_r: vec![0.0; MAX_BLOCK],
            volume: fader_gain(params.voice.volume_db),
            vol_k: 1.0 - (-1.0 / (SMOOTH_S * rate)).exp(),
            monitor: SynthMonitor::default(),
            fx: fx::FxChain::new(rate),
            next_age: 0,
            resets: 0,
        }
    }

    pub fn params(&self) -> &SynthParams {
        &self.params
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active()).count()
    }

    /// A voice is playing `note` (held, sustained or releasing).
    pub fn is_sounding(&self, note: u8) -> bool {
        self.voices.iter().any(|v| v.active() && v.note == note)
    }

    /// Notes of voices whose key is down (tests; allocates).
    pub fn sounding_notes(&self) -> Vec<u8> {
        self.voices
            .iter()
            .filter(|v| v.active() && v.gate)
            .map(|v| v.note)
            .collect()
    }

    /// The glided pitch of the voice playing `note`.
    pub fn voice_pitch(&self, note: u8) -> Option<f32> {
        self.voices
            .iter()
            .find(|v| v.active() && v.note == note)
            .map(|v| v.pitch)
    }

    #[doc(hidden)]
    pub fn debug_poison_voice(&mut self, i: usize) {
        self.voices[i].debug_poison();
    }

    fn bump_age(&mut self) -> u64 {
        self.next_age += 1;
        self.next_age
    }

    fn poly_on(&mut self, note: u8, velocity: u8) {
        let age = self.bump_age();
        if let Some(i) = self
            .voices
            .iter()
            .position(|v| v.active() && v.note == note && (v.gate || v.sustained))
        {
            self.voices[i].retarget(&self.params, note, Some(velocity), true, false, age);
            return;
        }
        let i = match self.voices.iter().position(|v| !v.active()) {
            Some(i) => i,
            None => {
                let i = self.steal_index();
                self.fade_out(i);
                i
            }
        };
        let glide_from = self.last_note;
        self.voices[i].start(&self.params, note, velocity, age, glide_from);
    }

    /// The oldest voice whose key is up and not sustained, else the oldest voice.
    fn steal_index(&self) -> usize {
        let oldest = |released: bool| {
            self.voices
                .iter()
                .enumerate()
                .filter(|(_, v)| !released || (!v.gate && !v.sustained))
                .min_by_key(|(_, v)| v.age)
                .map(|(i, _)| i)
        };
        oldest(true).or_else(|| oldest(false)).unwrap_or(0)
    }

    fn fade_out(&mut self, i: usize) {
        let slot = self
            .fades
            .iter()
            .position(|(_, g)| *g <= 0.0)
            .or_else(|| {
                self.fades
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1 .1.total_cmp(&b.1 .1))
                    .map(|(i, _)| i)
            })
            .unwrap_or(0);
        self.fades[slot] = (self.voices[i], 1.0);
    }

    fn mono_on(&mut self, note: u8, velocity: u8) {
        let held = !self.stack.is_empty();
        self.stack.push(note);
        let age = self.bump_age();
        let glide = held || self.params.voice.glide_mode == GlideMode::Always;
        let legato = self.params.voice.mode == VoiceMode::Legato && held;
        let from = if glide { self.last_note } else { None };
        let (params, v) = (&self.params, &mut self.voices[0]);
        if v.active() {
            v.retarget(
                params,
                note,
                (!legato).then_some(velocity),
                !legato,
                glide,
                age,
            );
        } else {
            v.start(params, note, velocity, age, from);
        }
    }

    fn mono_off(&mut self, note: u8) {
        let was_top = self.stack.top() == Some(note);
        self.stack.remove(note);
        if !was_top {
            return;
        }
        let age = self.bump_age();
        let pedal = self.pedal;
        let (params, v) = (&self.params, &mut self.voices[0]);
        if !v.active() || !v.gate {
            return;
        }
        match self.stack.top() {
            Some(prev) => v.retarget(params, prev, None, false, true, age),
            None if pedal => {
                v.gate = false;
                v.sustained = true;
            }
            None => v.release(),
        }
    }

    fn poly_off(&mut self, note: u8) {
        let pedal = self.pedal;
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active() && v.note == note && v.gate)
        {
            if pedal {
                v.gate = false;
                v.sustained = true;
            } else {
                v.release();
            }
        }
    }

    fn render_chunk(&mut self, at: usize, len: usize) {
        let dt = len as f32 / self.rate;
        let k = 1.0 - (-dt / SMOOTH_S).exp();
        for (s, b) in self.smooth.iter_mut().zip(&self.base) {
            *s += (b - *s) * k;
        }
        // LFOs in FREE mode run once for every voice; their RATE takes only global modulation.
        let mut global: Sources = [0.0; SOURCES];
        global[Source::Wheel as usize] = self.wheel;
        global[Source::Bend as usize] = self.bend;
        global[Source::Lfo1 as usize] = self.free_lfo[0];
        global[Source::Lfo2 as usize] = self.free_lfo[1];
        for i in 0..LFOS {
            let lp = self.params.lfo[i];
            if lp.mode != LfoMode::Free {
                continue;
            }
            let d = if i == 0 {
                Dest::Lfo1Rate
            } else {
                Dest::Lfo2Rate
            };
            let m = accumulate_global(&self.params.routes, &self.params, &global, i);
            let hz = ParamId::Lfo1Rate
                .meta()
                .kind
                .from_norm(live_norm(self.smooth[d as usize], m));
            self.free_lfo[i] = self.lfo[i].tick(&lp, rate_hz(&lp, hz, self.tempo), dt);
        }
        let ctx = VoiceCtx {
            p: &self.params,
            base: &self.smooth,
            rate: self.rate,
            wheel: self.wheel,
            bend: self.bend,
            tempo: self.tempo,
            free_lfo: self.free_lfo,
        };
        let bl = &mut self.bus_l[at..at + len];
        let br = &mut self.bus_r[at..at + len];
        for v in self.voices.iter_mut().filter(|v| v.active()) {
            if v.render(&ctx, len, bl, br) == Rendered::Reset {
                self.resets += 1;
            }
        }
        let step = 1.0 / (STEAL_FADE_S * self.rate);
        for (v, gain) in self.fades.iter_mut().filter(|(_, g)| *g > 0.0) {
            let mut tl = [0.0f32; TICK];
            let mut tr = [0.0f32; TICK];
            if v.render(&ctx, len, &mut tl[..len], &mut tr[..len]) != Rendered::Playing {
                *gain = 0.0;
                continue;
            }
            for i in 0..len {
                *gain = (*gain - step).max(0.0);
                bl[i] += tl[i] * *gain;
                br[i] += tr[i] * *gain;
            }
        }
    }

    fn apply_pending(&mut self) {
        let Some(p) = self.pending.take() else { return };
        self.params = p;
        self.base = base_norms(&p);
        self.smooth = self.base;
        for v in &mut self.voices {
            v.silence();
        }
        for (_, g) in &mut self.fades {
            *g = 0.0;
        }
        self.fx.clear();
        self.stack.clear();
        self.lfo = [Lfo::new(11), Lfo::new(23)];
        self.free_lfo = [0.0; LFOS];
        self.volume = fader_gain(p.voice.volume_db);
        let deferred = std::mem::replace(&mut self.deferred, [None; DEFERRED]);
        for (note, velocity) in deferred.into_iter().flatten() {
            self.note_on(note, velocity);
        }
    }

    fn update_monitor(&mut self) {
        let m = &mut self.monitor;
        m.voices = self.voices.iter().filter(|v| v.active()).count() as u8;
        m.resets = self.resets;
        m.filter = self.params.filter.kind;
        let free = |i: usize, lfo: &Lfo| [self.free_lfo[i], lfo.phase];
        match self
            .voices
            .iter()
            .filter(|v| v.active())
            .max_by_key(|v| v.age)
        {
            Some(v) => {
                m.sources = v.src;
                m.live = v.live;
                for i in 0..ENVS {
                    m.env[i] = [v.env[i].level, f32::from(v.env[i].stage_index())];
                }
                for i in 0..LFOS {
                    m.lfo[i] = if self.params.lfo[i].mode == LfoMode::Free {
                        free(i, &self.lfo[i])
                    } else {
                        [v.lfo[i].value, v.lfo[i].phase]
                    };
                }
                m.cutoff_hz = v.cutoff_hz;
                m.reso = v.live[Dest::Reso as usize];
                m.drive = v.live[Dest::Drive as usize];
            }
            None => {
                m.sources = [0.0; SOURCES];
                m.sources[Source::Wheel as usize] = self.wheel;
                m.sources[Source::Bend as usize] = self.bend;
                m.live = self.smooth;
                for d in [Dest::OscAPitch, Dest::OscBPitch, Dest::OscPitch] {
                    m.live[d as usize] = 0.5;
                }
                m.env = [[0.0; 2]; ENVS];
                for i in 0..LFOS {
                    let is_free = self.params.lfo[i].mode == LfoMode::Free;
                    m.lfo[i] = if is_free {
                        free(i, &self.lfo[i])
                    } else {
                        [0.0; 2]
                    };
                    if is_free {
                        m.sources[Source::Lfo1 as usize + i] = self.free_lfo[i];
                    }
                }
                m.cutoff_hz = self.params.filter.cutoff;
                m.reso = self.params.filter.reso;
                m.drive = self.params.filter.drive;
            }
        }
    }
}

impl Instrument for Synth {
    fn note_on(&mut self, note: u8, velocity: u8) {
        if velocity == 0 {
            return self.note_off(note);
        }
        if self.pending.is_some() {
            if let Some(slot) = self.deferred.iter_mut().find(|s| s.is_none()) {
                *slot = Some((note, velocity));
            }
            return;
        }
        match self.params.voice.mode {
            VoiceMode::Poly => self.poly_on(note, velocity),
            VoiceMode::Mono | VoiceMode::Legato => self.mono_on(note, velocity),
        }
        self.last_note = Some(f32::from(note));
    }

    fn note_off(&mut self, note: u8) {
        for slot in self.deferred.iter_mut() {
            if matches!(slot, Some((n, _)) if *n == note) {
                *slot = None;
            }
        }
        match self.params.voice.mode {
            VoiceMode::Poly => self.poly_off(note),
            VoiceMode::Mono | VoiceMode::Legato => self.mono_off(note),
        }
    }

    /// `wheel` −1…1; BEND RANGE applies in the voices.
    fn pitch_bend(&mut self, wheel: f32) {
        self.bend = wheel.clamp(-1.0, 1.0);
    }

    fn modulation(&mut self, depth: f32) {
        self.wheel = depth.clamp(0.0, 1.0);
    }

    /// The synth plays patches, not GM sounds.
    fn set_sound(&mut self, _sound: KeysSound) {}

    fn all_notes_off(&mut self) {
        for v in &mut self.voices {
            v.kill();
        }
        self.stack.clear();
        self.pedal = false;
        self.deferred = [None; DEFERRED];
    }

    fn render_add(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len()).min(self.bus_l.len());
        self.bus_l[..n].fill(0.0);
        self.bus_r[..n].fill(0.0);
        let mut at = 0;
        while at < n {
            let len = TICK.min(n - at);
            self.render_chunk(at, len);
            at += len;
        }
        for i in 0..n {
            self.bus_l[i] *= HEADROOM;
            self.bus_r[i] *= HEADROOM;
        }
        self.fx.process(
            &mut self.bus_l[..n],
            &mut self.bus_r[..n],
            &self.params.fx,
            self.tempo,
        );
        let target = fader_gain(self.params.voice.volume_db);
        let fading = self.pending.is_some();
        for i in 0..n {
            self.volume += (target - self.volume) * self.vol_k;
            let mut g = self.volume;
            if fading {
                if self.load_fade > 0 {
                    g *= self.load_fade as f32 / self.load_fade_len as f32;
                    self.load_fade -= 1;
                } else {
                    g = 0.0;
                }
            }
            l[i] += self.bus_l[i] * g;
            r[i] += self.bus_r[i] * g;
        }
        if fading && self.load_fade == 0 {
            self.apply_pending();
        }
        self.update_monitor();
    }

    fn set_param(&mut self, id: ParamId, value: f32) {
        match &mut self.pending {
            Some(p) => id.set(p, value),
            None => {
                id.set(&mut self.params, value);
                self.base = base_norms(&self.params);
            }
        }
    }

    fn set_route(&mut self, slot: usize, route: Option<Route>) {
        match &mut self.pending {
            Some(p) => p.routes.set(slot, route),
            None => self.params.routes.set(slot, route),
        };
    }

    fn load_params(&mut self, params: &SynthParams) {
        self.pending = Some(*params);
        self.load_fade = self.load_fade_len;
    }

    fn sustain(&mut self, down: bool) {
        self.pedal = down;
        if !down {
            for v in self.voices.iter_mut().filter(|v| v.sustained) {
                v.release();
            }
        }
    }

    fn set_tempo(&mut self, bpm: f32) {
        self.tempo = bpm.clamp(20.0, 400.0);
    }

    fn monitor(&self) -> Option<SynthMonitor> {
        Some(self.monitor)
    }
}
