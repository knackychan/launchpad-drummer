//! Application state coordinating router, lights, kit manager, saver and audio status.
//!
//! Lock order (take in this order, never the reverse; release before slow work):
//! `router` → `history` → `kit_manager` → `saver`. `painter`, `save_gate` and `assigns` are
//! separate: `painter` and `assigns` are leaves (nothing is taken while holding them) and
//! `save_gate` is taken first, only to order disk writes. No lock is held across disk I/O,
//! sample decoding or MIDI output. `config` → `startup` are leaves too (taken after nothing
//! else); the one exception to "no lock across disk I/O" is `config`, held while its own small
//! file is written.

use crate::audio::{self, AudioStats, AudioStatus};
use crate::config::{self, AppConfig, AudioPrefs};
use crate::devices::{self, MidiDevice, MidiMsg};
use crate::lights::LightPainter;
use crate::logfile;
use crate::router::{LearnTarget, Router, UiEvent};
use crate::saver::{SaveState, SaveStatus, Saver};
use crate::sequencer::{Fired, Params, SeqState};
use crate::startup::{self, Startup};
use crate::store::{prepare_sample, KitManager, LoadedKit};
use lpd_core::audio::{Command, Event, Sample, Taps};
use lpd_core::devices::launchpad::LpEvent;
use lpd_core::devices::lcxl::LcxlEvent;
use lpd_core::instrument::{BasicKeys, Instrument, KeysSettings, SoundFontKeys};
use lpd_core::kit::learn;
use lpd_core::kit::{self, History, Pad, Set};
use rtrb::{Consumer, Producer};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplePeaks {
    pub peaks: Vec<f32>,
    pub length_ms: f32,
    pub rate: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryState {
    pub can_undo: bool,
    pub can_redo: bool,
}

/// A set that was just created or opened.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenResult {
    pub set: Set,
    pub path: String,
    pub name: String,
    pub from_backup: bool,
    pub backup_saved_at: Option<String>,
}

/// A pad whose sample did not load.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingPad {
    pub bank: usize,
    pub x: u8,
    pub y: u8,
    pub sample: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DropReport {
    pub set: Set,
    pub imported: usize,
    pub leftover: usize,
    pub rejected: usize,
}

/// Which engine plays the keys instrument.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeysStatus {
    pub engine: String,
    pub name: String,
    pub error: Option<String>,
}

impl KeysStatus {
    pub fn builtin() -> Self {
        KeysStatus {
            engine: "builtin".into(),
            name: "BUILT-IN".into(),
            error: None,
        }
    }
}

/// Called when the keys engine changes without a command asking for it (a SoundFont reloaded
/// after the audio stream restarted).
pub type KeysNotifier = Arc<dyn Fn(&KeysStatus) + Send + Sync>;

/// The app config and the file it lives in (none in tests until `restore`).
#[derive(Default)]
struct ConfigFile {
    path: Option<PathBuf>,
    config: AppConfig,
}

/// Who owns the Launchpad's pads and LEDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Sampler = 0,
    Game = 1,
}

type Writer = Arc<dyn Fn(&Path, &Set) -> anyhow::Result<()> + Send + Sync>;

#[derive(Default)]
struct Assigns {
    next: u64,
    latest: HashMap<(usize, u8, u8), u64>,
}

/// A sample assignment in flight: what the request saw when it started.
#[derive(Clone, Debug)]
pub struct AssignTicket {
    pub dir: PathBuf,
    epoch: u64,
    seq: u64,
    bank: usize,
    x: u8,
    y: u8,
}

const STALE_SET: &str = "The set changed while the sample was loading; it was discarded.";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Apply `f` to a copy, write the copy, and keep it only when the write succeeded.
fn write_config(c: &mut ConfigFile, f: impl FnOnce(&mut AppConfig)) -> Result<(), String> {
    let mut next = c.config.clone();
    f(&mut next);
    if let Some(path) = &c.path {
        config::save(path, &next).map_err(|e| format!("Couldn't save the app config: {e:#}"))?;
    }
    c.config = next;
    Ok(())
}

/// What the player is told when a set folder does not open.
fn open_error(dir: &Path, e: &anyhow::Error) -> String {
    let name = startup::set_name(dir);
    if !dir.join(kit::SET_FILE).exists() && !dir.join(kit::BACKUP_FILE).exists() {
        return format!("{name} has no set.json — pick a set folder, or use New set");
    }
    // `kit::load` read the backup but could not move the broken set.json aside.
    if e.chain()
        .any(|c| c.to_string() == "move broken set.json aside")
    {
        return format!("Couldn't open {name}: {e:#}");
    }
    format!("Couldn't open {name} — set.json and backup are unreadable. Files left untouched.")
}

#[derive(Clone)]
pub struct AppState {
    pub router: Arc<Mutex<Router>>,
    pub painter: Arc<Mutex<LightPainter>>,
    pub kit_manager: Arc<Mutex<KitManager>>,
    pub audio_status: Arc<Mutex<AudioStatus>>,
    /// Engine output taps read by the telemetry thread (a leaf lock); swapped on a stream restart.
    pub taps: Arc<Mutex<Option<Taps>>>,
    /// The UI wants the `telemetry` feed (Play mode, window visible).
    telemetry_on: Arc<AtomicBool>,
    /// Dropout counter of the current stream (a leaf lock).
    audio_stats: Arc<Mutex<Arc<AudioStats>>>,
    /// Undo/redo of kit edits; lock after `router`.
    pub history: Arc<Mutex<History>>,
    /// What the app opened at launch, or the set opened since.
    pub startup: Arc<Mutex<Startup>>,
    config: Arc<Mutex<ConfigFile>>,
    /// Known MIDI devices (a leaf lock; the Launchpad bridge and the input poller report here).
    pub midi: devices::SharedRegistry,
    /// The newest `keys-wheel` event not emitted yet (a leaf lock): wheel messages arrive in
    /// floods, so the emitter thread sends only the latest value at ~60 Hz.
    wheel: Arc<Mutex<Option<UiEvent>>>,
    /// The Launch Control XL moved the mixer since `mixer-changed` was last emitted.
    mixer_dirty: Arc<AtomicBool>,
    /// A learned control changed the set (bank parameters) since `set-changed` was last emitted.
    set_dirty: Arc<AtomicBool>,
    /// Which engine plays the keys, and the hook that tells the UI when a background reload changes it.
    keys_status: Arc<Mutex<KeysStatus>>,
    keys_notifier: Arc<Mutex<Option<KeysNotifier>>>,
    /// The sequencer clock state (shared with the router's record hook and the clock thread).
    seq: Arc<Mutex<SeqState>>,
    saver: Arc<Mutex<Saver>>,
    save_gate: Arc<Mutex<()>>,
    writer: Writer,
    /// Newest set-open request; an older one finishing later is discarded.
    open_ticket: Arc<AtomicU64>,
    /// Count of committed set installs; `adopt` acts only for the latest.
    installed: Arc<AtomicU64>,
    assigns: Arc<Mutex<Assigns>>,
    input_mode: Arc<AtomicU8>,
}

impl AppState {
    pub fn new(set: Set, audio_tx: Producer<Command>, audio_rx: Consumer<Event>) -> Self {
        Self::with_writer(set, audio_tx, audio_rx, Arc::new(kit::save))
    }

    /// `writer` performs the actual disk save (replaced in tests to inject failures and delays).
    pub fn with_writer(
        set: Set,
        audio_tx: Producer<Command>,
        audio_rx: Consumer<Event>,
        writer: Writer,
    ) -> Self {
        let router = Router::new(set, Default::default(), audio_tx, audio_rx);
        let seq = router.seq_handle();
        let kit_manager = KitManager {
            dir: None,
            samples: Default::default(),
            failed_samples: Default::default(),
        };
        Self {
            router: Arc::new(Mutex::new(router)),
            painter: Arc::new(Mutex::new(LightPainter::new())),
            kit_manager: Arc::new(Mutex::new(kit_manager)),
            audio_status: Arc::new(Mutex::new(AudioStatus::default())),
            taps: Arc::default(),
            telemetry_on: Arc::new(AtomicBool::new(false)),
            audio_stats: Arc::new(Mutex::new(Arc::new(AudioStats::new()))),
            history: Arc::new(Mutex::new(History::new())),
            startup: Arc::new(Mutex::new(Startup::FirstRun)),
            config: Arc::default(),
            midi: Arc::default(),
            wheel: Arc::default(),
            mixer_dirty: Arc::new(AtomicBool::new(false)),
            set_dirty: Arc::new(AtomicBool::new(false)),
            keys_status: Arc::new(Mutex::new(KeysStatus::builtin())),
            keys_notifier: Arc::default(),
            seq,
            saver: Arc::new(Mutex::new(Saver::new())),
            save_gate: Arc::new(Mutex::new(())),
            writer,
            open_ticket: Arc::new(AtomicU64::new(0)),
            installed: Arc::new(AtomicU64::new(0)),
            assigns: Arc::new(Mutex::new(Assigns::default())),
            input_mode: Arc::new(AtomicU8::new(InputMode::Sampler as u8)),
        }
    }

