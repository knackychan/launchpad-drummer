//! Router: single owner of set state, bank selection, and high-priority MIDI-to-audio dispatch.

use crate::devices::MidiMsg;
use crate::lights::PaintSnapshot;
use crate::sequencer::{Fired, SeqState};
use lpd_core::audio::{db_to_gain, fader_gain, Command, Event, Sample, Trash};
use lpd_core::devices::launchpad::LpEvent;
use lpd_core::devices::lcxl::{self, LcxlEvent};
use lpd_core::instrument::Instrument;
use lpd_core::kit::learn::{self, Binding, LearnKind, ParamValue, Target};
use lpd_core::kit::{self, Set};
use lpd_core::seq::VEL_ACCENT;
use lpd_core::synth::params::{ParamId, Route};
use rtrb::{Consumer, Producer};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const TOP_MIXER: u8 = 7;

/// Something the UI should hear about, produced under the router lock and emitted by the caller
/// after the lock is released.
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    KeysNote {
        note: u8,
        velocity: u8,
        on: bool,
    },
    KeysWheel {
        pitch: f32,
        r#mod: f32,
    },
    MidiLog {
        text: String,
    },
    /// A learned note played a pad (the UI selects it, never re-triggers it).
    PadHit {
        bank: u8,
        x: u8,
        y: u8,
    },
    LearnBound {
        binding: Binding,
    },
    LearnIgnored {
        reason: String,
    },
}

impl UiEvent {
    /// The Tauri event name (plan 0.4).
    pub fn name(&self) -> &'static str {
        match self {
            Self::KeysNote { .. } => "keys-note",
            Self::KeysWheel { .. } => "keys-wheel",
            Self::MidiLog { .. } => "midi-log",
            Self::PadHit { .. } => "pad-hit",
            Self::LearnBound { .. } => "learn-bound",
            Self::LearnIgnored { .. } => "learn-ignored",
        }
    }

    pub fn payload(&self) -> serde_json::Value {
        match self {
            Self::KeysNote { note, velocity, on } => {
                serde_json::json!({ "note": note, "velocity": velocity, "on": on })
            }
            Self::KeysWheel { pitch, r#mod } => serde_json::json!({ "pitch": pitch, "mod": r#mod }),
            Self::MidiLog { text } => serde_json::json!({ "text": text }),
            Self::PadHit { bank, x, y } => serde_json::json!({ "bank": bank, "x": x, "y": y }),
            Self::LearnBound { binding } => serde_json::json!({ "binding": binding }),
            Self::LearnIgnored { reason } => serde_json::json!({ "reason": reason }),
        }
    }
}

/// What the next learnable input will be bound to (JS: `{ pad: { bank, x, y } }` or `{ param: id }`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LearnTarget {
    Pad { bank: u8, x: u8, y: u8 },
    Param(String),
}

/// What one MIDI message did, for `AppState::on_midi` to finish once it holds the router lock:
/// set edits go through the history path, which the router cannot reach.
#[derive(Debug, Default, PartialEq)]
pub struct MidiOutcome {
    /// The event for the UI (keys notes and wheels, pad hits).
    pub event: Option<UiEvent>,
    /// Learn: add this binding (an undoable edit) and tell the UI.
    pub bind: Option<Binding>,
    /// A learned note played this pad `(bank, x, y)`: the light painter flashes it.
    pub hit: Option<(usize, u8, u8)>,
    /// A learned CC changed the mixer (already applied).
    pub mixer_changed: bool,
    /// A learned CC changed a set parameter: apply it with `learn:{id}` history coalescing.
    pub set_param: Option<(String, ParamValue)>,
}

#[derive(Default, Debug, PartialEq)]
pub struct LcxlOutcome {
    pub mixer: bool,
    pub synth: Option<(ParamId, f32)>,
}

/// A refused set replacement: nothing changed and the inputs are handed back.
#[derive(Debug)]
pub struct Rejected {
    pub reason: String,
    pub set: Set,
    pub samples: HashMap<String, Arc<Sample>>,
}

pub struct Router {
    set: Set,
    samples: HashMap<String, Arc<Sample>>,
    /// Samples no longer reachable from the caches. The audio thread may still hold the last
    /// strong reference, so they are freed here, off the audio thread, once nothing else does.
    retired: Vec<Arc<Sample>>,
    auditions: Vec<Arc<Sample>>,
    bank: usize,
    /// Held pads by pad id (bank * 64 + y * 8 + x).
    held: HashSet<u16>,
    voice_counts: HashMap<u16, usize>,
    audio_tx: Producer<Command>,
    audio_rx: Consumer<Event>,
    /// xorshift32 state for humanize; never 0.
    rng: u32,
    /// Last wheel positions of the keys inputs, so each `keys-wheel` event carries both.
    keys_pitch: f32,
    keys_mod: f32,
    keys_sustain: bool,
    learn_armed: Option<LearnTarget>,
    /// `LearnIgnored` is sent once per arm.
    learn_ignored_sent: bool,
    /// Last CC value per (device, cc), for the toggle's rising edge.
    last_cc: Vec<(String, u8, u8)>,
    /// The sequencer clock state, shared with the clock thread (a leaf below the router lock).
    seq: Arc<Mutex<SeqState>>,
    /// Replaced keys instruments coming back from the audio thread; dropped off it (see `drain_trash`).
    trash: Option<Consumer<Trash>>,
}

impl Router {
    pub fn new(
        set: Set,
        samples: HashMap<String, Arc<Sample>>,
        mut audio_tx: Producer<Command>,
        audio_rx: Consumer<Event>,
    ) -> Self {
        Self::send_mixer_commands(&set, &mut audio_tx);
        let mut router = Self {
            set,
            samples,
            retired: Vec::new(),
            auditions: Vec::with_capacity(2),
            bank: 0,
            held: HashSet::new(),
            voice_counts: HashMap::new(),
            audio_tx,
            audio_rx,
            rng: time_seed(),
            keys_pitch: 0.0,
            keys_mod: 0.0,
            keys_sustain: false,
            learn_armed: None,
            learn_ignored_sent: false,
            last_cc: Vec::new(),
            seq: Arc::default(),
            trash: None,
        };
        router.send_keys();
        router.send_patch();
        router
    }

    /// The sequencer state, for `AppState` and the clock thread.
    pub fn seq_handle(&self) -> Arc<Mutex<SeqState>> {
        self.seq.clone()
    }

    /// Fix the humanize random sequence (tests).
    pub fn set_seed(&mut self, seed: u32) {
        self.rng = seed.max(1);
    }

    /// Uniform in [-1, 1).
    fn next_unit(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }

    pub fn bank(&self) -> usize {
        self.bank
    }

    pub fn set(&self) -> &Set {
        &self.set
    }

    pub fn set_mut(&mut self) -> &mut Set {
        &mut self.set
    }

    pub fn samples(&self) -> &HashMap<String, Arc<Sample>> {
        &self.samples
    }

    pub fn audition(&mut self, sample: Arc<Sample>) {
        if self.auditions.len() == 2 {
            let old = self.auditions.remove(0);
            // A queued audition may still own it even after two newer requests.
            self.retire([old]);
        }
        self.auditions.push(sample.clone());
        let _ = self.audio_tx.push(Command::Audition { sample });
    }

    pub fn stop_audition(&mut self) {
        let _ = self.audio_tx.push(Command::StopAudition);
    }

    /// Put a decoded sample into the active cache; the entry it replaces is retired.
    pub fn install_sample(&mut self, path: String, sample: Arc<Sample>) {
        if let Some(old) = self.samples.insert(path, sample) {
            self.retire([old]);
        }
    }

    /// Keep `samples` alive until the audio side lets go of them (see `drain_reclaimable`).
    pub fn retire(&mut self, samples: impl IntoIterator<Item = Arc<Sample>>) {
        for s in samples {
            if !self.retired.iter().any(|r| Arc::ptr_eq(r, &s)) {
                self.retired.push(s);
            }
        }
    }

