//! Dustline's live ship engine voice (spec `2026-11-05-dustline-ship-engine-design.md`, section 5.1).
//!
//! A second, private `Synth` holds one note while the game streams frames, one per engine control step (about
//! 1/40 s: 30 Hz at a 60 Hz display, 36 Hz at 144 Hz, 40 Hz offline): `bend` (−1…1 of a forced 24-semitone range),
//! `wheel` (0…1, the patch's WHEEL source) and `gain`. Bend and wheel glide to each frame evenly over 40 ms, longer
//! than any gap between frames, so the pitch never stalls between them; the gain ramps over 25 ms, so shift cuts stay
//! crisp. Both step once per 16-sample synth tick, and the gain moves per sample inside a tick.
//! With no frame for 250 ms the voice fades out over 50 ms and releases its note. Built off the audio thread; on it,
//! it never allocates. The user's Keys synth is a different `Synth` and is never touched.

use super::env::TICK;
use super::params::{SynthParams, VoiceMode};
use super::Synth;
use crate::instrument::Instrument;

/// The held note. It sounds twice the ship's fundamental, so the game measures its bend from 65.41 Hz (MIDI 36).
pub const GAME_NOTE: u8 = 48;
pub const GAME_VELOCITY: u8 = 127;
pub const GAME_BEND_RANGE: u8 = 24;
pub const GAME_RAMP_S: f32 = 0.025;
/// Bend and wheel glide time: longer than the longest gap between frames (1/30 s at a 60 Hz display).
pub const GAME_GLIDE_S: f32 = 0.040;
pub const GAME_WATCHDOG_S: f32 = 0.25;
/// One voice stands for both of the ship's engines (spec 5.4): BAKED sums two engine voices, each at the profile's tone
/// level, so the voice plays at twice its frame gain (+6 dB) to match them at the same gain.
pub const GAME_ENGINES_GAIN: f32 = 2.0;
pub const GAME_FADE_S: f32 = 0.05;

/// One frame from the game. `on: false` fades the voice out over one ramp and releases the note.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameFrame {
    pub on: bool,
    pub bend: f32,
    pub wheel: f32,
    pub gain: f32,
}

/// A value that moves to its target in equal steps, one per tick, and lands on it exactly.
#[derive(Debug, Clone, Copy)]
struct Ramp {
    value: f32,
    target: f32,
    step: f32,
    left: u32,
}

impl Ramp {
    fn at(v: f32) -> Ramp {
        Ramp {
            value: v,
            target: v,
            step: 0.0,
            left: 0,
        }
    }
    fn to(&mut self, target: f32, ticks: u32) {
        self.target = target;
        self.left = ticks.max(1);
        self.step = (target - self.value) / self.left as f32;
    }
    fn tick(&mut self) -> f32 {
        if self.left > 0 {
            self.left -= 1;
            self.value = if self.left == 0 {
                self.target
            } else {
                self.value + self.step
            };
        }
        self.value
    }
    fn settled(&self) -> bool {
        self.left == 0
    }
}

/// A finite value clamped to `lo..=hi`; anything else keeps `keep`.
fn clean(v: f32, lo: f32, hi: f32, keep: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        keep
    }
}

pub struct GameEngineVoice {
    synth: Synth,
    ramp_ticks: u32,
    glide_ticks: u32,
    fade_ticks: u32,
    watchdog: usize,
    bend: Ramp,
    wheel: Ramp,
    gain: Ramp,
    /// 1 while frames arrive; the watchdog and `stop` take it to 0.
    fade: Ramp,
    /// The output gain reached at the end of the last tick (gain × fade).
    level: f32,
    since_frame: usize,
    want_on: bool,
    held: bool,
    stopping: bool,
    finished: bool,
    l: [f32; TICK],
    r: [f32; TICK],
}

impl std::fmt::Debug for GameEngineVoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GameEngineVoice")
    }
}