    /// The one kit-edit path: records undo (coalescing by `key`), applies `f` to the
    /// router's set, re-sends mixer commands when mixer values changed and marks the set dirty.
    /// An edit that leaves the set unchanged adds no undo step.
    pub fn edit<R>(&self, key: Option<&str>, f: impl FnOnce(&mut Set) -> R) -> R {
        self.edit_at(key, Instant::now(), f)
    }

    pub fn edit_at<R>(&self, key: Option<&str>, now: Instant, f: impl FnOnce(&mut Set) -> R) -> R {
        let mut r = lock(&self.router);
        let out = self.edit_in(&mut r, key, now, f);
        drop(r);
        self.touch();
        out
    }

    fn edit_in<R>(
        &self,
        r: &mut Router,
        key: Option<&str>,
        now: Instant,
        f: impl FnOnce(&mut Set) -> R,
    ) -> R {
        let before = r.set().clone();
        let out = f(r.set_mut());
        if *r.set() != before {
            lock(&self.history).record(&before, key, now);
            if crate::router::mixer_audio_differs(&before, r.set()) {
                r.send_mixer();
            }
        }
        out
    }

    /// The set changed: schedule an autosave.
    pub(crate) fn touch(&self) {
        lock(&self.saver).touch(Instant::now());
    }

    pub fn undo(&self) -> Option<Set> {
        self.step_history(|h, current| h.undo(current))
    }

    pub fn redo(&self) -> Option<Set> {
        self.step_history(|h, current| h.redo(current))
    }

    fn step_history(&self, step: impl FnOnce(&mut History, &Set) -> Option<Set>) -> Option<Set> {
        let mut r = lock(&self.router);
        let set = {
            let mut h = lock(&self.history);
            step(&mut h, r.set())?
        };
        r.install_set(set.clone());
        drop(r);
        self.touch();
        Some(set)
    }

    pub fn history_state(&self) -> HistoryState {
        let h = lock(&self.history);
        HistoryState {
            can_undo: h.can_undo(),
            can_redo: h.can_redo(),
        }
    }

    pub fn trigger_pad(&self, x: u8, y: u8) {
        lock(&self.router).trigger_pad(x, y);
        self.record_hit(x, y);
    }

    pub fn release_pad(&self, x: u8, y: u8) {
        lock(&self.router).release_pad(x, y);
    }

    pub fn select_bank(&self, bank: usize) -> bool {
        lock(&self.router).select_bank(bank)
    }

    pub fn current_bank(&self) -> usize {
        lock(&self.router).bank()
    }

    pub fn stop_all(&self) {
        lock(&self.router).stop_all();
    }

    // ---- step sequencer (the clock thread lives in `sequencer.rs`) ------------------------

    pub fn seq_handle(&self) -> Arc<Mutex<SeqState>> {
        self.seq.clone()
    }

    /// Tempo, swing and length as the clock needs them.
    pub fn pattern_params(&self) -> Params {
        Params::from(&lock(&self.router).set().pattern)
    }

    /// Fire due sequencer steps (the router lock is taken here, the sequencer state is not held).
    pub fn seq_fire(&self, steps: &[usize], skip: &HashSet<(usize, usize)>) -> Vec<Fired> {
        let (fired, active) = {
            let mut r = lock(&self.router);
            let fired = r.seq_fire(steps, skip);
            (fired, r.bank())
        };
        for f in fired.iter().filter(|f| f.bank == active) {
            self.record_hit(f.x, f.y);
        }
        fired
    }

    pub fn seq_release(&self, bank: usize, x: u8, y: u8) {
        lock(&self.router).release_pad_in(bank, x, y);
    }

    /// The set changed behind the UI's back (recording): autosave, and tell the screen.
    pub fn mark_set_dirty(&self) {
        self.set_dirty.store(true, Ordering::SeqCst);
        self.touch();
    }

    /// The undo step of a recording take: the set as it is before REC writes into it. (`edit`
    /// records only when the set changed, and nothing has changed yet.)
    pub fn begin_take(&self) {
        let r = lock(&self.router);
        let before = r.set().clone();
        lock(&self.history).record(&before, None, Instant::now());
    }

    pub fn seq_play(&self, play: bool) {
        if play {
            let params = self.pattern_params();
            crate::sequencer::lock(&self.seq).start(Instant::now(), params);
        } else {
            crate::sequencer::lock(&self.seq).stop();
        }
    }

    /// Arming starts playback if it is stopped and opens the take's undo step; disarming leaves
    /// playback running.
    pub fn seq_rec(&self, armed: bool) {
        if !armed {
            crate::sequencer::lock(&self.seq).rec = false;
            return;
        }
        if crate::sequencer::lock(&self.seq).rec {
            return;
        }
        self.begin_take();
        let params = self.pattern_params();
        let mut s = crate::sequencer::lock(&self.seq);
        s.rec = true;
        if !s.playing {
            s.start(Instant::now(), params);
        }
    }

    /// Replace the pattern (validated). `key` folds a drag (`seq:bpm`, `seq:swing`, `seq:paint`)
    /// into one undo step.
    pub fn seq_update(
        &self,
        pattern: lpd_core::seq::Pattern,
        key: Option<&str>,
    ) -> Result<Set, String> {
        let mut pattern = pattern;
        lpd_core::seq::validate(&mut pattern)?;
        Ok(self.edit(key, |set| {
            set.pattern = pattern;
            set.clone()
        }))
    }

    pub fn seq_add_lane(&self, bank: usize, x: u8, y: u8) -> Result<Set, String> {
        if bank >= kit::BANKS || x > 7 || y > 7 {
            return Err("Pad does not exist".into());
        }
        let filled = lock(&self.router).set().banks[bank]
            .pad(x, y)
            .is_some_and(|p| !p.sample.is_empty());
        if !filled {
            return Err("Pad is empty".into());
        }
        let mut added = Ok(());
        let set = self.edit(None, |set| {
            added = lpd_core::seq::add_lane(&mut set.pattern, bank as u8, x, y);
            set.clone()
        });
        added.map(|_| set).map_err(str::to_string)
    }

    /// Sound a step's pad while the clock is stopped (not recorded, no history).
    pub fn seq_audition_step(&self, bank: usize, x: u8, y: u8) {
        if crate::sequencer::lock(&self.seq).playing {
            return;
        }
        let (sent, active) = {
            let mut r = lock(&self.router);
            let vel = f32::from(lpd_core::seq::VEL_ON) / 127.0;
            (r.trigger_pad_quiet(bank, x, y, vel), r.bank())
        };
        if sent && bank == active {
            self.record_hit(x, y);
        }
    }

    /// Flash/ripple bookkeeping; the router lock is already released.
    fn record_hit(&self, x: u8, y: u8) {
        lock(&self.painter).on_pad_hit(x, y, Instant::now());
    }

    /// Hardware input. Ignored while a game owns the pads (decided under the router lock, so a
    /// mode switch cannot interleave with a half-handled event). Returns the `learn-ignored`
    /// event when learning is armed (the Launchpad is not learnable).
    pub fn on_lp_event(&self, ev: LpEvent) -> Option<UiEvent> {
        let ignored = {
            let mut r = lock(&self.router);
            if self.input_mode() != InputMode::Sampler {
                return None;
            }
            r.on_lp_event(ev);
            match ev {
                LpEvent::PadDown { .. } | LpEvent::SideDown(_) | LpEvent::TopDown(_) => {
                    r.learn_ignore("Launchpad pads are fixed")
                }
                _ => None,
            }
        };
        if let LpEvent::PadDown { x, y } = ev {
            self.record_hit(x, y);
        }
        ignored
    }

    /// A message from a generic MIDI input. Every message updates the router (wheel tracking,
    /// learn, bindings); `log` says whether it also gets a `midi-log` line (the caller throttles
    /// the controller floods). The returned events are for the caller to emit once no lock is
    /// held; wheel moves are not returned but coalesced (see `take_wheel`). A learned binding
    /// is an undoable edit; a learned parameter is a mixer change (not undoable) or a set edit
    /// coalesced under `learn:{id}`. The log line is formatted after the lock is released.
    pub fn on_midi(&self, port: &str, role: &str, msg: MidiMsg, log: bool) -> Vec<UiEvent> {
        let now = Instant::now();
        let mut events = Vec::with_capacity(3);
        let mut r = lock(&self.router);
        let out = r.on_midi(port, role, msg);
        let active_bank = r.bank();
        let mut touched = out.mixer_changed;
        if let Some(binding) = out.bind {
            self.edit_in(&mut r, None, now, |set| {
                learn::bind(&mut set.learn, binding.clone())
            });
            touched = true;
            events.push(UiEvent::LearnBound { binding });
        }
        if let Some((id, value)) = out.set_param {
            let key = format!("learn:{id}");
            let changed = self.edit_in(&mut r, Some(&key), now, |set| {
                learn::apply_set_param(set, &id, value)
            });
            if changed {
                self.set_dirty.store(true, Ordering::SeqCst);
                touched = true;
                if id.starts_with("keys.") {
                    r.send_keys();
                }
            }
        }
        drop(r);
        if out.mixer_changed {
            self.mixer_dirty.store(true, Ordering::SeqCst);
        }
        if touched {
            self.touch();
        }
        if let Some((bank, x, y)) = out.hit {
            if bank == active_bank {
                self.record_hit(x, y);
            }
        }
        match out.event {
            Some(wheel @ UiEvent::KeysWheel { .. }) => *lock(&self.wheel) = Some(wheel),
            Some(other) => events.push(other),
            None => {}
        }
        if log {
            events.push(UiEvent::MidiLog {
                text: format!("{port}: {msg:?}"),
            });
        }
        events
    }