    /// Retired samples nothing else references. The caller drops them after releasing the router
    /// lock; the strong count can only fall (the audio thread never clones), so 1 is final.
    pub fn drain_reclaimable(&mut self) -> Vec<Arc<Sample>> {
        let (free, keep) = std::mem::take(&mut self.retired)
            .into_iter()
            .partition(|s| Arc::strong_count(s) == 1);
        self.retired = keep;
        free
    }

    pub fn retired_count(&self) -> usize {
        self.retired.len()
    }

    pub fn update_group(&mut self, group: usize, g: kit::Group) {
        if group < self.set.mixer.groups.len() {
            let params = kit::group_params(&g);
            self.set.mixer.groups[group] = g;
            let _ = self.audio_tx.push(Command::SetGroup {
                group: group as u8,
                params,
            });
        }
    }

    pub fn update_master_db(&mut self, master_db: f32) {
        self.set.mixer.master_db = master_db;
        let _ = self
            .audio_tx
            .push(Command::SetMasterGain(fader_gain(master_db)));
    }

    pub fn pad_id(bank: usize, x: u8, y: u8) -> u16 {
        (bank * 64 + y as usize * 8 + x as usize) as u16
    }

    pub fn is_pad_sounding(&self, bank: usize, x: u8, y: u8) -> bool {
        let id = Self::pad_id(bank, x, y);
        self.voice_counts.get(&id).copied().unwrap_or(0) > 0
    }

    pub fn poll_audio_events(&mut self) {
        while let Ok(evt) = self.audio_rx.pop() {
            match evt {
                Event::VoiceStarted { pad } => {
                    *self.voice_counts.entry(pad).or_insert(0) += 1;
                }
                Event::VoiceEnded { pad } => {
                    if let Some(count) = self.voice_counts.get_mut(&pad) {
                        if *count > 1 {
                            *count -= 1;
                        } else {
                            self.voice_counts.remove(&pad);
                        }
                    }
                }
            }
        }
    }

    /// Swap in another set. Nothing changes unless the audio queue has room for the whole
    /// transition, so the stop below cannot be lost. Voices of the old set fade out (StopAll is
    /// queued ahead of any trigger for the new set); their samples are retired, never freed here.
    /// Voice counters stay as they are so in-flight start/end events remain balanced.
    pub fn replace_set(
        &mut self,
        set: Set,
        samples: HashMap<String, Arc<Sample>>,
    ) -> Result<(), Box<Rejected>> {
        let needed = 1 + set.mixer.groups.len() + 1 + 3 + 1;
        if self.audio_tx.slots() < needed {
            return Err(Box::new(Rejected {
                reason: "The audio engine is busy. Try again in a moment.".into(),
                set,
                samples,
            }));
        }
        let _ = self.audio_tx.push(Command::StopAll);
        Self::send_mixer_commands(&set, &mut self.audio_tx);
        self.held.clear();
        self.set = set;
        self.send_keys();
        self.send_patch();
        let old = std::mem::replace(&mut self.samples, samples);
        self.retire(old.into_values());
        Ok(())
    }

    /// A new output stream is running: swap in its queues. The old engine is gone with its voices,
    /// so voice counts and holds are cleared, and the new engine gets the mixer. Nothing else is
    /// sent. The audio owner thread calls this after the old stream was dropped.
    pub fn replace_audio(&mut self, tx: Producer<Command>, rx: Consumer<Event>) {
        self.audio_tx = tx;
        self.audio_rx = rx;
        self.forget_voices();
        self.send_mixer();
        self.send_keys();
        self.send_patch();
        self.send_wheels();
    }

    /// The consumer of instruments the audio thread replaced (from the new stream's engine).
    pub fn set_trash(&mut self, trash: Option<Consumer<Trash>>) {
        self.trash = trash;
    }

    /// Replaced instruments, for the caller to drop once the router lock is released (a SoundFont
    /// synth is big).
    pub fn drain_trash(&mut self) -> Vec<Trash> {
        let mut out = Vec::new();
        if let Some(trash) = &mut self.trash {
            while let Ok(old) = trash.pop() {
                out.push(old);
            }
        }
        out
    }

    /// Swap the engine's keys instrument (built off the audio thread). The sound and the wheels
    /// are sent again so the new one starts in the current state.
    pub fn set_instrument(&mut self, instrument: Box<dyn Instrument>) {
        // A full queue hands the box back; it is dropped here, on the router thread.
        let _ = self.audio_tx.push(Command::SetInstrument(instrument));
        self.send_keys();
        self.send_patch();
        self.send_wheels();
    }

    /// The keys' sound, level and output from the set to the engine.
    pub fn send_keys(&mut self) {
        let k = &self.set.keys;
        let _ = self.audio_tx.push(Command::KeysSound(k.gm_sound));
        let _ = self.audio_tx.push(Command::KeysRoute {
            gain: db_to_gain(k.volume_db),
            group: k.output,
        });
        self.send_tempo();
    }

    pub fn send_tempo(&mut self) {
        let _ = self.audio_tx.push(Command::KeysTempo(self.set.pattern.bpm));
    }

    pub fn send_patch(&mut self) {
        let _ = self
            .audio_tx
            .push(Command::SynthLoad(Box::new(self.set.keys.patch.params)));
    }

    pub fn send_synth_param(&mut self, id: ParamId, value: f32) {
        let _ = self.audio_tx.push(Command::SynthParam(id, value));
    }

    pub fn send_route(&mut self, slot: usize, route: Option<Route>) {
        let _ = self.audio_tx.push(Command::SynthRoute {
            slot: slot as u8,
            route,
        });
    }

    pub fn keys_sustain(&mut self, down: bool) {
        self.keys_sustain = down;
        let _ = self.audio_tx.push(Command::KeysSustain(down));
    }

    fn send_wheels(&mut self) {
        let _ = self.audio_tx.push(Command::KeysPitch(self.keys_pitch));
        let _ = self.audio_tx.push(Command::KeysMod(self.keys_mod));
        let _ = self.audio_tx.push(Command::KeysSustain(self.keys_sustain));
    }

    /// A key played on the screen.
    pub fn keys_note(&mut self, note: u8, velocity: u8, on: bool) {
        let _ = self.audio_tx.push(if on {
            Command::KeysNoteOn { note, velocity }
        } else {
            Command::KeysNoteOff { note }
        });
    }

    /// The screen's pitch wheel, −1..1 (±2 semitones).
    pub fn keys_pitch_value(&mut self, value: f32) {
        let _ = self
            .audio_tx
            .push(Command::KeysPitch(value.clamp(-1.0, 1.0)));
    }

    /// The screen's mod wheel, 0..1.
    pub fn keys_mod_value(&mut self, value: f32) {
        let _ = self.audio_tx.push(Command::KeysMod(value.clamp(0.0, 1.0)));
    }

    /// The engine that owned the voices is gone: drop voice counts and holds.
    pub fn forget_voices(&mut self) {
        self.voice_counts.clear();
        self.held.clear();
    }

    /// Release every held pad (input ownership is changing).
    pub fn release_all_held(&mut self) {
        let ids: Vec<u16> = self.held.iter().copied().collect();
        for id in ids {
            self.release_id(id);
        }
    }

    /// Everything the light painter needs for the active bank, copied under one lock.
    pub fn paint_snapshot(&self) -> Option<PaintSnapshot> {
        let bank = self.set.banks.get(self.bank)?.clone();
        let loaded = bank
            .pads
            .iter()
            .filter(|p| self.samples.contains_key(&p.sample))
            .map(|p| p.sample.clone())
            .collect();
        let mut sounding = [false; 64];
        for y in 0..8u8 {
            for x in 0..8u8 {
                sounding[y as usize * 8 + x as usize] = self.is_pad_sounding(self.bank, x, y);
            }
        }
        Some(PaintSnapshot {
            bank_index: self.bank,
            bank,
            loaded,
            sounding,
        })
    }

    /// Replace the set but keep samples, holds and voice tracking (undo/redo). Undo/redo keep the
    /// current mixer values (`restore_keeping_mixer`), so nothing is sent to the engine.
    pub fn install_set(&mut self, set: Set) {
        let changed = self.set.keys.patch.params != set.keys.patch.params;
        self.set = set;
        self.send_keys();
        if changed {
            self.send_patch();
        }
    }

