//! The built-in keys synth: sixteen preallocated voices, six recipes (plan 1C Task 15).

use super::{Instrument, KeysSound, KEYS_VOICES};
use std::f32::consts::TAU;

/// Output headroom before the engine's keys gain.
const HEADROOM: f32 = 0.25;
/// Frames rendered with one pitch multiplier (bend and vibrato are stepped at this rate).
const CHUNK: usize = 16;
const VIBRATO_HZ: f32 = 5.5;
/// Vibrato depth at modulation 1.0, in semitones.
const VIBRATO_SEMITONES: f32 = 0.5;
const FAST_RELEASE_S: f32 = 0.005;
/// A decaying voice below this level is over.
const SILENCE: f32 = 1e-4;

#[derive(Clone, Copy)]
struct Env {
    attack: f32,
    /// Time for the level to fall to 1 % of the way to `sustain`; 0 = no decay stage.
    decay: f32,
    sustain: f32,
    release: f32,
}

fn env(sound: KeysSound) -> Env {
    let (attack, decay, sustain, release) = match sound {
        KeysSound::Piano => (0.002, 1.2, 0.0, 0.25),
        KeysSound::EPiano => (0.002, 1.5, 0.3, 0.4),
        KeysSound::Organ => (0.005, 0.0, 1.0, 0.005),
        KeysSound::SynthPad => (0.3, 0.0, 1.0, 1.2),
        KeysSound::SynthLead => (0.005, 0.2, 0.7, 0.15),
        KeysSound::SynthBass => (0.003, 0.3, 0.6, 0.08),
    };
    Env {
        attack,
        decay,
        sustain,
        release,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy)]
struct Voice {
    stage: Stage,
    level: f32,
    /// Per-sample change while attacking or releasing.
    step: f32,
    note: u8,
    sound: KeysSound,
    /// (velocity / 127)².
    gain: f32,
    age: u64,
    phase: [f32; 4],
    mod_phase: f32,
    lp: f32,
    /// Frames since the note started (the e-piano's modulation index follows it).
    frames: u32,
}

impl Voice {
    const fn idle() -> Self {
        Voice {
            stage: Stage::Idle,
            level: 0.0,
            step: 0.0,
            note: 0,
            sound: KeysSound::EPiano,
            gain: 0.0,
            age: 0,
            phase: [0.0; 4],
            mod_phase: 0.0,
            lp: 0.0,
            frames: 0,
        }
    }

    fn active(&self) -> bool {
        self.stage != Stage::Idle
    }

    fn release(&mut self, seconds: f32, rate: f32) {
        if !self.active() || self.stage == Stage::Release {
            return;
        }
        self.stage = Stage::Release;
        self.step = self.level / (seconds * rate).max(1.0);
    }

    /// The next envelope value.
    fn envelope(&mut self, e: &Env, decay_k: f32) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += self.step;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = if e.decay > 0.0 {
                        Stage::Decay
                    } else {
                        Stage::Sustain
                    };
                }
            }
            Stage::Decay => {
                self.level = e.sustain + (self.level - e.sustain) * decay_k;
                if self.level - e.sustain < SILENCE {
                    self.level = e.sustain;
                    self.stage = if e.sustain > 0.0 {
                        Stage::Sustain
                    } else {
                        Stage::Idle
                    };
                }
            }
            Stage::Sustain => self.level = e.sustain,
            Stage::Release => {
                self.level -= self.step;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}