    /// Arm learning for `target` (replacing any armed target).
    pub fn arm_learn(&self, target: LearnTarget) {
        lock(&self.router).arm_learn(target);
    }

    pub fn cancel_learn(&self) {
        lock(&self.router).cancel_learn();
    }

    /// Remove one binding (an undoable edit); returns the set.
    pub fn unbind(&self, index: usize) -> Set {
        self.edit(None, |set| {
            learn::unbind(&mut set.learn, index);
            set.clone()
        })
    }

    /// The latest unsent `keys-wheel` event, if any (called every ~16 ms by the emitter thread,
    /// so the last value always arrives).
    pub fn take_wheel(&self) -> Option<UiEvent> {
        lock(&self.wheel).take()
    }

    /// A keys input closed or changed role: its wheels go back to rest.
    pub fn keys_input_closed(&self) {
        let reset = lock(&self.router).reset_keys_wheels();
        if reset.is_some() {
            *lock(&self.wheel) = reset;
        }
    }

    pub fn midi_devices(&self) -> Vec<MidiDevice> {
        lock(&self.midi).list()
    }

    pub fn midi_status(&self, port: &str) -> Option<String> {
        lock(&self.midi).get(port).map(|d| d.status.clone())
    }

    /// Choose how a generic input is used and remember it. Launchpad and LCXL ports have a fixed
    /// role. Returns the new device list when the registry changed (the poller reopens the
    /// port on its next round).
    pub fn set_midi_role(&self, port: &str, role: &str) -> Result<Option<Vec<MidiDevice>>, String> {
        if !matches!(role, "keys" | "other") {
            return Err(format!("Unknown MIDI role: {role}"));
        }
        let current = devices::classify(port, &self.get_config().midi_roles);
        if matches!(current, "launchpad" | "lcxl") {
            return Err(format!("{port} has a fixed role"));
        }
        self.update_config(|c| {
            c.midi_roles.insert(port.to_string(), role.to_string());
        })?;
        let mut registry = lock(&self.midi);
        let Some(status) = registry.get(port).map(|d| d.status.clone()) else {
            return Ok(None);
        };
        Ok(registry.set(port, role, &status).then(|| registry.list()))
    }

    /// A Launch Control XL control. Mixer values are not undoable (UI spec 7.3), but the set is
    /// dirty for autosave and the screen is told (throttled, see `take_mixer_change`). Returns
    /// the `learn-ignored` event when learning is armed (the LCXL is fixed in Part 1).
    pub fn on_lcxl(&self, ev: LcxlEvent) -> Option<UiEvent> {
        let (changed, ignored) = {
            let mut r = lock(&self.router);
            let changed = r.on_lcxl(ev);
            (changed, r.learn_ignore("LCXL controls are fixed in Part 1"))
        };
        if changed {
            self.mixer_dirty.store(true, Ordering::SeqCst);
            self.touch();
        }
        ignored
    }

    /// True once per hardware mixer change; the LCXL bridge emits `mixer-changed` for it.
    pub fn take_mixer_change(&self) -> bool {
        self.mixer_dirty.swap(false, Ordering::SeqCst)
    }

    /// True once per learned set change; the sync thread emits `set-changed` for it.
    pub fn take_set_change(&self) -> bool {
        self.set_dirty.swap(false, Ordering::SeqCst)
    }

    pub fn mixer(&self) -> lpd_core::kit::Mixer {
        lock(&self.router).set().mixer.clone()
    }

    /// Mute and solo of the eight channels, for the Launch Control XL LEDs.
    pub fn mute_solo(&self) -> ([bool; 8], [bool; 8]) {
        let r = lock(&self.router);
        let (mut mute, mut solo) = ([false; 8], [false; 8]);
        for (i, g) in r.set().mixer.groups.iter().take(8).enumerate() {
            mute[i] = g.mute;
            solo[i] = g.solo;
        }
        (mute, solo)
    }

    pub fn input_mode(&self) -> InputMode {
        if self.input_mode.load(Ordering::SeqCst) == InputMode::Game as u8 {
            InputMode::Game
        } else {
            InputMode::Sampler
        }
    }

    /// Hand the pads and LEDs to the sampler or a game. Entering a game releases every held
    /// sampler pad first; either change forces a full repaint. Returns the sampler's bank.
    pub fn set_input_mode(&self, mode: InputMode) -> usize {
        let mut r = lock(&self.router);
        let previous = self.input_mode.swap(mode as u8, Ordering::SeqCst);
        if previous != mode as u8 && mode == InputMode::Game {
            r.release_all_held();
        }
        let bank = r.bank();
        drop(r);
        if previous != mode as u8 {
            lock(&self.painter).invalidate();
        }
        bank
    }

    /// Sampler frame for the active bank. The router lock is held only to copy a snapshot; the
    /// painter works on the copy.
    pub fn paint_frame(&self, now: Instant) -> Vec<(u8, [u8; 3])> {
        let snapshot = lock(&self.router).paint_snapshot();
        match snapshot {
            Some(snap) => lock(&self.painter).paint_snapshot(&snap, now),
            None => vec![],
        }
    }

    /// Housekeeping from the device thread: voice events in, freed samples out. The samples are
    /// dropped here, after the router lock is released, never on the audio thread.
    pub fn maintain(&self) {
        let (reclaimed, trash) = {
            let mut r = lock(&self.router);
            r.poll_audio_events();
            (r.drain_reclaimable(), r.drain_trash())
        };
        drop(reclaimed);
        // Replaced keys instruments (a SoundFont synth is big) are freed here, not on the audio thread.
        drop(trash);
    }

    pub fn set_audio_status(&self, status: AudioStatus) {
        *lock(&self.audio_status) = status;
    }

    pub fn get_audio_status(&self) -> AudioStatus {
        let status = lock(&self.audio_status).clone();
        self.with_live(status)
    }

    /// The current output rate, without cloning the whole status.
    pub fn sample_rate(&self) -> u32 {
        lock(&self.audio_status).sample_rate
    }

    /// Fill the dropout and latency fields from the live counters (the stored status never
    /// carries them).
    fn with_live(&self, mut status: AudioStatus) -> AudioStatus {
        let stats = lock(&self.audio_stats).clone();
        status.dropouts = stats.dropouts();
        status.last_dropout_ago_ms = stats.last_dropout_ago_ms(Instant::now());
        let buffer = status.buffer_size.unwrap_or_else(|| stats.frames());
        status.latency = audio::latency(buffer, status.sample_rate, stats.output_us());
        status
    }

    /// The output stream's error callback fired. Returns the new status the first time only
    /// (a burst of errors reaches the UI once), and only for a stream that was running.
    pub fn poll_stream_lost(&self) -> Option<AudioStatus> {
        let stats = lock(&self.audio_stats).clone();
        if !stats.take_lost() {
            return None;
        }
        let lost = {
            let mut status = lock(&self.audio_status);
            if status.state == "running" {
                status.state = "lost".into();
                Some(status.clone())
            } else {
                None
            }
        };
        lost.map(|s| self.with_live(s))
    }

    /// A new output stream is running (called by the audio owner thread): the router gets its
    /// queues and the mixer, the telemetry thread its taps, and the status its counters.
    pub fn install_audio(
        &self,
        commands: Producer<Command>,
        events: Consumer<Event>,
        taps: Taps,
        stats: Arc<AudioStats>,
        status: AudioStatus,
    ) -> AudioStatus {
        lock(&self.router).replace_audio(commands, events);
        self.set_taps(Some(taps));
        // Status before stats: an error the new stream flags at once is then seen against the
        // new status, not overwritten by it.
        self.set_audio_status(status);
        self.set_audio_stats(stats);
        // Pad lights followed voices of the old stream.
        lock(&self.painter).invalidate();
        self.reload_soundfont();
        self.get_audio_status()
    }

    /// The new engine starts with the built-in synth; a configured SoundFont is loaded again for
    /// its sample rate, in the background.
    fn reload_soundfont(&self) {
        let Some(path) = self.get_config().soundfont else {
            *lock(&self.keys_status) = KeysStatus::builtin();
            return;
        };
        let state = self.clone();
        std::thread::spawn(move || {
            let status = state.load_keys_instrument(Some(&path));
            if let Some(notify) = lock(&state.keys_notifier).clone() {
                notify(&status);
            }
        });
    }

    /// The router takes the instrument-trash queue of the new engine.
    pub fn install_trash(&self, trash: Option<Consumer<Box<dyn Instrument>>>) {
        lock(&self.router).set_trash(trash);
    }

    pub fn set_keys_notifier(&self, notifier: KeysNotifier) {
        *lock(&self.keys_notifier) = Some(notifier);
    }

    pub fn keys_status(&self) -> KeysStatus {
        lock(&self.keys_status).clone()
    }