    /// Push every group and the master gain to the engine.
    pub fn send_mixer(&mut self) {
        Self::send_mixer_commands(&self.set, &mut self.audio_tx);
    }

    fn send_mixer_commands(set: &Set, tx: &mut Producer<Command>) {
        for (i, g) in set.mixer.groups.iter().enumerate() {
            let _ = tx.push(Command::SetGroup {
                group: i as u8,
                params: kit::group_params(g),
            });
        }
        let _ = tx.push(Command::SetMasterGain(fader_gain(set.mixer.master_db)));
    }

    pub fn select_bank(&mut self, bank: usize) -> bool {
        if bank < self.set.banks.len() {
            self.bank = bank;
            true
        } else {
            false
        }
    }

    /// Fade out everything and stop the sequencer clock (REC stays armed).
    pub fn stop_all(&mut self) {
        let _ = self.audio_tx.push(Command::StopAll);
        self.seq
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .stop();
    }

    /// Current bank, full velocity.
    pub fn trigger_pad(&mut self, x: u8, y: u8) {
        self.trigger_pad_in(self.bank, x, y, 1.0);
    }

    /// The trigger path for every pad hit that is not the sequencer's own (screen, Launchpad,
    /// computer keys, learned notes): pad gain × `velocity_gain` × bank humanize, and the hit is
    /// reported to the sequencer's record hook.
    pub fn trigger_pad_in(&mut self, bank: usize, x: u8, y: u8, velocity_gain: f32) {
        if self.trigger_pad_quiet(bank, x, y, velocity_gain) {
            let seq = self.seq.clone();
            let mut s = seq.lock().unwrap_or_else(PoisonError::into_inner);
            s.record(&mut self.set.pattern, bank, x, y, Instant::now());
        }
    }

    /// Trigger without recording: the clock's own hits and step auditions. Returns whether a
    /// voice was sent.
    pub fn trigger_pad_quiet(&mut self, bank: usize, x: u8, y: u8, velocity_gain: f32) -> bool {
        let Some(b) = self.set.banks.get(bank) else {
            return false;
        };
        let humanize_db = b.humanize_db;
        let Some(pad) = b.pad(x, y) else {
            return false;
        };
        let Some(sample) = self.samples.get(&pad.sample) else {
            return false;
        };
        let mut params = kit::play_params(pad, sample);
        let sample = sample.clone();
        let mut gain = velocity_gain;
        if humanize_db > 0.0 {
            gain *= db_to_gain(self.next_unit() * humanize_db / 2.0);
        }
        params.gain *= gain;
        let id = Self::pad_id(bank, x, y);
        let sent = self.audio_tx.push(Command::Trigger {
            pad: id,
            sample,
            params,
        });
        if sent.is_ok() {
            self.held.insert(id);
        }
        sent.is_ok()
    }

    /// Fire the sequencer steps `steps` (indices inside the pattern length): every unmuted lane
    /// with a non-zero value whose `(lane, step)` is not in `skip` triggers its pad, quietly.
    /// Lanes whose pad has no sample do nothing.
    pub fn seq_fire(&mut self, steps: &[usize], skip: &HashSet<(usize, usize)>) -> Vec<Fired> {
        let mut fired = Vec::new();
        for &step in steps {
            for lane in 0..self.set.pattern.lanes.len() {
                let l = &self.set.pattern.lanes[lane];
                let value = l.steps.get(step).copied().unwrap_or(0);
                if l.mute || value == 0 || skip.contains(&(lane, step)) {
                    continue;
                }
                let (bank, x, y) = (usize::from(l.bank), l.x, l.y);
                let vel = if value >= 2 {
                    VEL_ACCENT
                } else {
                    lpd_core::seq::VEL_ON
                };
                if self.trigger_pad_quiet(bank, x, y, f32::from(vel) / 127.0) {
                    let hold = self.set.banks[bank]
                        .pad(x, y)
                        .is_some_and(|p| p.mode == kit::PadMode::Hold);
                    fired.push(Fired { bank, x, y, hold });
                }
            }
        }
        fired
    }

    /// Release (x, y) of the current bank; if that is not held (bank switched while
    /// holding), release the pad held at (x, y) in another bank.
    pub fn release_pad(&mut self, x: u8, y: u8) {
        let current = Self::pad_id(self.bank, x, y);
        let id = if self.held.contains(&current) {
            Some(current)
        } else {
            let pos = Self::pad_id(0, x, y);
            self.held.iter().copied().find(|id| id % 64 == pos)
        };
        if let Some(id) = id {
            self.release_id(id);
        }
    }

    pub fn release_pad_in(&mut self, bank: usize, x: u8, y: u8) {
        self.release_id(Self::pad_id(bank, x, y));
    }

    fn release_id(&mut self, id: u16) {
        if self.held.remove(&id) {
            let _ = self.audio_tx.push(Command::Release { pad: id });
        }
    }

    /// A message from a generic MIDI input (`role` is "keys" or "other") on `port`. Learn and
    /// bindings come first (a bound note never reaches the keys); what is left becomes keys
    /// events for role `keys`. Everything the caller must finish or emit is plain data in the
    /// outcome (nothing is emitted here).
    pub fn on_midi(&mut self, port: &str, role: &str, msg: MidiMsg) -> MidiOutcome {
        let mut out = MidiOutcome::default();
        if !matches!(role, "keys" | "other") {
            return out;
        }
        if let Some(binding) = self.armed_binding(port, &msg) {
            self.learn_armed = None;
            out.bind = Some(binding);
            return out;
        }
        match msg {
            MidiMsg::NoteOn { note, vel, .. } => {
                if let Some((bank, x, y)) = self.bound_pad(port, note) {
                    self.trigger_pad_in(usize::from(bank), x, y, f32::from(vel) / 127.0);
                    out.hit = Some((usize::from(bank), x, y));
                    out.event = Some(UiEvent::PadHit { bank, x, y });
                    return out;
                }
            }
            MidiMsg::NoteOff { note, .. } => {
                if let Some((bank, x, y)) = self.bound_pad(port, note) {
                    self.release_pad_in(usize::from(bank), x, y);
                    return out;
                }
            }
            MidiMsg::Cc { cc, value, .. } => {
                if let Some(id) = self.bound_param(port, cc) {
                    self.apply_learned_cc(port, cc, value, &id, &mut out);
                    return out;
                }
            }
            MidiMsg::PitchBend { .. } => {}
        }
        if role == "keys" {
            out.event = self.keys_event(msg);
        }
        out
    }

    fn keys_event(&mut self, msg: MidiMsg) -> Option<UiEvent> {
        match msg {
            MidiMsg::NoteOn { note, vel, .. } => {
                self.keys_note(note, vel, true);
                Some(UiEvent::KeysNote {
                    note,
                    velocity: vel,
                    on: true,
                })
            }
            MidiMsg::NoteOff { note, .. } => {
                self.keys_note(note, 0, false);
                Some(UiEvent::KeysNote {
                    note,
                    velocity: 0,
                    on: false,
                })
            }
            MidiMsg::PitchBend { value, .. } => {
                self.keys_pitch = f32::from(value) / 8192.0;
                self.keys_pitch_value(self.keys_pitch);
                Some(self.wheel_event())
            }
            MidiMsg::Cc { cc: 1, value, .. } => {
                self.keys_mod = f32::from(value) / 127.0;
                self.keys_mod_value(self.keys_mod);
                Some(self.wheel_event())
            }
            MidiMsg::Cc { cc: 64, value, .. } => {
                self.keys_sustain(value >= 64);
                None
            }
            MidiMsg::Cc { .. } => None,
        }
    }