fn polyblep(mut t: f32, dt: f32) -> f32 {
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

fn saw(phase: f32, dt: f32) -> f32 {
    2.0 * phase - 1.0 - polyblep(phase, dt)
}

fn square(phase: f32, dt: f32) -> f32 {
    let raw = if phase < 0.5 { 1.0 } else { -1.0 };
    raw + polyblep(phase, dt) - polyblep((phase + 0.5).fract(), dt)
}

fn advance(phase: &mut f32, inc: f32) {
    *phase += inc;
    if *phase >= 1.0 {
        *phase -= 1.0;
    }
}

pub struct BasicKeys {
    rate: f32,
    voices: [Voice; KEYS_VOICES],
    sound: KeysSound,
    next_age: u64,
    /// Pitch bend in semitones.
    bend: f32,
    depth: f32,
    lfo: f32,
    bass_a: f32,
}

impl BasicKeys {
    pub fn new(sample_rate: u32) -> Self {
        let rate = sample_rate as f32;
        BasicKeys {
            rate,
            voices: [Voice::idle(); KEYS_VOICES],
            sound: KeysSound::default(),
            next_age: 0,
            bend: 0.0,
            depth: 0.0,
            lfo: 0.0,
            bass_a: 1.0 - (-TAU * 800.0 / rate).exp(),
        }
    }

    /// Voices that are producing sound, including ones in their release.
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active()).count()
    }

    /// Is a voice playing `note` (held or still ringing)?
    pub fn is_sounding(&self, note: u8) -> bool {
        self.voices.iter().any(|v| v.active() && v.note == note)
    }

    fn start_voice(&mut self, note: u8, velocity: u8) {
        let slot = self
            .voices
            .iter()
            .position(|v| v.active() && v.note == note && v.stage != Stage::Release)
            .or_else(|| self.voices.iter().position(|v| !v.active()))
            .unwrap_or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.age)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            });
        let was_idle = !self.voices[slot].active();
        let v = &mut self.voices[slot];
        if was_idle {
            *v = Voice::idle();
        }
        v.note = note;
        v.sound = self.sound;
        let vel = f32::from(velocity.min(127)) / 127.0;
        v.gain = vel * vel;
        v.age = self.next_age;
        self.next_age += 1;
        v.frames = 0;
        let attack = (env(self.sound).attack * self.rate).max(1.0);
        // An attack always starts from the current level, so a stolen voice does not click.
        v.step = (1.0 - v.level) / attack;
        v.stage = Stage::Attack;
    }

    fn sample(&mut self, vi: usize, mul: f32, e: &Env) -> f32 {
        let rate = self.rate;
        let bass_a = self.bass_a;
        let decay_k = if e.decay > 0.0 {
            (-(100f32.ln()) / (e.decay * rate)).exp()
        } else {
            1.0
        };
        let v = &mut self.voices[vi];
        let level = v.envelope(e, decay_k);
        let f = 440.0 * ((f32::from(v.note) - 69.0) / 12.0).exp2() * mul / rate;
        let raw = match v.sound {
            KeysSound::Piano => {
                let mut s = 0.0;
                for (k, amp) in [1.0, 0.5, 0.25].iter().enumerate() {
                    advance(&mut v.phase[k], f * (k + 1) as f32);
                    s += amp * (TAU * v.phase[k]).sin();
                }
                s
            }
            KeysSound::EPiano => {
                let t = v.frames as f32 / rate;
                let index = (1.5 - 1.3 * (t / 0.4)).max(0.2);
                advance(&mut v.mod_phase, f);
                advance(&mut v.phase[0], f);
                (TAU * v.phase[0] + index * (TAU * v.mod_phase).sin()).sin()
            }
            KeysSound::Organ => {
                let mut s = 0.0;
                for (k, amp) in [1.0, 0.7, 0.4, 0.25].iter().enumerate() {
                    advance(&mut v.phase[k], f * (k + 1) as f32);
                    s += amp * (TAU * v.phase[k]).sin();
                }
                s
            }
            KeysSound::SynthPad => {
                let mut s = 0.0;
                for (k, cents) in [-7.0f32, 0.0, 7.0].iter().enumerate() {
                    let inc = f * (cents / 1200.0).exp2();
                    advance(&mut v.phase[k], inc);
                    s += 0.9 * saw(v.phase[k], inc);
                }
                s
            }
            KeysSound::SynthLead => {
                advance(&mut v.phase[0], f);
                square(v.phase[0], f) + 0.3 * saw(v.phase[0], f)
            }
            KeysSound::SynthBass => {
                let inc = f * 0.5;
                advance(&mut v.phase[0], inc);
                v.lp += bass_a * (saw(v.phase[0], inc) - v.lp);
                v.lp
            }
        };
        v.frames = v.frames.saturating_add(1);
        raw * level * v.gain * HEADROOM
    }
}

impl Instrument for BasicKeys {
    fn note_on(&mut self, note: u8, velocity: u8) {
        if velocity == 0 {
            self.note_off(note);
        } else {
            self.start_voice(note, velocity);
        }
    }

    fn note_off(&mut self, note: u8) {
        for v in &mut self.voices {
            if v.active() && v.note == note && v.stage != Stage::Release {
                v.release(env(v.sound).release, self.rate);
            }
        }
    }

    fn pitch_bend(&mut self, semitones: f32) {
        self.bend = semitones.clamp(-2.0, 2.0);
    }

    fn modulation(&mut self, depth: f32) {
        self.depth = depth.clamp(0.0, 1.0);
    }

    fn set_sound(&mut self, sound: KeysSound) {
        self.sound = sound;
    }

    fn all_notes_off(&mut self) {
        for v in &mut self.voices {
            v.release(FAST_RELEASE_S, self.rate);
        }
    }

    fn render_add(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let mut at = 0;
        while at < n {
            let len = CHUNK.min(n - at);
            let vibrato = (TAU * self.lfo).sin() * VIBRATO_SEMITONES * self.depth;
            let mul = ((self.bend + vibrato) / 12.0).exp2();
            self.lfo += VIBRATO_HZ * len as f32 / self.rate;
            self.lfo -= self.lfo.floor();
            for vi in 0..KEYS_VOICES {
                if !self.voices[vi].active() {
                    continue;
                }
                let e = env(self.voices[vi].sound);
                for i in at..at + len {
                    let s = self.sample(vi, mul, &e);
                    l[i] += s;
                    r[i] += s;
                }
            }
            at += len;
        }
    }
}
