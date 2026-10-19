//! Application state coordinating router, lights, kit manager, saver and audio status.
//!
//! Lock order (take in this order, never the reverse; release before slow work):
//! `router` → `history` → `kit_manager` → `saver`. `painter`, `save_gate` and `assigns` are
//! separate: `painter` and `assigns` are leaves (nothing is taken while holding them) and
//! `save_gate` is taken first, only to order disk writes. No lock is held across disk I/O,
//! sample decoding or MIDI output. `config` → `startup` are leaves too (taken after nothing
//! else); the one exception to "no lock across disk I/O" is `config`, held while its own small
//! file is written.

use crate::audio::{AudioStats, AudioStatus};
use crate::config::{self, AppConfig};
use crate::lights::LightPainter;
use crate::logfile;
use crate::router::Router;
use crate::saver::{SaveState, SaveStatus, Saver};
use crate::startup::{self, Startup};
use crate::store::{prepare_sample, KitManager, LoadedKit};
use lpd_core::audio::{Command, Event, Sample, Taps};
use lpd_core::devices::launchpad::LpEvent;
use lpd_core::kit::{self, History, Pad, Set};
use rtrb::{Consumer, Producer};
use std::collections::HashMap;
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
    /// Engine output taps read by the telemetry thread (a leaf lock). Task 9 swaps them on a
    /// stream restart.
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
        let kit_manager = KitManager {
            dir: None,
            samples: Default::default(),
            failed_samples: Default::default(),
        };
        Self {
            router: Arc::new(Mutex::new(router)),
            painter: Arc::new(Mutex::new(LightPainter::new())),
            kit_manager: Arc::new(Mutex::new(kit_manager)),
            audio_status: Arc::new(Mutex::new(AudioStatus {
                state: "stopped".into(),
                device_name: "none".into(),
                sample_rate: 48_000,
                buffer_size: None,
                channels: 2,
                dropouts: 0,
                last_dropout_ago_ms: None,
            })),
            taps: Arc::default(),
            telemetry_on: Arc::new(AtomicBool::new(false)),
            audio_stats: Arc::new(Mutex::new(Arc::new(AudioStats::new()))),
            history: Arc::new(Mutex::new(History::new())),
            startup: Arc::new(Mutex::new(Startup::FirstRun)),
            config: Arc::default(),
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
    fn touch(&self) {
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

    /// Flash/ripple bookkeeping; the router lock is already released.
    fn record_hit(&self, x: u8, y: u8) {
        lock(&self.painter).on_pad_hit(x, y, Instant::now());
    }

    /// Hardware input. Ignored while a game owns the pads (decided under the router lock, so a
    /// mode switch cannot interleave with a half-handled event).
    pub fn on_lp_event(&self, ev: LpEvent) {
        {
            let mut r = lock(&self.router);
            if self.input_mode() != InputMode::Sampler {
                return;
            }
            r.on_lp_event(ev);
        }
        if let LpEvent::PadDown { x, y } = ev {
            self.record_hit(x, y);
        }
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
        let reclaimed = {
            let mut r = lock(&self.router);
            r.poll_audio_events();
            r.drain_reclaimable()
        };
        drop(reclaimed);
    }

    pub fn set_audio_status(&self, status: AudioStatus) {
        *lock(&self.audio_status) = status;
    }

    pub fn get_audio_status(&self) -> AudioStatus {
        let status = lock(&self.audio_status).clone();
        self.with_dropouts(status)
    }

    /// The current output rate, without cloning the whole status.
    pub fn sample_rate(&self) -> u32 {
        lock(&self.audio_status).sample_rate
    }

    /// Fill the dropout fields from the live counter (the stored status never carries them).
    fn with_dropouts(&self, mut status: AudioStatus) -> AudioStatus {
        let stats = lock(&self.audio_stats).clone();
        status.dropouts = stats.dropouts();
        status.last_dropout_ago_ms = stats.last_dropout_ago_ms(Instant::now());
        status
    }

    /// The output stream reported an error. Returns the new status the first time only, so a
    /// burst of errors reaches the UI once.
    pub fn audio_stream_failed(&self) -> Option<AudioStatus> {
        let failed = {
            let mut status = lock(&self.audio_status);
            if status.state == "running" {
                status.state = "error".into();
                Some(status.clone())
            } else {
                None
            }
        };
        failed.map(|s| self.with_dropouts(s))
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

    /// Remembers the `.sf2` file. The keys instrument stays built-in until Task 15.
    pub fn set_soundfont(&self, path: Option<String>) -> Result<KeysStatus, String> {
        self.update_config(|c| c.soundfont = path)?;
        Ok(KeysStatus {
            engine: "builtin".into(),
            name: "BUILT-IN".into(),
            error: None,
        })
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