    /// The binding the armed target wants from this message: a note for a pad, a CC for a
    /// parameter. Anything else is not learned (it plays or acts as usual).
    fn armed_binding(&self, port: &str, msg: &MidiMsg) -> Option<Binding> {
        match (self.learn_armed.as_ref()?, msg) {
            (LearnTarget::Pad { bank, x, y }, MidiMsg::NoteOn { note, .. }) => Some(Binding {
                device: port.to_string(),
                kind: LearnKind::Note,
                number: *note,
                target: Target::Pad {
                    bank: *bank,
                    x: *x,
                    y: *y,
                },
            }),
            (LearnTarget::Param(id), MidiMsg::Cc { cc, .. }) => Some(Binding {
                device: port.to_string(),
                kind: LearnKind::Cc,
                number: *cc,
                target: Target::Param(id.clone()),
            }),
            _ => None,
        }
    }

    fn bound_pad(&self, port: &str, note: u8) -> Option<(u8, u8, u8)> {
        self.set.learn.iter().find_map(|b| match &b.target {
            Target::Pad { bank, x, y }
                if b.kind == LearnKind::Note && b.number == note && b.device == port =>
            {
                Some((*bank, *x, *y))
            }
            _ => None,
        })
    }

    fn bound_param(&self, port: &str, cc: u8) -> Option<String> {
        self.set.learn.iter().find_map(|b| match &b.target {
            Target::Param(id) if b.kind == LearnKind::Cc && b.number == cc && b.device == port => {
                Some(id.clone())
            }
            _ => None,
        })
    }

    /// A CC bound to parameter `id`. Group and master values apply here (no history); `bank.*`
    /// goes back in the outcome, as do the sequencer's tempo and swing and the keys volume; `seq.play` starts or stops the
    /// clock; `keys.mod` moves the mod wheel, `keys.volume` is a set edit.
    fn apply_learned_cc(&mut self, port: &str, cc: u8, value: u8, id: &str, out: &mut MidiOutcome) {
        let Some(param) = learn::param_value(id, value) else {
            return;
        };
        if param == ParamValue::Toggle {
            let last = self.swap_last_cc(port, cc, value);
            if !(last < 64 && value >= 64) {
                return;
            }
        }
        if id == "stopAll" {
            self.stop_all();
        } else if id == "master" {
            if learn::apply_mixer_param(&mut self.set.mixer, id, param) {
                let gain = fader_gain(self.set.mixer.master_db);
                let _ = self.audio_tx.push(Command::SetMasterGain(gain));
                out.mixer_changed = true;
            }
        } else if let Some(group) = learn::group_index(id) {
            if learn::apply_mixer_param(&mut self.set.mixer, id, param) {
                self.send_group(group);
                out.mixer_changed = true;
            }
        } else if id == "keys.mod" {
            if let ParamValue::Continuous(v) = param {
                self.keys_mod = v;
                self.keys_mod_value(v);
                out.event = Some(self.wheel_event());
            }
        } else if id == "seq.play" {
            let params = crate::sequencer::Params::from(&self.set.pattern);
            let mut s = self.seq.lock().unwrap_or_else(PoisonError::into_inner);
            if s.playing {
                s.stop();
            } else {
                s.start(Instant::now(), params);
            }
        } else if id.starts_with("bank.")
            || id.starts_with("synth.")
            || matches!(id, "seq.bpm" | "seq.swing" | "keys.volume")
        {
            out.set_param = Some((id.to_string(), param));
        }
    }

    fn swap_last_cc(&mut self, port: &str, cc: u8, value: u8) -> u8 {
        if let Some(entry) = self
            .last_cc
            .iter_mut()
            .find(|(device, number, _)| device == port && *number == cc)
        {
            return std::mem::replace(&mut entry.2, value);
        }
        self.last_cc.push((port.to_string(), cc, value));
        0
    }

    fn send_group(&mut self, group: usize) {
        if let Some(g) = self.set.mixer.groups.get(group) {
            let params = kit::group_params(g);
            let _ = self.audio_tx.push(Command::SetGroup {
                group: group as u8,
                params,
            });
        }
    }

    pub fn arm_learn(&mut self, target: LearnTarget) {
        self.learn_armed = Some(target);
        self.learn_ignored_sent = false;
    }

    pub fn cancel_learn(&mut self) {
        self.learn_armed = None;
    }

    pub fn learn_armed(&self) -> bool {
        self.learn_armed.is_some()
    }

    /// Launchpad or LCXL input while learning is armed is not bound; the UI is told once per arm.
    pub fn learn_ignore(&mut self, reason: &str) -> Option<UiEvent> {
        if self.learn_armed.is_none() || self.learn_ignored_sent {
            return None;
        }
        self.learn_ignored_sent = true;
        Some(UiEvent::LearnIgnored {
            reason: reason.to_string(),
        })
    }

    /// A keys input went away: both wheels return to rest. `None` when they already were.
    pub fn reset_keys_wheels(&mut self) -> Option<UiEvent> {
        if self.keys_pitch == 0.0 && self.keys_mod == 0.0 {
            if self.keys_sustain {
                self.keys_sustain(false);
            }
            return None;
        }
        self.keys_pitch = 0.0;
        self.keys_mod = 0.0;
        self.keys_sustain = false;
        self.send_wheels();
        Some(self.wheel_event())
    }

    fn wheel_event(&self) -> UiEvent {
        UiEvent::KeysWheel {
            pitch: self.keys_pitch,
            r#mod: self.keys_mod,
        }
    }

    /// Faders/buttons change the mixer directly; knobs return an undoable synth edit for AppState.
    pub fn on_lcxl(&mut self, ev: LcxlEvent, synth_engine: bool) -> LcxlOutcome {
        if let LcxlEvent::Knob { row, ch, value } = ev {
            let synth = synth_engine
                .then(|| {
                    lcxl::SYNTH_KNOBS
                        .get(row as usize)?
                        .get(ch as usize)
                        .copied()
                })
                .flatten()
                .map(|id| {
                    (
                        id,
                        id.meta().kind.from_norm(f32::from(value.min(127)) / 127.0),
                    )
                });
            return LcxlOutcome {
                mixer: false,
                synth,
            };
        }
        let ch = match ev {
            LcxlEvent::Fader { ch, .. }
            | LcxlEvent::Knob { ch, .. }
            | LcxlEvent::Button { ch, .. } => ch as usize,
        };
        let Some(old) = self.set.mixer.groups.get(ch) else {
            return LcxlOutcome::default();
        };
        let mut g = old.clone();
        match ev {
            LcxlEvent::Fader { value, .. } => g.volume_db = lcxl::fader_db(value),
            LcxlEvent::Knob { .. } => return LcxlOutcome::default(),
            LcxlEvent::Button {
                row: 0, on: true, ..
            } => g.mute = !g.mute,
            LcxlEvent::Button {
                row: 1, on: true, ..
            } => g.solo = !g.solo,
            LcxlEvent::Button { .. } => return LcxlOutcome::default(),
        }
        if g == *old {
            return LcxlOutcome::default();
        }
        self.update_group(ch, g);
        LcxlOutcome {
            mixer: true,
            synth: None,
        }
    }

    pub fn on_lp_event(&mut self, ev: LpEvent) {
        match ev {
            LpEvent::PadDown { x, y } => self.trigger_pad(x, y),
            LpEvent::PadUp { x, y } => self.release_pad(x, y),
            LpEvent::SideDown(i) => {
                self.select_bank(i as usize);
            }
            LpEvent::TopDown(TOP_MIXER) => {
                self.stop_all();
            }
            _ => {}
        }
    }
}

/// True when the sets differ in what the engine hears from the mixer (group names ignored).
pub fn mixer_audio_differs(a: &Set, b: &Set) -> bool {
    a.mixer.master_db != b.mixer.master_db
        || a.mixer.groups.len() != b.mixer.groups.len()
        || a.mixer
            .groups
            .iter()
            .zip(&b.mixer.groups)
            .any(|(x, y)| kit::group_params(x) != kit::group_params(y))
}