impl GameEngineVoice {
    /// Builds the voice and applies `patch` with LEGATO and a 24-semitone bend range forced. Allocates: call it off
    /// the audio thread.
    pub fn new(sample_rate: u32, patch: &SynthParams) -> Box<GameEngineVoice> {
        let rate = sample_rate.max(1) as f32;
        let mut params = *patch;
        params.voice.mode = VoiceMode::Legato;
        params.voice.bend_range = GAME_BEND_RANGE;
        let mut synth = Synth::new(sample_rate);
        synth.load_params(&params);
        // Play out the 5 ms load fade now, so the first note is not held back.
        let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
        for _ in 0..4 {
            synth.render_add(&mut l, &mut r);
        }
        let ticks = |s: f32| ((s * rate / TICK as f32).round() as u32).max(1);
        Box::new(GameEngineVoice {
            synth,
            ramp_ticks: ticks(GAME_RAMP_S),
            glide_ticks: ticks(GAME_GLIDE_S),
            fade_ticks: ticks(GAME_FADE_S),
            watchdog: (GAME_WATCHDOG_S * rate) as usize,
            bend: Ramp::at(0.0),
            wheel: Ramp::at(0.0),
            gain: Ramp::at(0.0),
            fade: Ramp::at(0.0),
            level: 0.0,
            since_frame: 0,
            want_on: false,
            held: false,
            stopping: false,
            finished: false,
            l: [0.0; TICK],
            r: [0.0; TICK],
        })
    }

    /// The patch as the voice plays it.
    pub fn params(&self) -> &SynthParams {
        self.synth.params()
    }

    /// The ramped values at the end of the last tick (`on`: the note is held).
    pub fn current(&self) -> GameFrame {
        GameFrame {
            on: self.held,
            bend: self.bend.value,
            wheel: self.wheel.value,
            gain: self.level,
        }
    }

    /// The voice has faded out after `stop` and can go back through the trash.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// One 40 Hz frame. Non-finite values keep the previous target; the rest is clamped.
    pub fn frame(&mut self, f: GameFrame) {
        if self.stopping {
            return;
        }
        self.since_frame = 0;
        let bend = clean(f.bend, -1.0, 1.0, self.bend.target);
        let wheel = clean(f.wheel, 0.0, 1.0, self.wheel.target);
        let gain = clean(f.gain, 0.0, 1.0, 0.0);
        self.want_on = f.on;
        if f.on && !self.held {
            // A new note starts at the frame's pitch and wheel and fades in over one ramp.
            self.bend = Ramp::at(bend);
            self.wheel = Ramp::at(wheel);
            self.gain = Ramp::at(0.0);
            self.gain.to(gain, self.ramp_ticks);
            self.fade = Ramp::at(1.0);
            self.synth.pitch_bend(bend);
            self.synth.modulation(wheel);
            self.synth.note_on(GAME_NOTE, GAME_VELOCITY);
            self.held = true;
            return;
        }
        self.bend.to(bend, self.glide_ticks);
        self.wheel.to(wheel, self.glide_ticks);
        self.gain.to(if f.on { gain } else { 0.0 }, self.ramp_ticks);
        if f.on && self.fade.target < 1.0 {
            self.fade.to(1.0, self.ramp_ticks);
        }
    }

    /// Fade out over 50 ms, release the note and finish.
    pub fn stop(&mut self) {
        if !self.stopping {
            self.stopping = true;
            self.fade.to(0.0, self.fade_ticks);
        }
    }

    /// Adds into `l` and `r` (same length, at most `MAX_BLOCK`). Never allocates.
    pub fn render_add(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        if self.finished {
            return;
        }
        let mut at = 0;
        while at < n {
            let len = TICK.min(n - at);
            self.synth.pitch_bend(self.bend.tick());
            self.synth.modulation(self.wheel.tick());
            let g0 = self.level;
            let g1 = self.gain.tick() * self.fade.tick();
            self.l[..len].fill(0.0);
            self.r[..len].fill(0.0);
            self.synth
                .render_add(&mut self.l[..len], &mut self.r[..len]);
            for j in 0..len {
                let g = (g0 + (g1 - g0) * (j + 1) as f32 / len as f32) * GAME_ENGINES_GAIN;
                l[at + j] += self.l[j] * g;
                r[at + j] += self.r[j] * g;
            }
            self.level = g1;
            at += len;
        }
        self.since_frame = self.since_frame.saturating_add(n);
        if self.since_frame >= self.watchdog && self.fade.target > 0.0 {
            self.fade.to(0.0, self.fade_ticks);
        }
        let silent = self.level == 0.0 && self.gain.settled() && self.fade.settled();
        if self.held && silent && (!self.want_on || self.fade.target == 0.0) {
            self.synth.note_off(GAME_NOTE);
            self.held = false;
        }
        if self.stopping && self.fade.settled() {
            self.synth.all_notes_off();
            self.held = false;
            self.finished = true;
        }
    }
}