    /// Build the keys instrument (the built-in synth, or the SoundFont at `path`) for the current
    /// output rate and hand it to the engine. A load error keeps the current instrument. Slow:
    /// call it off the UI thread. Without a running stream nothing is loaded (the engine does
    /// it when the stream starts).
    pub fn load_keys_instrument(&self, path: Option<&str>) -> KeysStatus {
        let rate = self.sample_rate();
        let status = match path {
            None => {
                if rate > 0 {
                    lock(&self.router).set_instrument(Box::new(BasicKeys::new(rate)));
                }
                KeysStatus::builtin()
            }
            Some(_) if rate == 0 => KeysStatus::builtin(),
            Some(p) => match SoundFontKeys::load(Path::new(p), rate) {
                Ok(keys) => {
                    lock(&self.router).set_instrument(Box::new(keys));
                    KeysStatus {
                        engine: "soundfont".into(),
                        name: Path::new(p)
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_uppercase())
                            .unwrap_or_else(|| "SOUNDFONT".into()),
                        error: None,
                    }
                }
                Err(e) => {
                    let file = Path::new(p)
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.to_string());
                    logfile::log(format!("SoundFont {p}: {e:#}"));
                    KeysStatus {
                        error: Some(format!("Couldn't load {file}: {e:#}")),
                        ..self.keys_status()
                    }
                }
            },
        };
        *lock(&self.keys_status) = status.clone();
        status
    }

    /// The stream could not be (re)started: no taps, fresh counters, status "error".
    pub fn audio_failed(&self, prefs: &AudioPrefs) {
        // The old stream is gone with its voices: nothing may stay "sounding" or held.
        lock(&self.router).forget_voices();
        lock(&self.painter).invalidate();
        self.set_taps(None);
        self.set_audio_stats(Arc::new(AudioStats::new()));
        let mut status = lock(&self.audio_status);
        status.state = "error".into();
        status.backend = if prefs.backend.eq_ignore_ascii_case("asio") {
            "ASIO"
        } else {
            "WASAPI"
        }
        .into();
        status.asio_fallback = false;
        if let Some(device) = &prefs.device {
            status.device_name = device.clone();
        }
        status.buffer_size = prefs.buffer_frames;
    }

    /// Remember the audio prefs (`set_audio`); the stream restarts separately.
    pub fn set_audio_prefs(&self, prefs: AudioPrefs) -> Result<(), String> {
        self.update_config(|c| c.audio = prefs)
    }

    pub fn set_taps(&self, taps: Option<Taps>) {
        *lock(&self.taps) = taps;
    }

    /// Point the status and `reset_dropouts` at the counter of a new stream.
    pub fn set_audio_stats(&self, stats: Arc<AudioStats>) {
        *lock(&self.audio_stats) = stats;
    }

    pub fn dropout_count(&self) -> u32 {
        lock(&self.audio_stats).dropouts()
    }

    pub fn reset_dropouts(&self) {
        lock(&self.audio_stats).reset();
    }

    pub fn set_telemetry(&self, on: bool) {
        self.telemetry_on.store(on, Ordering::Relaxed);
    }

    pub fn telemetry_enabled(&self) -> bool {
        self.telemetry_on.load(Ordering::Relaxed)
    }

    pub fn get_current_set(&self) -> Set {
        lock(&self.router).set().clone()
    }

    /// Replace a pad's parameters. `key` names a continuous control (for example
    /// `pad:0:2:3:volume`) so a drag is one undo step. An existing pad keeps its sample (a stale
    /// editor copy must not revert an assignment); use `assign_sample` to change it.
    pub fn update_pad(&self, bank: usize, mut pad: Pad, key: Option<&str>) {
        self.edit(key, |set| {
            if let Some(b) = set.banks.get_mut(bank) {
                if let Some(old) = b.pad(pad.x, pad.y) {
                    pad.sample = old.sample.clone();
                }
                b.set_pad(pad);
            }
        });
    }

    pub fn clear_pad(&self, bank: usize, x: u8, y: u8) -> Set {
        self.edit(None, |set| {
            kit::ops::clear_pad(set, bank, x, y);
            set.clone()
        })
    }

    pub fn move_pad(&self, from: (usize, u8, u8), to: (usize, u8, u8), copy: bool) -> Option<Set> {
        self.edit(None, |set| {
            kit::ops::move_pad(set, from, to, copy).map(|_| set.clone())
        })
    }

    /// Decode outside locks, then publish the entire batch as one edit in the original set.
    pub fn drop_files(
        &self,
        bank: usize,
        x: u8,
        y: u8,
        paths: Vec<String>,
    ) -> Result<DropReport, String> {
        let (audio, rejected): (Vec<_>, Vec<_>) = paths
            .into_iter()
            .partition(|p| kit::ops::is_audio_path(Path::new(p)));
        let mut rejected = rejected.len();
        let (tickets, before) = {
            let r = lock(&self.router);
            let b = r.set().banks.get(bank).ok_or("That bank does not exist.")?;
            if x > 7 || y > 7 {
                return Err("That pad does not exist.".into());
            }
            let positions = kit::ops::drop_positions(b, x, y, audio.len());
            let mut tickets = Vec::new();
            let mut before = Vec::new();
            for (x, y) in positions {
                tickets.push(self.begin_assign(bank, x, y)?);
                before.push(b.pad(x, y).cloned());
            }
            (tickets, before)
        };
        let leftover = audio.len() - tickets.len();
        let mut prepared = Vec::new();
        for (i, (ticket, path)) in tickets.iter().zip(audio).enumerate() {
            match prepare_sample(&ticket.dir, &path) {
                Ok((rel, sample)) => prepared.push((i, rel, sample)),
                Err(e) => {
                    rejected += 1;
                    logfile::log(format!("{path}: {e:#}"));
                }
            }
        }
        let mut r = lock(&self.router);
        for (i, _, _) in &prepared {
            let t = &tickets[*i];
            self.check_assignment(&r, t)?;
            if r.set().banks[bank].pad(t.x, t.y) != before[*i].as_ref() {
                return Err("A target pad changed while the samples were loading.".into());
            }
        }
        let imported = prepared.len();
        let mut assignments = Vec::new();
        for (i, rel, sample) in prepared {
            let t = &tickets[i];
            self.commit_sample(&mut r, t.epoch, &rel, sample)?;
            assignments.push((t.x, t.y, rel));
        }
        self.edit_in(&mut r, None, Instant::now(), |set| {
            for (x, y, rel) in assignments {
                kit::ops::assign_sample(&mut set.banks[bank], x, y, rel);
            }
        });
        let set = r.set().clone();
        drop(r);
        if imported > 0 {
            self.touch();
        }
        Ok(DropReport {
            set,
            imported,
            leftover,
            rejected,
        })
    }

    fn check_assignment(&self, _r: &Router, ticket: &AssignTicket) -> Result<(), String> {
        if lock(&self.saver).epoch() != ticket.epoch {
            return Err(STALE_SET.into());
        }
        if lock(&self.assigns)
            .latest
            .get(&(ticket.bank, ticket.x, ticket.y))
            != Some(&ticket.seq)
        {
            return Err("A newer sample was assigned to this pad meanwhile.".into());
        }
        Ok(())
    }

    pub fn locate_sample(&self, bank: usize, x: u8, y: u8, path: &str) -> Result<Set, String> {
        let (ticket, before) = {
            let r = lock(&self.router);
            let before = r
                .set()
                .banks
                .get(bank)
                .and_then(|b| b.pad(x, y))
                .cloned()
                .ok_or("Pad is empty")?;
            (self.begin_assign(bank, x, y)?, before)
        };
        let (rel, sample) = prepare_sample(&ticket.dir, path).map_err(|e| format!("{e:#}"))?;
        let mut r = lock(&self.router);
        self.check_assignment(&r, &ticket)?;
        if r.set().banks[bank].pad(x, y) != Some(&before) {
            return Err("The pad changed while the sample was loading.".into());
        }
        self.commit_sample(&mut r, ticket.epoch, &rel, sample)?;
        self.edit_in(&mut r, None, Instant::now(), |set| {
            let mut pad = before.clone();
            pad.sample = rel;
            set.banks[bank].set_pad(pad);
        });
        lock(&self.kit_manager)
            .failed_samples
            .retain(|(p, _)| p != &before.sample);
        let set = r.set().clone();
        drop(r);
        self.touch();
        Ok(set)
    }

    /// A rename is an undoable kit edit; value changes (volume, pan, ...) are not (UI spec 7.3).
    /// Both happen under one router lock and mark the set dirty once.
    pub fn update_group(&self, group: usize, g: lpd_core::kit::Group) {
        let mut r = lock(&self.router);
        let Some(old) = r.set().mixer.groups.get(group) else {
            return;
        };
        if old.name != g.name {
            let name = g.name.clone();
            self.edit_in(&mut r, None, Instant::now(), |set| {
                set.mixer.groups[group].name = name
            });
        }
        r.update_group(group, g);
        drop(r);
        self.touch();
    }

    pub fn update_master_db(&self, master_db: f32) {
        lock(&self.router).update_master_db(master_db);
        self.touch();
    }

    pub fn update_bank_lights(&self, bank: usize, lights: lpd_core::kit::LightModes) {
        self.edit(None, |set| {
            if let Some(b) = set.banks.get_mut(bank) {
                b.lights = lights;
            }
        });
    }

    pub fn update_bank_humanize(&self, bank: usize, humanize_db: f32) {
        self.update_bank_humanize_at(bank, humanize_db, Instant::now());
    }

    /// `update_bank_humanize` at a given time (tests inject it to check coalescing).
    pub fn update_bank_humanize_at(&self, bank: usize, humanize_db: f32, now: Instant) {
        let humanize_db = humanize_db.clamp(0.0, 6.0);
        let key = format!("bank:{bank}:humanize");
        self.edit_at(Some(&key), now, |set| {
            if let Some(b) = set.banks.get_mut(bank) {
                b.humanize_db = humanize_db;
            }
        });
    }

    /// Peak envelope of a loaded sample for the editor waveform: `buckets` max-abs values
    /// over both channels. None when the sample is not loaded.
    pub fn sample_peaks(&self, path: &str, buckets: usize) -> Option<SamplePeaks> {
        let sample = lock(&self.kit_manager).samples.get(path)?.clone();
        let frames = sample.frames();
        let buckets = buckets.clamp(16, 2048);
        let mut peaks = vec![0.0f32; buckets];
        if frames > 0 {
            for (i, peak) in peaks.iter_mut().enumerate() {
                let start = i * frames / buckets;
                let end = ((i + 1) * frames / buckets).clamp(start + 1, frames);
                for f in start..end {
                    let (l, r) = sample.frame(f as isize);
                    *peak = peak.max(l.abs()).max(r.abs());
                }
            }
        }
        Some(SamplePeaks {
            peaks,
            length_ms: frames as f32 * 1000.0 / sample.rate.max(1) as f32,
            rate: sample.rate,
        })
    }

    pub fn rename_bank(&self, bank: usize, name: String) {
        self.edit(None, |set| {
            if let Some(b) = set.banks.get_mut(bank) {
                b.name = name;
            }
        });
    }

    // ---- opening and creating sets -------------------------------------------------------

    /// Start an open/new request. Only the newest request may commit.
    pub fn begin_open(&self) -> u64 {
        self.open_ticket.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Launch: load the config at `config_path`, reopen its last set when possible and
    /// remember the outcome for `get_startup`. Slow: loads samples.
    pub fn restore(&self, config_path: PathBuf) -> Startup {
        let loaded_config = config::load(&config_path);
        let last = loaded_config.last_set.clone();
        *lock(&self.config) = ConfigFile {
            path: Some(config_path),
            config: loaded_config,
        };
        let (decision, kit) =
            startup::decide_startup(last.as_deref().map(Path::new), KitManager::load_from_dir);
        let decision = match (decision, kit) {
            (Startup::Loaded { path, name, .. }, Some(kit)) => {
                let ticket = self.begin_open();
                match self.finish_open(ticket, kit) {
                    Ok(_) => self.get_startup(),
                    Err(reason) => {
                        logfile::log(format!("Startup: couldn't install {path}: {reason}"));
                        Startup::Unreadable { path, name }
                    }
                }
            }
            (other, _) => other,
        };
        logfile::log(format!("Startup: {decision:?}"));
        *lock(&self.startup) = decision.clone();
        decision
    }

    pub fn get_startup(&self) -> Startup {
        lock(&self.startup).clone()
    }

    /// Open a set folder. A failure changes nothing. Slow: call from a worker thread.
    pub fn open_set(&self, dir_str: &str) -> Result<OpenResult, String> {
        let ticket = self.begin_open();
        let dir = Path::new(dir_str);
        let loaded = KitManager::load_from_dir(dir).map_err(|e| {
            logfile::log(format!("Open {}: {e:#}", dir.display()));
            open_error(dir, &e)
        })?;
        self.finish_open(ticket, loaded)
    }

    /// Commit a loaded kit if no newer open/new request started meanwhile.
    pub fn finish_open(&self, ticket: u64, loaded: LoadedKit) -> Result<OpenResult, String> {
        for (path, error) in &loaded.failed_samples {
            logfile::log(format!("{path}: {error}"));
        }
        let set = loaded.set.clone();
        let dir = loaded.dir.clone();
        let seq = self.install(
            ticket,
            loaded.set,
            Some(loaded.dir),
            loaded.samples,
            loaded.failed_samples,
        )?;
        if loaded.from_backup {
            // set.json was moved aside: let autosave write it back.
            self.touch();
        }
        Ok(self.adopt(seq, &dir, set, loaded.from_backup))
    }

    /// A set is now open: it becomes the startup state and the last set in the config, unless
    /// a newer install got there first (`seq` is checked under the config lock).
    fn adopt(&self, seq: u64, dir: &Path, set: Set, from_backup: bool) -> OpenResult {
        let result = OpenResult {
            set,
            path: dir.display().to_string(),
            name: startup::set_name(dir),
            from_backup,
            backup_saved_at: startup::backup_time(dir),
        };
        let mut c = lock(&self.config);
        if self.installed.load(Ordering::SeqCst) != seq {
            return result;
        }
        *lock(&self.startup) = Startup::Loaded {
            path: result.path.clone(),
            name: result.name.clone(),
            from_backup,
            backup_saved_at: result.backup_saved_at.clone(),
        };
        let last = Some(result.path.clone());
        let written = write_config(&mut c, |c| c.last_set = last);
        drop(c);
        if let Err(e) = written {
            logfile::log(e);
        }
        logfile::log(format!("Opened set {}", result.path));
        result
    }

    /// Create an empty set in a folder that holds no kit. Nothing on disk or in memory changes
    /// when the folder already holds one. Slow: call from a worker thread.
    pub fn new_set(&self, dir_str: &str) -> Result<OpenResult, String> {
        let ticket = self.begin_open();
        let dir = Path::new(dir_str);
        kit::ensure_new_set_dir(dir).map_err(|e| format!("{e:#}"))?;
        let created_dir = !dir.exists();
        std::fs::create_dir_all(dir).map_err(|e| format!("{e:#}"))?;
        let set = Set::new();
        kit::save(dir, &set).map_err(|e| format!("{e:#}"))?;
        let installed = self.install(
            ticket,
            set.clone(),
            Some(dir.to_path_buf()),
            Default::default(),
            vec![],
        );
        let seq = match installed {
            Ok(seq) => seq,
            Err(reason) => {
                // Not adopted: take back what this call created so a retry finds the folder free.
                let _ = std::fs::remove_file(dir.join(kit::SET_FILE));
                if created_dir {
                    let _ = std::fs::remove_dir(dir);
                }
                return Err(reason);
            }
        };
        Ok(self.adopt(seq, dir, set, false))
    }

    // ---- app config ----------------------------------------------------------------------

    pub fn get_config(&self) -> AppConfig {
        lock(&self.config).config.clone()
    }

    /// Change the config and write it (when it has a file). The `config` leaf lock is held
    /// across its own small write so writes land in order; nothing else waits on it (not
    /// `save_gate`: opening a set must not wait for a set save in flight).
    fn update_config(&self, f: impl FnOnce(&mut AppConfig)) -> Result<(), String> {
        write_config(&mut lock(&self.config), f)
    }

    pub fn set_library_dir(&self, dir: Option<String>) -> Result<(), String> {
        self.update_config(|c| c.library_dir = dir)
    }

    /// Remembers the `.sf2` file and loads it (slow: run it off the UI thread). `None` goes back
    /// to the built-in synth.
    pub fn set_soundfont(&self, path: Option<String>) -> Result<KeysStatus, String> {
        self.update_config(|c| c.soundfont = path.clone())?;
        Ok(self.load_keys_instrument(path.as_deref()))
    }

    /// A key played on the screen: the engine and the mirror (`keys-note`, for the caller to emit).
    pub fn keys_note(&self, note: u8, velocity: u8, on: bool) -> UiEvent {
        lock(&self.router).keys_note(note, velocity, on);
        UiEvent::KeysNote {
            note,
            velocity: if on { velocity } else { 0 },
            on,
        }
    }

    /// The screen's pitch wheel, −1..1.
    pub fn keys_pitch(&self, value: f32) {
        lock(&self.router).keys_pitch_value(value);
    }

    pub fn keys_mod(&self, value: f32) {
        lock(&self.router).keys_mod_value(value);
    }

    /// The keys settings are an undoable edit (a slider drag is one step); the engine follows.
    pub fn set_keys(&self, settings: KeysSettings) -> Set {
        let set = self.edit(Some("keys"), |set| {
            set.keys = settings;
            set.clone()
        });
        lock(&self.router).send_keys();
        set
    }

    /// Pads whose sample failed to load or is not loaded: bank, then top row first, then x.
    pub fn missing_samples(&self) -> Vec<MissingPad> {
        let r = lock(&self.router);
        let km = lock(&self.kit_manager);
        let mut missing: Vec<MissingPad> = r
            .set()
            .banks
            .iter()
            .enumerate()
            .flat_map(|(bank, b)| b.pads.iter().map(move |p| (bank, p)))
            .filter(|(_, p)| {
                !p.sample.is_empty()
                    && (km.failed_samples.iter().any(|(f, _)| *f == p.sample)
                        || !r.samples().contains_key(&p.sample))
            })
            .map(|(bank, p)| MissingPad {
                bank,
                x: p.x,
                y: p.y,
                sample: p.sample.clone(),
            })
            .collect();
        drop(km);
        drop(r);
        missing.sort_by_key(|m| (m.bank, std::cmp::Reverse(m.y), m.x));
        missing
    }

    fn install(
        &self,
        ticket: u64,
        set: Set,
        dir: Option<PathBuf>,
        samples: HashMap<String, Arc<Sample>>,
        failed: Vec<(String, String)>,
    ) -> Result<u64, String> {
        let mut r = lock(&self.router);
        if self.open_ticket.load(Ordering::SeqCst) != ticket {
            return Err("A newer open/new request replaced this one.".into());
        }
        let cache = samples.clone();
        if let Err(rejected) = r.replace_set(set, samples) {
            drop(r);
            return Err(rejected.reason);
        }
        // Taken under the router lock, so it orders installs exactly as they happened.
        let seq = self.installed.fetch_add(1, Ordering::SeqCst) + 1;
        lock(&self.history).clear();
        {
            let mut km = lock(&self.kit_manager);
            let old = std::mem::replace(&mut km.samples, cache);
            r.retire(old.into_values());
            km.failed_samples = failed;
            lock(&self.saver).reset(dir.is_some());
            km.dir = dir;
        }
        drop(r);
        *lock(&self.assigns) = Assigns::default();
        Ok(seq)
    }

    // ---- saving --------------------------------------------------------------------------

    /// Save now (the SAVE button). Shares the autosave path.
    pub fn save_set(&self) -> Result<(), String> {
        self.run_save(Instant::now())
    }

    /// Autosave: write when the debounce (or a failure retry) is due.
    pub fn autosave_tick(&self, now: Instant) {
        if lock(&self.saver).due(now) {
            let _ = self.run_save(now);
        }
    }

    /// Writes are serialised (they share temp and backup file names). The set, folder, set
    /// identity and revision are captured together under brief locks; the write itself runs
    /// with none held, and only that revision is acknowledged afterwards.
    fn run_save(&self, now: Instant) -> Result<(), String> {
        let _gate = lock(&self.save_gate);
        let (set, dir, epoch, rev) = {
            let r = lock(&self.router);
            let km = lock(&self.kit_manager);
            let saver = lock(&self.saver);
            (r.set().clone(), km.dir.clone(), saver.epoch(), saver.rev())
        };
        let result = match &dir {
            Some(dir) => (self.writer)(dir, &set)
                .map_err(|e| format!("Couldn't save to {}: {e:#}", dir.display())),
            None => Err("No set folder yet. Create or open a set first.".to_string()),
        };
        let new_failure = {
            let mut saver = lock(&self.saver);
            let status = saver.status();
            let new_failure = result.as_ref().err().filter(|reason| {
                status.state != SaveState::Failed || status.reason.as_ref() != Some(*reason)
            });
            let new_failure = new_failure.cloned();
            saver.finish(epoch, rev, result.clone(), now);
            new_failure
        };
        // Each new failure is logged once, whether the save was manual or automatic.
        if let Some(reason) = new_failure {
            logfile::log(format!("Save failed: {reason}"));
        }
        result
    }

    pub fn save_status(&self) -> SaveStatus {
        lock(&self.saver).status().clone()
    }

    /// The save status if it changed since last asked (for the `save-status` event).
    pub fn take_save_change(&self) -> Option<SaveStatus> {
        lock(&self.saver).take_change()
    }

    // ---- samples -------------------------------------------------------------------------

    /// Folder and set identity for a sample import.
    fn sample_target(&self) -> Result<(PathBuf, u64), String> {
        let km = lock(&self.kit_manager);
        let saver = lock(&self.saver);
        match km.dir.clone() {
            Some(dir) => Ok((dir, saver.epoch())),
            None => Err("No set folder yet. Create or open a set first.".into()),
        }
    }

    /// Put a prepared sample into both caches, if the set it was prepared for is still open.
    fn commit_sample(
        &self,
        r: &mut Router,
        epoch: u64,
        path: &str,
        sample: Arc<Sample>,
    ) -> Result<(), String> {
        let mut km = lock(&self.kit_manager);
        if lock(&self.saver).epoch() != epoch {
            return Err(STALE_SET.into());
        }
        r.install_sample(path.to_string(), sample.clone());
        if let Some(old) = km.samples.insert(path.to_string(), sample) {
            r.retire([old]);
        }
        km.failed_samples.retain(|(p, _)| p != path);
        Ok(())
    }

    /// Copy `src` into the set folder and load it, without assigning it to a pad. Slow.
    pub fn import_sample(&self, src: &str) -> Result<String, String> {
        let (dir, epoch) = self.sample_target()?;
        let (rel, sample) = prepare_sample(&dir, src).map_err(|e| format!("{e:#}"))?;
        let mut r = lock(&self.router);
        self.commit_sample(&mut r, epoch, &rel, sample)?;
        Ok(rel)
    }

    /// Start a sample assignment to one pad: remembers the set and request it belongs to.
    pub fn begin_assign(&self, bank: usize, x: u8, y: u8) -> Result<AssignTicket, String> {
        if bank >= kit::BANKS || x > 7 || y > 7 {
            return Err("That pad does not exist.".into());
        }
        let (dir, epoch) = self.sample_target()?;
        let mut assigns = lock(&self.assigns);
        assigns.next += 1;
        let seq = assigns.next;
        assigns.latest.insert((bank, x, y), seq);
        Ok(AssignTicket {
            dir,
            epoch,
            seq,
            bank,
            x,
            y,
        })
    }

    /// Install a decoded sample and set it on its pad as one undoable edit. Rejected when the set
    /// changed or a newer assignment to the same pad started. Returns the authoritative set.
    pub fn finish_assign(
        &self,
        ticket: &AssignTicket,
        rel: &str,
        sample: Arc<Sample>,
        default_color: Option<[u8; 3]>,
    ) -> Result<Set, String> {
        let mut r = lock(&self.router);
        if lock(&self.saver).epoch() != ticket.epoch {
            return Err(STALE_SET.into());
        }
        let newest = lock(&self.assigns)
            .latest
            .get(&(ticket.bank, ticket.x, ticket.y))
            .copied();
        if newest != Some(ticket.seq) {
            return Err("A newer sample was assigned to this pad meanwhile.".into());
        }
        self.commit_sample(&mut r, ticket.epoch, rel, sample)?;
        let (bank, x, y) = (ticket.bank, ticket.x, ticket.y);
        let rel = rel.to_string();
        self.edit_in(&mut r, None, Instant::now(), |set| {
            let b = &mut set.banks[bank];
            match b.pads.iter_mut().find(|p| p.x == x && p.y == y) {
                Some(pad) => pad.sample = rel,
                None => b.set_pad(Pad::new(x, y, rel, default_color.unwrap_or([63, 63, 63]))),
            }
        });
        let set = r.set().clone();
        drop(r);
        self.touch();
        Ok(set)
    }

    /// Assign a sample to a pad: a set-relative path, or any file (copied into the set first).
    /// A failure leaves the pad as it was. Slow: call from a worker thread.
    pub fn assign_sample(
        &self,
        bank: usize,
        x: u8,
        y: u8,
        input: &str,
        default_color: Option<[u8; 3]>,
    ) -> Result<Set, String> {
        let ticket = self.begin_assign(bank, x, y)?;
        let (rel, sample) = prepare_sample(&ticket.dir, input).map_err(|e| format!("{e:#}"))?;
        self.finish_assign(&ticket, &rel, sample, default_color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpd_core::kit::Pad;
    use rtrb::RingBuffer;

    fn app() -> (AppState, Consumer<Command>) {
        let (tx, cmd_rx) = RingBuffer::new(1024);
        let (_evt_tx, evt_rx) = RingBuffer::new(16);
        (AppState::new(Set::new(), tx, evt_rx), cmd_rx)
    }

    #[test]
    fn edit_then_undo_restores_pad() {
        let (state, _rx) = app();
        state.edit(None, |set| {
            set.banks[0].set_pad(Pad::new(1, 2, "samples/a.wav", [1, 2, 3]))
        });
        assert!(state.get_current_set().banks[0].pad(1, 2).is_some());
        assert!(state.history_state().can_undo);

        let undone = state.undo().expect("one step");
        assert!(undone.banks[0].pad(1, 2).is_none());
        assert!(state.get_current_set().banks[0].pad(1, 2).is_none());
        assert!(state.history_state().can_redo);

        let redone = state.redo().expect("redo step");
        assert!(redone.banks[0].pad(1, 2).is_some());
        assert!(state.get_current_set().banks[0].pad(1, 2).is_some());
    }

    #[test]
    fn update_pad_command_is_undoable() {
        let (state, _rx) = app();
        state.update_pad(3, Pad::new(0, 0, "samples/a.wav", [1, 2, 3]), None);
        assert!(state.get_current_set().banks[3].pad(0, 0).is_some());
        state.undo().unwrap();
        assert!(state.get_current_set().banks[3].pad(0, 0).is_none());
    }

    #[test]
    fn group_volume_change_is_not_undoable() {
        let (state, _rx) = app();
        let mut g = state.get_current_set().mixer.groups[0].clone();
        g.volume_db = -6.0;
        state.update_group(0, g);
        state.update_master_db(-3.0);
        assert_eq!(state.get_current_set().mixer.groups[0].volume_db, -6.0);
        assert_eq!(state.get_current_set().mixer.master_db, -3.0);
        assert!(!state.history_state().can_undo);
        assert_eq!(state.undo(), None);
    }

    #[test]
    fn group_rename_is_undoable_but_keeps_volume() {
        let (state, _rx) = app();
        let mut g = state.get_current_set().mixer.groups[1].clone();
        g.name = "Snares".into();
        state.update_group(1, g.clone());
        assert!(state.history_state().can_undo);
        g.volume_db = -9.0;
        state.update_group(1, g);
        let undone = state.undo().unwrap();
        assert_eq!(undone.mixer.groups[1].name, "Group 2");
        assert_eq!(undone.mixer.groups[1].volume_db, -9.0);
    }

    fn save_rev(state: &AppState) -> u64 {
        lock(&state.saver).rev()
    }

    #[test]
    fn update_group_out_of_range_changes_nothing() {
        let (state, _rx) = app();
        let g = state.get_current_set().mixer.groups[0].clone();
        let before = save_rev(&state);
        state.update_group(99, g);
        assert_eq!(save_rev(&state), before, "not marked dirty");
        assert!(!state.history_state().can_undo);
    }

    #[test]
    fn lcxl_move_marks_dirty_flags_the_screen_and_is_not_undoable() {
        let (state, _rx) = app();
        let before = save_rev(&state);
        state.on_lcxl(LcxlEvent::Fader { ch: 1, value: 127 });
        assert_eq!(state.get_current_set().mixer.groups[1].volume_db, 6.0);
        assert_eq!(save_rev(&state), before + 1, "one touch");
        assert!(
            !state.history_state().can_undo,
            "mixer values are not undoable"
        );
        assert!(state.take_mixer_change());
        assert!(!state.take_mixer_change(), "reported once");
        // A repeat of the same value changes nothing.
        state.on_lcxl(LcxlEvent::Fader { ch: 1, value: 127 });
        assert_eq!(save_rev(&state), before + 1);
        assert!(!state.take_mixer_change());
    }

    #[test]
    fn mute_solo_reflects_the_mixer() {
        let (state, _rx) = app();
        state.on_lcxl(LcxlEvent::Button {
            row: 0,
            ch: 4,
            on: true,
        });
        state.on_lcxl(LcxlEvent::Button {
            row: 1,
            ch: 6,
            on: true,
        });
        let (mute, solo) = state.mute_solo();
        assert!(mute[4] && !mute[6]);
        assert!(solo[6] && !solo[4]);
    }

    fn key_note(note: u8) -> MidiMsg {
        MidiMsg::NoteOn {
            ch: 0,
            note,
            vel: 100,
        }
    }

    #[test]
    fn learning_a_param_adds_an_undoable_binding_and_tells_the_ui() {
        let (state, _rx) = app();
        state.arm_learn(LearnTarget::Param("group.2.volume".into()));
        let events = state.on_midi(
            "K",
            "other",
            MidiMsg::Cc {
                ch: 0,
                cc: 21,
                value: 5,
            },
            false,
        );
        assert!(matches!(
            events.as_slice(),
            [UiEvent::LearnBound { binding }] if binding.number == 21
        ));
        assert_eq!(state.get_current_set().learn.len(), 1);
        assert!(state.history_state().can_undo);
        assert!(state.undo().unwrap().learn.is_empty());
        // Disarmed after the bind: the next CC does nothing.
        let after = state.on_midi(
            "K",
            "other",
            MidiMsg::Cc {
                ch: 0,
                cc: 22,
                value: 5,
            },
            false,
        );
        assert!(after.is_empty());
    }

    #[test]
    fn learning_a_pad_binds_a_note_and_unbind_removes_it() {
        let (state, _rx) = app();
        state.arm_learn(LearnTarget::Pad {
            bank: 0,
            x: 3,
            y: 4,
        });
        state.on_midi("K", "keys", key_note(48), false);
        assert_eq!(state.get_current_set().learn.len(), 1);
        let set = state.unbind(0);
        assert!(set.learn.is_empty());
        assert!(
            state.undo().unwrap().learn.len() == 1,
            "unbinding is undoable"
        );
    }

    #[test]
    fn launchpad_and_lcxl_input_while_armed_say_so_once() {
        let (state, _rx) = app();
        assert!(state.on_lp_event(LpEvent::PadDown { x: 0, y: 0 }).is_none());
        state.arm_learn(LearnTarget::Param("master".into()));
        let first = state.on_lp_event(LpEvent::PadDown { x: 0, y: 0 });
        assert_eq!(
            first,
            Some(UiEvent::LearnIgnored {
                reason: "Launchpad pads are fixed".into()
            })
        );
        assert!(state.on_lp_event(LpEvent::PadDown { x: 1, y: 0 }).is_none());
        assert!(state
            .on_lcxl(LcxlEvent::Fader { ch: 0, value: 100 })
            .is_none());
        assert!(state.get_current_set().learn.is_empty(), "nothing is bound");
        // The LCXL still moved the mixer.
        assert_ne!(state.get_current_set().mixer.groups[0].volume_db, 0.0);
        state.arm_learn(LearnTarget::Param("master".into()));
        assert_eq!(
            state.on_lcxl(LcxlEvent::Fader { ch: 0, value: 50 }),
            Some(UiEvent::LearnIgnored {
                reason: "LCXL controls are fixed in Part 1".into()
            })
        );
    }

    #[test]
    fn a_learned_mixer_cc_is_not_undoable_but_marks_the_mixer_and_set_dirty() {
        let (state, _rx) = app();
        state.arm_learn(LearnTarget::Param("group.3.volume".into()));
        state.on_midi(
            "K",
            "other",
            MidiMsg::Cc {
                ch: 0,
                cc: 7,
                value: 0,
            },
            false,
        );
        let _ = state.take_mixer_change();
        let before = save_rev(&state);
        state.on_midi(
            "K",
            "other",
            MidiMsg::Cc {
                ch: 0,
                cc: 7,
                value: 127,
            },
            false,
        );
        assert_eq!(state.get_current_set().mixer.groups[3].volume_db, 6.0);
        assert_eq!(save_rev(&state), before + 1);
        assert!(state.take_mixer_change());
        // Only the binding is in the history.
        assert!(state.undo().unwrap().learn.is_empty());
        assert!(!state.history_state().can_undo);
    }

    #[test]
    fn a_learned_bank_cc_sweep_is_one_undo_step() {
        let (state, _rx) = app();
        state.arm_learn(LearnTarget::Param("bank.1.humanize".into()));
        state.on_midi(
            "K",
            "other",
            MidiMsg::Cc {
                ch: 0,
                cc: 20,
                value: 0,
            },
            false,
        );
        for v in [10, 40, 80, 127] {
            state.on_midi(
                "K",
                "other",
                MidiMsg::Cc {
                    ch: 0,
                    cc: 20,
                    value: v,
                },
                false,
            );
        }
        assert_eq!(state.get_current_set().banks[1].humanize_db, 6.0);
        assert!(state.take_set_change());
        let undone = state.undo().unwrap();
        assert_eq!(
            undone.banks[1].humanize_db, 0.0,
            "one step undoes the sweep"
        );
        assert_eq!(undone.learn.len(), 1, "the binding is still there");
    }

    #[test]
    fn arming_rec_starts_the_clock_and_opens_one_undo_step() {
        let (state, _rx) = app();
        assert!(!state.history_state().can_undo);
        state.seq_rec(true);
        {
            let s = state.seq_handle();
            let s = s.lock().unwrap();
            assert!(s.playing && s.rec);
        }
        assert!(state.history_state().can_undo, "the take's step");
        state.seq_rec(true); // arming again opens no second step
        state.seq_rec(false);
        assert!(!state.seq_handle().lock().unwrap().rec);
        assert!(
            state.seq_handle().lock().unwrap().playing,
            "disarming leaves playback running"
        );
    }

    #[test]
    fn play_and_stop_toggle_the_clock() {
        let (state, _rx) = app();
        state.seq_play(true);
        assert!(state.seq_handle().lock().unwrap().playing);
        state.stop_all();
        assert!(!state.seq_handle().lock().unwrap().playing);
        state.seq_play(true);
        state.seq_play(false);
        assert!(!state.seq_handle().lock().unwrap().playing);
    }

    #[test]
    fn add_lane_needs_a_filled_pad_and_a_new_lane() {
        let (state, _rx) = app();
        assert_eq!(state.seq_add_lane(0, 3, 3).unwrap_err(), "Pad is empty");
        state.edit(None, |set| {
            set.banks[0].set_pad(lpd_core::kit::Pad::new(3, 3, "samples/a.wav", [63, 0, 0]));
        });
        let set = state.seq_add_lane(0, 3, 3).unwrap();
        assert_eq!(set.pattern.lanes.len(), 1);
        assert_eq!(
            state.seq_add_lane(0, 3, 3).unwrap_err(),
            "Lane already there"
        );
        assert_eq!(
            state.seq_add_lane(9, 0, 0).unwrap_err(),
            "Pad does not exist"
        );
    }

    #[test]
    fn seq_update_validates_clamps_and_coalesces_by_key() {
        let (state, _rx) = app();
        let mut p = state.get_current_set().pattern;
        p.length = 12;
        assert!(state.seq_update(p.clone(), None).is_err());
        p.length = 16;
        p.bpm = 500.0;
        p.swing = 10.0;
        let set = state.seq_update(p.clone(), None).unwrap();
        assert_eq!((set.pattern.bpm, set.pattern.swing), (200.0, 50.0));
        let mut bad = state.get_current_set().pattern;
        bad.lanes.push(lpd_core::seq::Lane {
            bank: 0,
            x: 0,
            y: 0,
            mute: false,
            steps: vec![0; 5],
        });
        assert!(state.seq_update(bad, None).is_err());
        let mut high = state.get_current_set().pattern;
        lpd_core::seq::add_lane(&mut high, 0, 0, 0).unwrap();
        high.lanes[0].steps[0] = 3;
        assert!(state.seq_update(high, None).is_err());
        // Two tempo drags within a second are one undo step.
        let before = state.get_current_set().pattern;
        for bpm in [110.0, 120.0, 130.0] {
            let mut q = before.clone();
            q.bpm = bpm;
            state.seq_update(q, Some("seq:bpm")).unwrap();
        }
        assert_eq!(state.get_current_set().pattern.bpm, 130.0);
        let undone = state.undo().unwrap();
        assert_eq!(undone.pattern.bpm, 200.0, "one step undoes the whole drag");
    }

    #[test]
    fn auditioning_a_step_is_silent_while_the_clock_runs() {
        let (state, mut rx) = app();
        state.seq_play(true);
        while rx.pop().is_ok() {}
        state.seq_audition_step(0, 0, 0);
        assert!(rx.pop().is_err());
    }

    fn with_audio_rate(state: &AppState, rate: u32) {
        state.set_audio_status(AudioStatus {
            state: "running".into(),
            sample_rate: rate,
            ..Default::default()
        });
    }

    #[test]
    fn a_missing_soundfont_keeps_the_instrument_and_reports_the_error() {
        let (state, mut rx) = app();
        with_audio_rate(&state, 48_000);
        while rx.pop().is_ok() {}
        let status = state
            .set_soundfont(Some("does-not-exist.sf2".into()))
            .unwrap();
        assert_eq!(status.engine, "builtin");
        assert_eq!(status.name, "BUILT-IN");
        let error = status.error.clone().expect("an error line");
        assert!(
            error.starts_with("Couldn't load does-not-exist.sf2: "),
            "{error}"
        );
        assert_eq!(state.keys_status(), status);
        assert!(rx.pop().is_err(), "no instrument was sent");
        assert_eq!(
            state.get_config().soundfont.as_deref(),
            Some("does-not-exist.sf2")
        );
    }

    #[test]
    fn clearing_the_soundfont_sends_a_fresh_builtin_instrument() {
        let (state, mut rx) = app();
        with_audio_rate(&state, 48_000);
        while rx.pop().is_ok() {}
        let status = state.set_soundfont(None).unwrap();
        assert_eq!(status, KeysStatus::builtin());
        let mut sent_instrument = false;
        while let Ok(c) = rx.pop() {
            sent_instrument |= matches!(c, Command::SetInstrument(_));
        }
        assert!(sent_instrument);
    }

    #[test]
    fn set_keys_is_one_undo_step_per_drag_and_reaches_the_engine() {
        let (state, mut rx) = app();
        while rx.pop().is_ok() {}
        for volume in [-10.0, -12.0, -14.0] {
            let set = state.set_keys(KeysSettings {
                sound: lpd_core::instrument::KeysSound::Organ,
                volume_db: volume,
                output: Some(2),
            });
            assert_eq!(set.keys.volume_db, volume);
        }
        let mut routes = 0;
        while let Ok(c) = rx.pop() {
            if let Command::KeysRoute { group: Some(2), .. } = c {
                routes += 1;
            }
        }
        assert_eq!(routes, 3);
        let undone = state.undo().unwrap();
        assert_eq!(
            undone.keys,
            KeysSettings::default(),
            "one step undoes the drag"
        );
    }

    #[test]
    fn screen_keys_reach_the_engine_and_tell_the_mirror() {
        let (state, mut rx) = app();
        while rx.pop().is_ok() {}
        let on = state.keys_note(60, 100, true);
        assert_eq!(
            on,
            UiEvent::KeysNote {
                note: 60,
                velocity: 100,
                on: true
            }
        );
        let off = state.keys_note(60, 100, false);
        assert_eq!(
            off,
            UiEvent::KeysNote {
                note: 60,
                velocity: 0,
                on: false
            }
        );
        state.keys_pitch(0.5);
        state.keys_mod(2.0);
        let sent: Vec<Command> = std::iter::from_fn(|| rx.pop().ok()).collect();
        assert!(matches!(
            sent[0],
            Command::KeysNoteOn {
                note: 60,
                velocity: 100
            }
        ));
        assert!(matches!(sent[1], Command::KeysNoteOff { note: 60 }));
        assert!(matches!(sent[2], Command::KeysPitch(p) if (p - 1.0).abs() < 1e-6));
        assert!(
            matches!(sent[3], Command::KeysMod(m) if m == 1.0),
            "clamped to 1"
        );
    }

    #[test]
    fn a_learned_keys_volume_edits_the_set_and_the_engine_follows() {
        let (state, mut rx) = app();
        state.arm_learn(LearnTarget::Param("keys.volume".into()));
        state.on_midi(
            "K",
            "other",
            MidiMsg::Cc {
                ch: 0,
                cc: 20,
                value: 0,
            },
            false,
        );
        while rx.pop().is_ok() {}
        state.on_midi(
            "K",
            "other",
            MidiMsg::Cc {
                ch: 0,
                cc: 20,
                value: 127,
            },
            false,
        );
        assert_eq!(state.get_current_set().keys.volume_db, 6.0);
        let mut routed = false;
        while let Ok(c) = rx.pop() {
            routed |= matches!(c, Command::KeysRoute { gain, .. } if gain > 1.9);
        }
        assert!(routed);
    }

    #[test]
    fn rename_with_value_change_is_one_touch_and_one_step() {
        let (state, _rx) = app();
        let mut g = state.get_current_set().mixer.groups[2].clone();
        g.name = "Toms".into();
        g.volume_db = -4.0;
        let before = save_rev(&state);
        state.update_group(2, g);
        assert_eq!(save_rev(&state), before + 1, "one touch");
        let set = state.get_current_set();
        assert_eq!(set.mixer.groups[2].name, "Toms");
        assert_eq!(set.mixer.groups[2].volume_db, -4.0);
        let undone = state.undo().unwrap();
        assert_eq!(undone.mixer.groups[2].name, "Group 3");
        assert_eq!(
            undone.mixer.groups[2].volume_db, -4.0,
            "values are not undone"
        );
        assert!(state.undo().is_none());
    }

    #[test]
    fn midi_role_is_saved_and_updates_the_listed_device() {
        let (state, _rx) = app();
        lock(&state.midi).set("Some Synth", "other", "connected");
        let list = state.set_midi_role("Some Synth", "keys").unwrap().unwrap();
        assert_eq!(list[0].role, "keys");
        assert_eq!(list[0].status, "connected");
        assert_eq!(
            state
                .get_config()
                .midi_roles
                .get("Some Synth")
                .map(String::as_str),
            Some("keys")
        );
        // Nothing changes the second time; an unplugged port is still remembered.
        assert_eq!(state.set_midi_role("Some Synth", "keys").unwrap(), None);
        assert_eq!(state.set_midi_role("Gone Synth", "keys").unwrap(), None);
        assert!(state.get_config().midi_roles.contains_key("Gone Synth"));
    }

    #[test]
    fn midi_role_rejects_fixed_devices_and_unknown_roles() {
        let (state, _rx) = app();
        assert!(state.set_midi_role("Launchpad MK2", "keys").is_err());
        assert!(state.set_midi_role("Launch Control XL", "other").is_err());
        assert!(state.set_midi_role("Some Synth", "launchpad").is_err());
        assert!(state.get_config().midi_roles.is_empty());
    }

    #[test]
    fn on_midi_hands_back_events_and_coalesces_wheels() {
        let (state, _rx) = app();
        let note = MidiMsg::NoteOn {
            ch: 0,
            note: 60,
            vel: 90,
        };
        assert_eq!(state.on_midi("K", "keys", note, true).len(), 2);
        let silent = state.on_midi("K", "keys", note, false);
        assert!(matches!(silent.as_slice(), [UiEvent::KeysNote { .. }]));

        // Every wheel message updates the router; only the newest value is kept for emitting.
        for value in [100, 200, 300] {
            let events = state.on_midi("K", "keys", MidiMsg::PitchBend { ch: 0, value }, false);
            assert!(events.is_empty());
        }
        assert_eq!(
            state.take_wheel(),
            Some(UiEvent::KeysWheel {
                pitch: 300.0 / 8192.0,
                r#mod: 0.0
            })
        );
        assert_eq!(state.take_wheel(), None);

        state.keys_input_closed();
        assert_eq!(
            state.take_wheel(),
            Some(UiEvent::KeysWheel {
                pitch: 0.0,
                r#mod: 0.0
            })
        );
    }

    #[test]
    fn undo_marks_the_set_dirty() {
        let (state, _rx) = app();
        state.rename_bank(0, "A".into());
        let before = save_rev(&state);
        state.undo().unwrap();
        assert_eq!(save_rev(&state), before + 1);
        state.redo().unwrap();
        assert_eq!(save_rev(&state), before + 2);
    }
}