fn time_seed() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
        .unwrap_or(0x9E37_79B9);
    nanos.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpd_core::kit::Pad;
    use rtrb::RingBuffer;

    fn router_with_pad(humanize_db: f32, volume_db: f32) -> (Router, Consumer<Command>) {
        let mut set = Set::new();
        let mut pad = Pad::new(0, 0, "samples/a.wav", [63, 0, 0]);
        pad.volume_db = volume_db;
        set.banks[0].set_pad(pad.clone());
        set.banks[0].humanize_db = humanize_db;
        set.banks[1].set_pad(pad);
        let mut samples = HashMap::new();
        samples.insert(
            "samples/a.wav".to_string(),
            Arc::new(Sample {
                rate: 48_000,
                data: vec![0.5; 200],
            }),
        );
        let (tx, mut cmd_rx) = RingBuffer::new(1024);
        let (_evt_tx, evt_rx) = RingBuffer::new(16);
        let router = Router::new(set, samples, tx, evt_rx);
        while cmd_rx.pop().is_ok() {}
        (router, cmd_rx)
    }

    fn trigger_gains(rx: &mut Consumer<Command>) -> Vec<f32> {
        let mut gains = Vec::new();
        while let Ok(cmd) = rx.pop() {
            if let Command::Trigger { params, .. } = cmd {
                gains.push(params.gain);
            }
        }
        gains
    }

    #[test]
    fn humanize_varies_gain_within_half_range() {
        let (mut router, mut rx) = router_with_pad(6.0, 0.0);
        router.set_seed(12345);
        for _ in 0..10 {
            router.trigger_pad(0, 0);
        }
        let gains = trigger_gains(&mut rx);
        assert_eq!(gains.len(), 10);
        let (lo, hi) = (db_to_gain(-3.0), db_to_gain(3.0));
        assert!(gains.iter().all(|g| *g >= lo && *g <= hi), "{gains:?}");
        assert!(gains.iter().any(|g| *g != gains[0]), "{gains:?}");
    }

    #[test]
    fn no_humanize_gives_exact_pad_gain() {
        let (mut router, mut rx) = router_with_pad(0.0, -6.0);
        router.set_seed(7);
        for _ in 0..5 {
            router.trigger_pad(0, 0);
        }
        let gains = trigger_gains(&mut rx);
        assert_eq!(gains.len(), 5);
        assert!(gains.iter().all(|g| *g == db_to_gain(-6.0)), "{gains:?}");
    }

    fn key(r: &mut Router, role: &str, msg: MidiMsg) -> Option<UiEvent> {
        r.on_midi("K", role, msg).event
    }

    fn router() -> Router {
        router_with_pad(0.0, 0.0).0
    }

    fn set_group_commands(rx: &mut Consumer<Command>) -> Vec<(u8, lpd_core::audio::GroupParams)> {
        let mut out = Vec::new();
        while let Ok(cmd) = rx.pop() {
            if let Command::SetGroup { group, params } = cmd {
                out.push((group, params));
            }
        }
        out
    }

    #[test]
    fn lcxl_faders_still_drive_the_mixer() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        assert!(
            r.on_lcxl(LcxlEvent::Fader { ch: 2, value: 127 }, true)
                .mixer
        );
        assert_eq!(r.set().mixer.groups[2].volume_db, 6.0);
        let sent = set_group_commands(&mut rx);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, 2);
        // The same value again changes nothing and sends nothing.
        assert!(
            !r.on_lcxl(LcxlEvent::Fader { ch: 2, value: 127 }, true)
                .mixer
        );
        assert!(set_group_commands(&mut rx).is_empty());
    }

    #[test]
    fn lcxl_knobs_drive_the_synth_only_with_the_synth_engine() {
        let (mut r, _rx) = router_with_pad(0.0, 0.0);
        let mixer = r.set().mixer.clone();
        let knob = LcxlEvent::Knob {
            row: 1,
            ch: 0,
            value: 127,
        };
        assert_eq!(
            r.on_lcxl(knob, true).synth,
            Some((ParamId::FilterCutoff, 20000.0))
        );
        let off = r.on_lcxl(knob, false);
        assert_eq!(off.synth, None);
        assert!(!off.mixer);
        assert_eq!(r.set().mixer, mixer);
    }

    #[test]
    fn lcxl_buttons_toggle_mute_and_solo_and_ignore_releases() {
        let (mut r, _rx) = router_with_pad(0.0, 0.0);
        let press = |row| LcxlEvent::Button {
            row,
            ch: 3,
            on: true,
        };
        let release = |row| LcxlEvent::Button {
            row,
            ch: 3,
            on: false,
        };
        assert!(r.on_lcxl(press(0), true).mixer);
        assert!(r.set().mixer.groups[3].mute);
        assert!(!r.on_lcxl(release(0), true).mixer);
        assert!(r.set().mixer.groups[3].mute);
        assert!(r.on_lcxl(press(0), true).mixer);
        assert!(!r.set().mixer.groups[3].mute);
        assert!(r.on_lcxl(press(1), true).mixer);
        assert!(r.set().mixer.groups[3].solo);
        assert!(!r.on_lcxl(release(1), true).mixer);
        assert!(r.set().mixer.groups[3].solo);
    }

    fn cc_msg(cc: u8, value: u8) -> MidiMsg {
        MidiMsg::Cc { ch: 0, cc, value }
    }

    fn note_on(note: u8, vel: u8) -> MidiMsg {
        MidiMsg::NoteOn { ch: 0, note, vel }
    }

    fn bound(device: &str, kind: LearnKind, number: u8, target: Target) -> Binding {
        Binding {
            device: device.into(),
            kind,
            number,
            target,
        }
    }

    #[test]
    fn an_armed_param_binds_the_next_cc_and_disarms() {
        let mut r = router();
        r.arm_learn(LearnTarget::Param("group.2.volume".into()));
        let out = r.on_midi("Pad Kontrol", "other", cc_msg(21, 64));
        assert_eq!(
            out.bind,
            Some(bound(
                "Pad Kontrol",
                LearnKind::Cc,
                21,
                Target::Param("group.2.volume".into())
            ))
        );
        assert!(!r.learn_armed());
        // The binding is added by the caller; the router changed nothing else.
        assert!(r.set().learn.is_empty());
        assert!(!out.mixer_changed);
    }

    #[test]
    fn an_armed_pad_binds_the_next_note_and_keeps_it_from_the_keys() {
        let mut r = router();
        r.arm_learn(LearnTarget::Pad {
            bank: 1,
            x: 2,
            y: 3,
        });
        let out = r.on_midi("K", "keys", note_on(60, 100));
        assert_eq!(
            out.bind,
            Some(bound(
                "K",
                LearnKind::Note,
                60,
                Target::Pad {
                    bank: 1,
                    x: 2,
                    y: 3
                }
            ))
        );
        assert_eq!(out.event, None, "the learning note does not play the keys");
    }

    #[test]
    fn the_wrong_kind_of_message_is_not_learned() {
        let mut r = router();
        r.arm_learn(LearnTarget::Param("master".into()));
        let out = r.on_midi("K", "keys", note_on(60, 100));
        assert_eq!(out.bind, None);
        assert!(r.learn_armed());
        r.arm_learn(LearnTarget::Pad {
            bank: 0,
            x: 0,
            y: 0,
        });
        assert_eq!(r.on_midi("K", "keys", cc_msg(7, 9)).bind, None);
        assert!(r.learn_armed());
        r.cancel_learn();
        assert!(!r.learn_armed());
    }

    #[test]
    fn learn_ignored_is_reported_once_per_arm() {
        let mut r = router();
        assert_eq!(r.learn_ignore("x"), None, "not armed");
        r.arm_learn(LearnTarget::Param("master".into()));
        assert_eq!(
            r.learn_ignore("Launchpad pads are fixed"),
            Some(UiEvent::LearnIgnored {
                reason: "Launchpad pads are fixed".into()
            })
        );
        assert_eq!(r.learn_ignore("Launchpad pads are fixed"), None);
        r.arm_learn(LearnTarget::Param("master".into()));
        assert!(r.learn_ignore("again").is_some(), "a new arm starts over");
    }

    #[test]
    fn a_bound_note_triggers_the_pad_and_reports_a_hit() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        r.set_mut().learn.push(bound(
            "K",
            LearnKind::Note,
            36,
            Target::Pad {
                bank: 0,
                x: 0,
                y: 0,
            },
        ));
        let out = r.on_midi("K", "keys", note_on(36, 127));
        assert_eq!(
            out.event,
            Some(UiEvent::PadHit {
                bank: 0,
                x: 0,
                y: 0
            })
        );
        assert_eq!(out.hit, Some((0, 0, 0)));
        assert_eq!(
            trigger_gains(&mut rx),
            vec![1.0],
            "velocity 127 is full gain"
        );
        // Another device's note 36 is not bound.
        let other = r.on_midi("Other", "keys", note_on(36, 127));
        assert_eq!(other.hit, None);
        assert!(matches!(other.event, Some(UiEvent::KeysNote { .. })));
    }

    #[test]
    fn a_bound_note_off_releases_and_never_reaches_the_keys() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        r.set_mut().learn.push(bound(
            "K",
            LearnKind::Note,
            36,
            Target::Pad {
                bank: 0,
                x: 0,
                y: 0,
            },
        ));
        r.on_midi("K", "keys", note_on(36, 100));
        let off = r.on_midi("K", "keys", MidiMsg::NoteOff { ch: 0, note: 36 });
        assert_eq!(off.event, None);
        let mut released = false;
        while let Ok(cmd) = rx.pop() {
            released |= matches!(cmd, Command::Release { .. });
        }
        assert!(released);
    }

    #[test]
    fn a_bound_cc_sets_the_group_volume() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        r.set_mut().learn.push(bound(
            "K",
            LearnKind::Cc,
            7,
            Target::Param("group.1.volume".into()),
        ));
        let out = r.on_midi("K", "other", cc_msg(7, 127));
        assert!(out.mixer_changed);
        assert_eq!(r.set().mixer.groups[1].volume_db, 6.0);
        let sent = set_group_commands(&mut rx);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, 1);
        // The same value again changes nothing.
        assert!(!r.on_midi("K", "other", cc_msg(7, 127)).mixer_changed);
    }

    #[test]
    fn a_bound_master_cc_sets_the_master() {
        let mut r = router();
        r.set_mut()
            .learn
            .push(bound("K", LearnKind::Cc, 9, Target::Param("master".into())));
        assert!(r.on_midi("K", "keys", cc_msg(9, 127)).mixer_changed);
        assert_eq!(r.set().mixer.master_db, 6.0);
    }

    #[test]
    fn a_toggle_fires_only_on_the_rising_edge() {
        let mut r = router();
        r.set_mut().learn.push(bound(
            "K",
            LearnKind::Cc,
            64,
            Target::Param("group.0.mute".into()),
        ));
        let mut toggles = 0;
        for v in [0, 100, 120, 10, 90] {
            if r.on_midi("K", "other", cc_msg(64, v)).mixer_changed {
                toggles += 1;
            }
        }
        assert_eq!(toggles, 2);
        assert!(
            !r.set().mixer.groups[0].mute,
            "two toggles end where they began"
        );
    }

    #[test]
    fn a_bank_parameter_is_handed_back_for_the_history_path() {
        let mut r = router();
        r.set_mut().learn.push(bound(
            "K",
            LearnKind::Cc,
            20,
            Target::Param("bank.2.humanize".into()),
        ));
        let out = r.on_midi("K", "other", cc_msg(20, 127));
        assert_eq!(
            out.set_param,
            Some(("bank.2.humanize".to_string(), ParamValue::Continuous(6.0)))
        );
        assert_eq!(r.set().banks[2].humanize_db, 0.0, "applied by AppState");
    }

    #[test]
    fn stop_all_is_a_learnable_toggle() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        r.set_mut().learn.push(bound(
            "K",
            LearnKind::Cc,
            30,
            Target::Param("stopAll".into()),
        ));
        r.on_midi("K", "other", cc_msg(30, 127));
        let mut stopped = false;
        while let Ok(cmd) = rx.pop() {
            stopped |= matches!(cmd, Command::StopAll);
        }
        assert!(stopped);
    }

    #[test]
    fn learn_events_have_the_registry_names() {
        let hit = UiEvent::PadHit {
            bank: 1,
            x: 2,
            y: 3,
        };
        assert_eq!(hit.name(), "pad-hit");
        assert_eq!(
            hit.payload(),
            serde_json::json!({ "bank": 1, "x": 2, "y": 3 })
        );
        let ignored = UiEvent::LearnIgnored { reason: "r".into() };
        assert_eq!(ignored.name(), "learn-ignored");
        assert_eq!(ignored.payload(), serde_json::json!({ "reason": "r" }));
        let b = bound("K", LearnKind::Cc, 1, Target::Param("master".into()));
        let ev = UiEvent::LearnBound { binding: b };
        assert_eq!(ev.name(), "learn-bound");
        assert_eq!(ev.payload()["binding"]["target"]["param"], "master");
    }

    #[test]
    fn learn_targets_parse_from_the_js_shapes() {
        let pad: LearnTarget = serde_json::from_str(r#"{"pad":{"bank":0,"x":1,"y":2}}"#).unwrap();
        assert_eq!(
            pad,
            LearnTarget::Pad {
                bank: 0,
                x: 1,
                y: 2
            }
        );
        let param: LearnTarget = serde_json::from_str(r#"{"param":"group.2.volume"}"#).unwrap();
        assert_eq!(param, LearnTarget::Param("group.2.volume".into()));
    }

    fn playing_rec_router() -> (Router, Consumer<Command>) {
        let (r, rx) = router_with_pad(0.0, 0.0);
        {
            let seq = r.seq_handle();
            let mut s = seq.lock().unwrap();
            s.start(
                Instant::now(),
                crate::sequencer::Params::from(&r.set().pattern),
            );
            s.rec = true;
        }
        (r, rx)
    }

    #[test]
    fn a_pad_hit_while_recording_lands_in_the_pattern() {
        let (mut r, _rx) = playing_rec_router();
        r.trigger_pad(0, 0);
        let lanes = &r.set().pattern.lanes;
        assert_eq!(lanes.len(), 1);
        assert_eq!((lanes[0].bank, lanes[0].x, lanes[0].y), (0, 0, 0));
        assert_eq!(lanes[0].steps.iter().filter(|&&v| v == 1).count(), 1);
    }

    #[test]
    fn quiet_triggers_and_empty_pads_are_not_recorded() {
        let (mut r, _rx) = playing_rec_router();
        assert!(r.trigger_pad_quiet(0, 0, 0, 1.0));
        r.trigger_pad_in(0, 5, 5, 1.0); // no pad there
        assert!(r.set().pattern.lanes.is_empty());
    }

    #[test]
    fn a_pad_hit_without_rec_is_not_recorded() {
        let (mut r, _rx) = router_with_pad(0.0, 0.0);
        r.trigger_pad(0, 0);
        assert!(r.set().pattern.lanes.is_empty());
    }

    #[test]
    fn the_clock_fires_lanes_with_their_velocity_and_skips() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        let p = &mut r.set_mut().pattern;
        lpd_core::seq::add_lane(p, 0, 0, 0).unwrap();
        lpd_core::seq::add_lane(p, 1, 0, 0).unwrap();
        p.lanes[0].steps[2] = 1;
        p.lanes[1].steps[2] = 2;
        let fired = r.seq_fire(&[2], &HashSet::new());
        assert_eq!(fired.len(), 2);
        let gains = trigger_gains(&mut rx);
        assert!((gains[0] - 90.0 / 127.0).abs() < 1e-6, "{gains:?}");
        assert_eq!(gains[1], 1.0, "an accent is full velocity");
        // A skipped (lane, step) and a muted lane stay quiet; a step without a value too.
        r.set_mut().pattern.lanes[1].mute = true;
        let skip = HashSet::from([(0usize, 2usize)]);
        assert!(r.seq_fire(&[2], &skip).is_empty());
        assert!(r.seq_fire(&[3], &HashSet::new()).is_empty());
        assert!(trigger_gains(&mut rx).is_empty());
    }

    #[test]
    fn a_lane_without_a_sample_stays_silent_and_a_hold_pad_is_flagged() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        let p = &mut r.set_mut().pattern;
        lpd_core::seq::add_lane(p, 0, 7, 7).unwrap(); // no pad there
        lpd_core::seq::add_lane(p, 0, 0, 0).unwrap();
        p.lanes[0].steps[0] = 1;
        p.lanes[1].steps[0] = 1;
        r.set_mut().banks[0].pads[0].mode = kit::PadMode::Hold;
        let fired = r.seq_fire(&[0], &HashSet::new());
        assert_eq!(fired.len(), 1);
        assert!(fired[0].hold);
        assert_eq!(trigger_gains(&mut rx).len(), 1);
    }

    #[test]
    fn stop_all_stops_the_clock_but_keeps_rec_armed() {
        let (mut r, _rx) = playing_rec_router();
        r.stop_all();
        let seq = r.seq_handle();
        let s = seq.lock().unwrap();
        assert!(!s.playing);
        assert!(s.rec);
    }

    #[test]
    fn a_learned_seq_play_toggles_the_clock_and_tempo_goes_to_the_history_path() {
        let mut r = router();
        r.set_mut().learn.push(bound(
            "K",
            LearnKind::Cc,
            50,
            Target::Param("seq.play".into()),
        ));
        r.set_mut().learn.push(bound(
            "K",
            LearnKind::Cc,
            51,
            Target::Param("seq.bpm".into()),
        ));
        r.on_midi("K", "other", cc_msg(50, 127));
        assert!(r.seq_handle().lock().unwrap().playing);
        r.on_midi("K", "other", cc_msg(50, 0));
        r.on_midi("K", "other", cc_msg(50, 127));
        assert!(
            !r.seq_handle().lock().unwrap().playing,
            "the second rising edge stops it"
        );
        let out = r.on_midi("K", "other", cc_msg(51, 127));
        assert_eq!(
            out.set_param,
            Some(("seq.bpm".to_string(), ParamValue::Continuous(200.0)))
        );
    }

    #[test]
    fn keys_notes_become_keys_note_events() {
        let mut r = router();
        let on = key(
            &mut r,
            "keys",
            MidiMsg::NoteOn {
                ch: 0,
                note: 60,
                vel: 100,
            },
        );
        assert_eq!(
            on,
            Some(UiEvent::KeysNote {
                note: 60,
                velocity: 100,
                on: true
            })
        );
        let off = key(&mut r, "keys", MidiMsg::NoteOff { ch: 0, note: 60 });
        assert_eq!(
            off,
            Some(UiEvent::KeysNote {
                note: 60,
                velocity: 0,
                on: false
            })
        );
    }

    #[test]
    fn wheel_events_carry_both_last_values() {
        let mut r = router();
        let bend = key(&mut r, "keys", MidiMsg::PitchBend { ch: 0, value: 4096 });
        assert_eq!(
            bend,
            Some(UiEvent::KeysWheel {
                pitch: 0.5,
                r#mod: 0.0
            })
        );
        let modw = key(
            &mut r,
            "keys",
            MidiMsg::Cc {
                ch: 0,
                cc: 1,
                value: 127,
            },
        );
        assert_eq!(
            modw,
            Some(UiEvent::KeysWheel {
                pitch: 0.5,
                r#mod: 1.0
            })
        );
        let low = key(
            &mut r,
            "keys",
            MidiMsg::PitchBend {
                ch: 0,
                value: -8192,
            },
        );
        assert_eq!(
            low,
            Some(UiEvent::KeysWheel {
                pitch: -1.0,
                r#mod: 1.0
            })
        );
    }

    #[test]
    fn resetting_the_wheels_reports_only_when_they_moved() {
        let mut r = router();
        assert_eq!(r.reset_keys_wheels(), None);
        key(&mut r, "keys", MidiMsg::PitchBend { ch: 0, value: 4096 });
        assert_eq!(
            r.reset_keys_wheels(),
            Some(UiEvent::KeysWheel {
                pitch: 0.0,
                r#mod: 0.0
            })
        );
        assert_eq!(r.reset_keys_wheels(), None);
    }

    #[test]
    fn resetting_the_wheels_also_centres_them_in_the_engine() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        key(&mut r, "keys", MidiMsg::PitchBend { ch: 0, value: 4096 });
        while rx.pop().is_ok() {}
        r.reset_keys_wheels();
        let sent = drain(&mut rx);
        assert!(
            matches!(sent[0], Command::KeysPitch(p) if p == 0.0),
            "{sent:?}"
        );
        assert!(
            matches!(sent[1], Command::KeysMod(m) if m == 0.0),
            "{sent:?}"
        );
    }

    #[test]
    fn other_ccs_and_other_roles_say_nothing() {
        let mut r = router();
        let cc = key(
            &mut r,
            "keys",
            MidiMsg::Cc {
                ch: 0,
                cc: 7,
                value: 5,
            },
        );
        assert_eq!(cc, None);
        let other = key(
            &mut r,
            "other",
            MidiMsg::NoteOn {
                ch: 1,
                note: 36,
                vel: 90,
            },
        );
        assert_eq!(other, None);
    }

    fn drain(rx: &mut Consumer<Command>) -> Vec<Command> {
        let mut v = Vec::new();
        while let Ok(c) = rx.pop() {
            v.push(c);
        }
        v
    }

    #[test]
    fn an_unbound_keys_note_plays_the_keys_not_a_pad() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        key(&mut r, "keys", note_on(36, 127));
        key(&mut r, "keys", MidiMsg::NoteOff { ch: 0, note: 36 });
        let sent = drain(&mut rx);
        assert!(
            matches!(
                sent.as_slice(),
                [
                    Command::KeysNoteOn {
                        note: 36,
                        velocity: 127
                    },
                    Command::KeysNoteOff { note: 36 }
                ]
            ),
            "{sent:?}"
        );
    }

    #[test]
    fn a_note_bound_to_a_pad_triggers_the_pad_instead_of_the_keys() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        r.set_mut().learn.push(bound(
            "K",
            LearnKind::Note,
            36,
            Target::Pad {
                bank: 0,
                x: 0,
                y: 0,
            },
        ));
        r.on_midi("K", "keys", note_on(36, 127));
        let sent = drain(&mut rx);
        assert!(
            sent.iter().any(|c| matches!(c, Command::Trigger { .. })),
            "{sent:?}"
        );
        assert!(
            !sent.iter().any(|c| matches!(c, Command::KeysNoteOn { .. })),
            "{sent:?}"
        );
    }

    #[test]
    fn the_sustain_pedal_reaches_the_engine() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        key(&mut r, "keys", cc_msg(64, 127));
        key(&mut r, "keys", cc_msg(64, 0));
        assert!(matches!(
            drain(&mut rx).as_slice(),
            [Command::KeysSustain(true), Command::KeysSustain(false)]
        ));
    }

    #[test]
    fn a_closed_keys_input_lifts_the_pedal() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        key(&mut r, "keys", cc_msg(64, 127));
        drain(&mut rx);
        r.reset_keys_wheels();
        assert!(matches!(
            drain(&mut rx).as_slice(),
            [Command::KeysSustain(false)]
        ));
    }

    #[test]
    fn a_bpm_change_sends_the_tempo() {
        let (tx, mut rx) = RingBuffer::new(1024);
        let (_, events) = RingBuffer::new(16);
        let state = crate::state::AppState::new(Set::new(), tx, events);
        drain(&mut rx);
        let mut pattern = state.get_current_set().pattern;
        pattern.bpm = 128.0;
        state.seq_update(pattern, None).unwrap();
        assert!(drain(&mut rx)
            .iter()
            .any(|c| matches!(c, Command::KeysTempo(128.0))));
    }

    #[test]
    fn the_wheels_reach_the_engine() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        key(&mut r, "keys", MidiMsg::PitchBend { ch: 0, value: 4096 });
        key(&mut r, "keys", cc_msg(1, 127));
        let sent = drain(&mut rx);
        assert!(
            matches!(sent[0], Command::KeysPitch(p) if (p - 0.5).abs() < 1e-6),
            "{sent:?}"
        );
        assert!(
            matches!(sent[1], Command::KeysMod(m) if m == 1.0),
            "{sent:?}"
        );
    }

    #[test]
    fn another_role_sends_nothing_to_the_engine() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        key(&mut r, "other", note_on(36, 100));
        assert!(rx.pop().is_err());
    }

    #[test]
    fn keys_settings_reach_the_engine_and_a_new_instrument_starts_in_the_current_state() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        r.set_mut().keys.gm_sound = lpd_core::instrument::KeysSound::Organ;
        r.set_mut().keys.volume_db = -12.0;
        r.set_mut().keys.output = Some(3);
        r.send_keys();
        let sent = drain(&mut rx);
        assert!(matches!(
            sent[0],
            Command::KeysSound(lpd_core::instrument::KeysSound::Organ)
        ));
        assert!(
            matches!(sent[1], Command::KeysRoute { gain, group: Some(3) } if (gain - db_to_gain(-12.0)).abs() < 1e-6)
        );
        r.set_instrument(Box::new(lpd_core::synth::Synth::new(48_000)));
        let sent = drain(&mut rx);
        assert!(matches!(sent[0], Command::SetInstrument(_)));
        assert!(matches!(sent[1], Command::KeysSound(_)));
        assert!(matches!(sent[2], Command::KeysRoute { .. }));
        assert!(matches!(sent[3], Command::KeysTempo(_)));
        assert!(matches!(sent[4], Command::SynthLoad(_)));
        assert!(matches!(sent[5], Command::KeysPitch(_)));
        assert!(matches!(sent[6], Command::KeysMod(_)));
        assert!(matches!(sent[7], Command::KeysSustain(_)));
    }

    #[test]
    fn undo_resends_the_patch_only_when_it_changed() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        let mut set = r.set().clone();
        set.keys.patch.params.filter.cutoff = 300.0;
        r.install_set(set.clone());
        assert!(drain(&mut rx)
            .iter()
            .any(|c| matches!(c, Command::SynthLoad(p) if p.filter.cutoff == 300.0)));
        set.keys.volume_db = -12.0;
        r.install_set(set);
        assert!(!drain(&mut rx)
            .iter()
            .any(|c| matches!(c, Command::SynthLoad(_))));
    }

    #[test]
    fn a_learned_keys_mod_moves_the_wheel() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        r.set_mut().learn.push(bound(
            "K",
            LearnKind::Cc,
            9,
            Target::Param("keys.mod".into()),
        ));
        let out = r.on_midi("K", "other", cc_msg(9, 127));
        assert_eq!(
            out.event,
            Some(UiEvent::KeysWheel {
                pitch: 0.0,
                r#mod: 1.0
            })
        );
        assert!(matches!(drain(&mut rx)[..], [Command::KeysMod(m)] if m == 1.0));
        // keys.volume is a set edit handled by AppState.
        r.set_mut().learn.push(bound(
            "K",
            LearnKind::Cc,
            10,
            Target::Param("keys.volume".into()),
        ));
        let out = r.on_midi("K", "other", cc_msg(10, 127));
        assert_eq!(
            out.set_param,
            Some(("keys.volume".to_string(), ParamValue::Continuous(6.0)))
        );
    }

    #[test]
    fn ui_events_have_the_registry_names_and_payloads() {
        let note = UiEvent::KeysNote {
            note: 60,
            velocity: 100,
            on: true,
        };
        assert_eq!(note.name(), "keys-note");
        assert_eq!(
            note.payload(),
            serde_json::json!({ "note": 60, "velocity": 100, "on": true })
        );
        let wheel = UiEvent::KeysWheel {
            pitch: 0.5,
            r#mod: 1.0,
        };
        assert_eq!(wheel.name(), "keys-wheel");
        assert_eq!(
            wheel.payload(),
            serde_json::json!({ "pitch": 0.5, "mod": 1.0 })
        );
        let log = UiEvent::MidiLog { text: "x".into() };
        assert_eq!(log.name(), "midi-log");
        assert_eq!(log.payload(), serde_json::json!({ "text": "x" }));
    }

    #[test]
    fn velocity_gain_scales_trigger() {
        let (mut router, mut rx) = router_with_pad(0.0, 0.0);
        router.trigger_pad_in(1, 0, 0, 0.5);
        assert_eq!(trigger_gains(&mut rx), vec![0.5]);
    }

    #[test]
    fn release_from_another_bank_uses_pad_id() {
        let (mut router, mut rx) = router_with_pad(0.0, 0.0);
        router.trigger_pad_in(1, 0, 0, 1.0);
        router.release_pad_in(1, 0, 0);
        let mut released = Vec::new();
        while let Ok(cmd) = rx.pop() {
            if let Command::Release { pad } = cmd {
                released.push(pad);
            }
        }
        assert_eq!(released, vec![Router::pad_id(1, 0, 0)]);
    }

    #[test]
    fn a_trigger_lost_to_a_full_queue_is_not_held() {
        let mut set = Set::new();
        set.banks[0].set_pad(Pad::new(0, 0, "samples/a.wav", [63, 0, 0]));
        let mut samples = HashMap::new();
        samples.insert(
            "samples/a.wav".to_string(),
            Arc::new(Sample {
                rate: 48_000,
                data: vec![0.5; 200],
            }),
        );
        // Smaller than the mixer burst `new` sends: the queue is full afterwards.
        let (tx, mut cmd_rx) = RingBuffer::new(2);
        let (_evt_tx, evt_rx) = RingBuffer::new(16);
        let mut router = Router::new(set, samples, tx, evt_rx);
        router.trigger_pad(0, 0);
        assert!(!router.held.contains(&Router::pad_id(0, 0, 0)));

        while cmd_rx.pop().is_ok() {}
        router.release_pad(0, 0);
        assert!(
            cmd_rx.pop().is_err(),
            "no release for a pad that never sounded"
        );
    }

    #[test]
    fn replace_audio_swaps_queues_and_resends_the_mixer() {
        let (mut router, mut old_rx) = router_with_pad(0.0, 0.0);
        router.trigger_pad(0, 0);
        router.poll_audio_events();
        router.voice_counts.insert(Router::pad_id(0, 0, 0), 2);
        assert!(!router.held.is_empty());
        while old_rx.pop().is_ok() {}

        let (new_tx, mut new_rx) = RingBuffer::new(1024);
        let (_evt_tx, new_events) = RingBuffer::new(16);
        router.replace_audio(new_tx, new_events);

        // Fresh stream: no voices, nothing held, mixer sent first.
        assert!(!router.is_pad_sounding(0, 0, 0));
        assert!(router.held.is_empty());
        let first: Vec<Command> = (0..9).filter_map(|_| new_rx.pop().ok()).collect();
        assert_eq!(first.len(), 9);
        assert!(first[..8]
            .iter()
            .enumerate()
            .all(|(i, c)| matches!(c, Command::SetGroup { group, .. } if *group as usize == i)));
        assert!(matches!(first[8], Command::SetMasterGain(_)));
        // Then the keys: sound, route and the wheels (nothing else).
        let keys = drain(&mut new_rx);
        assert!(
            matches!(
                keys.as_slice(),
                [
                    Command::KeysSound(_),
                    Command::KeysRoute { .. },
                    Command::KeysTempo(_),
                    Command::SynthLoad(_),
                    Command::KeysPitch(_),
                    Command::KeysMod(_),
                    Command::KeysSustain(_)
                ]
            ),
            "{keys:?}"
        );

        // A trigger now lands in the new queue only.
        router.trigger_pad(0, 0);
        assert!(matches!(new_rx.pop(), Ok(Command::Trigger { .. })));
        assert!(old_rx.pop().is_err());
    }

    #[test]
    fn release_after_bank_switch_still_releases_pressed_pad() {
        let (mut router, mut rx) = router_with_pad(0.0, 0.0);
        router.trigger_pad(0, 0);
        router.select_bank(1);
        router.release_pad(0, 0);
        let mut released = Vec::new();
        while let Ok(cmd) = rx.pop() {
            if let Command::Release { pad } = cmd {
                released.push(pad);
            }
        }
        assert_eq!(released, vec![Router::pad_id(0, 0, 0)]);
    }
}
