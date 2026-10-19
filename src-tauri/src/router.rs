//! Router: single owner of set state, bank selection, and high-priority MIDI-to-audio dispatch.

use crate::devices::MidiMsg;
use crate::lights::PaintSnapshot;
use lpd_core::audio::{db_to_gain, fader_gain, Command, Event, Sample};
use lpd_core::devices::launchpad::LpEvent;
use lpd_core::kit::{self, Set};
use rtrb::{Consumer, Producer};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const TOP_MIXER: u8 = 7;

/// Something the UI should hear about, produced under the router lock and emitted by the caller
/// after the lock is released.
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    KeysNote { note: u8, velocity: u8, on: bool },
    KeysWheel { pitch: f32, r#mod: f32 },
    MidiLog { text: String },
}

impl UiEvent {
    /// The Tauri event name (plan 0.4).
    pub fn name(&self) -> &'static str {
        match self {
            Self::KeysNote { .. } => "keys-note",
            Self::KeysWheel { .. } => "keys-wheel",
            Self::MidiLog { .. } => "midi-log",
        }
    }

    pub fn payload(&self) -> serde_json::Value {
        match self {
            Self::KeysNote { note, velocity, on } => {
                serde_json::json!({ "note": note, "velocity": velocity, "on": on })
            }
            Self::KeysWheel { pitch, r#mod } => serde_json::json!({ "pitch": pitch, "mod": r#mod }),
            Self::MidiLog { text } => serde_json::json!({ "text": text }),
        }
    }
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
}

impl Router {
    pub fn new(
        set: Set,
        samples: HashMap<String, Arc<Sample>>,
        mut audio_tx: Producer<Command>,
        audio_rx: Consumer<Event>,
    ) -> Self {
        Self::send_mixer_commands(&set, &mut audio_tx);
        Self {
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
        }
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
        let needed = 1 + set.mixer.groups.len() + 1;
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
        // keys: Task 15 (re-send the keys settings to the new engine here)
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
        self.set = set;
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

    pub fn stop_all(&mut self) {
        let _ = self.audio_tx.push(Command::StopAll);
    }

    /// Current bank, full velocity.
    pub fn trigger_pad(&mut self, x: u8, y: u8) {
        self.trigger_pad_in(self.bank, x, y, 1.0);
    }

    /// The one trigger path: pad gain × `velocity_gain` × bank humanize (±humanize_db/2).
    pub fn trigger_pad_in(&mut self, bank: usize, x: u8, y: u8, velocity_gain: f32) {
        let Some(b) = self.set.banks.get(bank) else {
            return;
        };
        let humanize_db = b.humanize_db;
        let Some(pad) = b.pad(x, y) else {
            return;
        };
        let Some(sample) = self.samples.get(&pad.sample) else {
            return;
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

    /// A message from a generic MIDI input (`role` is "keys" or "other"). Returns what the UI
    /// should hear; the caller emits it after releasing the router lock.
    pub fn on_midi(&mut self, port: &str, role: &str, msg: MidiMsg) -> Vec<UiEvent> {
        let mut events = Vec::with_capacity(2);
        if role == "keys" {
            match msg {
                MidiMsg::NoteOn { note, vel, .. } => events.push(UiEvent::KeysNote {
                    note,
                    velocity: vel,
                    on: true,
                }),
                MidiMsg::NoteOff { note, .. } => events.push(UiEvent::KeysNote {
                    note,
                    velocity: 0,
                    on: false,
                }),
                MidiMsg::PitchBend { value, .. } => {
                    self.keys_pitch = f32::from(value) / 8192.0;
                    events.push(self.wheel_event());
                }
                MidiMsg::Cc { cc: 1, value, .. } => {
                    self.keys_mod = f32::from(value) / 127.0;
                    events.push(self.wheel_event());
                }
                MidiMsg::Cc { .. } => {}
            }
        }
        // Role "other" acts only through learn bindings (Task 12).
        events.push(UiEvent::MidiLog {
            text: format!("{port}: {msg:?}"),
        });
        events
    }

    fn wheel_event(&self) -> UiEvent {
        UiEvent::KeysWheel {
            pitch: self.keys_pitch,
            r#mod: self.keys_mod,
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

    fn router() -> Router {
        router_with_pad(0.0, 0.0).0
    }

    #[test]
    fn keys_notes_become_keys_note_events_and_a_log_line() {
        let mut r = router();
        let on = r.on_midi(
            "Keystation 49 MK3",
            "keys",
            MidiMsg::NoteOn {
                ch: 0,
                note: 60,
                vel: 100,
            },
        );
        assert_eq!(
            on,
            vec![
                UiEvent::KeysNote {
                    note: 60,
                    velocity: 100,
                    on: true
                },
                UiEvent::MidiLog {
                    text: "Keystation 49 MK3: NoteOn { ch: 0, note: 60, vel: 100 }".into()
                },
            ]
        );
        let off = r.on_midi(
            "Keystation 49 MK3",
            "keys",
            MidiMsg::NoteOff { ch: 0, note: 60 },
        );
        assert_eq!(
            off[0],
            UiEvent::KeysNote {
                note: 60,
                velocity: 0,
                on: false
            }
        );
    }

    #[test]
    fn wheel_events_carry_both_last_values() {
        let mut r = router();
        let bend = r.on_midi("K", "keys", MidiMsg::PitchBend { ch: 0, value: 4096 });
        assert_eq!(
            bend[0],
            UiEvent::KeysWheel {
                pitch: 0.5,
                r#mod: 0.0
            }
        );
        let modw = r.on_midi(
            "K",
            "keys",
            MidiMsg::Cc {
                ch: 0,
                cc: 1,
                value: 127,
            },
        );
        assert_eq!(
            modw[0],
            UiEvent::KeysWheel {
                pitch: 0.5,
                r#mod: 1.0
            }
        );
        let low = r.on_midi(
            "K",
            "keys",
            MidiMsg::PitchBend {
                ch: 0,
                value: -8192,
            },
        );
        assert_eq!(
            low[0],
            UiEvent::KeysWheel {
                pitch: -1.0,
                r#mod: 1.0
            }
        );
    }

    #[test]
    fn other_ccs_and_other_roles_only_log() {
        let mut r = router();
        let cc = r.on_midi(
            "K",
            "keys",
            MidiMsg::Cc {
                ch: 0,
                cc: 7,
                value: 5,
            },
        );
        assert!(matches!(cc.as_slice(), [UiEvent::MidiLog { .. }]));
        let other = r.on_midi(
            "loopMIDI Port",
            "other",
            MidiMsg::NoteOn {
                ch: 1,
                note: 36,
                vel: 90,
            },
        );
        assert_eq!(
            other,
            vec![UiEvent::MidiLog {
                text: "loopMIDI Port: NoteOn { ch: 1, note: 36, vel: 90 }".into()
            }]
        );
    }

    #[test]
    fn midi_input_never_triggers_pads_or_audio() {
        let (mut r, mut rx) = router_with_pad(0.0, 0.0);
        r.on_midi(
            "K",
            "keys",
            MidiMsg::NoteOn {
                ch: 0,
                note: 36,
                vel: 127,
            },
        );
        assert!(rx.pop().is_err());
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
        assert!(new_rx.pop().is_err(), "nothing else is queued");

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
