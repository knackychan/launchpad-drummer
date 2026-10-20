//! Load progress (`LoadStatus`), request ownership and the background restore, without Tauri.

mod common;

use common::{rig, within, write_wav};
use launchpad_drummer_lib::config::{self, AppConfig};
use launchpad_drummer_lib::startup::{
    LoadPhase, LoadStatus, LoadStep, LoadTracker, Startup, PROGRESS_INTERVAL,
};
use launchpad_drummer_lib::state::AppState;
use launchpad_drummer_lib::store::KitManager;
use lpd_core::kit::{self, Pad, Set};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---- the wire shape and the tracker --------------------------------------------------------

#[test]
fn the_status_serializes_in_camel_case_with_a_camel_case_phase() {
    let status = LoadStatus {
        request_id: 3,
        revision: 9,
        phase: LoadPhase::LoadingSamples,
        completed: 12,
        total: Some(32),
        failed: 1,
        error: None,
    };
    assert_eq!(
        serde_json::to_value(&status).unwrap(),
        serde_json::json!({
            "requestId": 3, "revision": 9, "phase": "loadingSamples",
            "completed": 12, "total": 32, "failed": 1, "error": null
        })
    );
    let phases: Vec<_> = [
        LoadPhase::Settings,
        LoadPhase::ReadingSet,
        LoadPhase::PreparingSamples,
        LoadPhase::Installing,
        LoadPhase::NeedsSet,
    ]
    .iter()
    .map(|p| serde_json::to_value(p).unwrap())
    .collect();
    assert_eq!(
        phases,
        ["settings", "readingSet", "preparingSamples", "installing", "needsSet"]
    );
}

#[test]
fn the_initial_status_is_request_zero_revision_zero_settings() {
    let s = LoadStatus::initial();
    assert_eq!((s.request_id, s.revision, s.phase), (0, 0, LoadPhase::Settings));
    assert_eq!(s.total, None);
}

#[test]
fn revisions_only_rise_and_a_new_request_keeps_counting() {
    let mut t = LoadTracker::default();
    let now = Instant::now();
    let a = t.begin(1, LoadPhase::Settings);
    let b = t.update(1, LoadStep::phase(LoadPhase::ReadingSet), now).unwrap().unwrap();
    let c = t.begin(2, LoadPhase::ReadingSet);
    assert!(a.revision < b.revision && b.revision < c.revision, "{a:?} {b:?} {c:?}");
    assert_eq!(c.completed, 0);
}

#[test]
fn a_superseded_request_cannot_publish_progress_error_or_ready() {
    let mut t = LoadTracker::default();
    let now = Instant::now();
    t.begin(1, LoadPhase::ReadingSet);
    t.begin(2, LoadPhase::ReadingSet);
    for step in [
        LoadStep::counted(LoadPhase::LoadingSamples, 1, 2, 0),
        LoadStep::failed("old failure"),
        LoadStep::ready(0),
    ] {
        assert!(t.update(1, step, now).is_none());
    }
    assert_eq!(t.status().request_id, 2);
    assert_eq!(t.status().phase, LoadPhase::ReadingSet);
    assert_eq!(t.status().error, None);
}

#[test]
fn progress_within_a_phase_is_throttled_but_always_stored() {
    let mut t = LoadTracker::default();
    let t0 = Instant::now();
    t.begin(1, LoadPhase::ReadingSet);
    let counted = |n| LoadStep::counted(LoadPhase::LoadingSamples, n, 100, 0);
    // A phase change is sent at once.
    assert!(t.update(1, counted(0), t0).unwrap().is_some());
    // Inside the interval: stored, not sent.
    assert!(t.update(1, counted(1), t0 + Duration::from_millis(10)).unwrap().is_none());
    assert_eq!(t.status().completed, 1);
    // Past the interval: sent, with the latest numbers.
    let sent = t.update(1, counted(5), t0 + PROGRESS_INTERVAL).unwrap().unwrap();
    assert_eq!(sent.completed, 5);
    // The last file and the terminal phase bypass the throttle.
    assert!(t.update(1, counted(100), t0 + PROGRESS_INTERVAL + Duration::from_millis(1)).unwrap().is_some());
    assert!(t.update(1, LoadStep::ready(0), t0 + PROGRESS_INTERVAL + Duration::from_millis(2)).unwrap().is_some());
}

// ---- helpers --------------------------------------------------------------------------------

fn save_set(dir: &Path, bank0_name: &str, samples: &[&str]) {
    let mut set = Set::new();
    set.banks[0].name = bank0_name.into();
    for (i, s) in samples.iter().enumerate() {
        set.banks[0].set_pad(Pad::new(i as u8, 0, *s, [0, 0, 0]));
        if !s.contains("missing") {
            write_wav(&dir.join(s), 480);
        }
    }
    kit::save(dir, &set).unwrap();
}

fn config_with_last_set(tmp: &Path, last: &Path) -> PathBuf {
    let path = tmp.join("cfg").join("config.json");
    let c = AppConfig {
        last_set: Some(last.display().to_string()),
        ..Default::default()
    };
    config::save(&path, &c).unwrap();
    path
}

type Seen = Arc<Mutex<Vec<LoadStatus>>>;

/// Record every status the notifier is given.
fn record(state: &AppState) -> Seen {
    let seen: Seen = Arc::default();
    let sink = seen.clone();
    state.set_load_notifier(Arc::new(move |s| sink.lock().unwrap().push(s.clone())));
    seen
}

fn phases(seen: &Seen) -> Vec<LoadPhase> {
    let mut out: Vec<LoadPhase> = Vec::new();
    for s in seen.lock().unwrap().iter() {
        if out.last() != Some(&s.phase) {
            out.push(s.phase);
        }
    }
    out
}

// ---- phase transitions ----------------------------------------------------------------------

#[test]
fn restoring_a_saved_set_goes_settings_reading_loading_installing_ready() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Live");
    save_set(&dir, "Live", &["a.wav", "b.wav", "missing.wav"]);
    let r = rig(1024);
    let seen = record(&r.state);
    r.state.init_config(config_with_last_set(tmp.path(), &dir));
    let ticket = r.state.begin_restore();
    let outcome = r.state.run_restore(ticket);
    assert!(matches!(outcome, Startup::Loaded { .. }));
    assert_eq!(
        phases(&seen),
        [
            LoadPhase::Settings,
            LoadPhase::ReadingSet,
            LoadPhase::LoadingSamples,
            LoadPhase::Installing,
            LoadPhase::Ready
        ]
    );
    let done = r.state.get_load_status();
    assert_eq!((done.request_id, done.phase, done.failed), (ticket, LoadPhase::Ready, 1));
    // The last count seen while loading is the full count, failures included.
    let last_count = seen
        .lock()
        .unwrap()
        .iter()
        .filter(|s| s.phase == LoadPhase::LoadingSamples)
        .next_back()
        .cloned()
        .unwrap();
    assert_eq!((last_count.completed, last_count.total, last_count.failed), (3, Some(3), 1));
}

#[test]
fn no_saved_set_needs_a_set_and_missing_or_unreadable_ones_too() {
    let tmp = tempfile::tempdir().unwrap();
    // First run: no last set in the config.
    let r = rig(1024);
    let seen = record(&r.state);
    assert_eq!(r.state.restore(tmp.path().join("none").join("config.json")), Startup::FirstRun);
    assert_eq!(phases(&seen), [LoadPhase::Settings, LoadPhase::NeedsSet]);
    assert!(r.state.save_set().is_err(), "saving stays off without a set");

    // Missing folder.
    let gone = tmp.path().join("gone");
    let r = rig(1024);
    let seen = record(&r.state);
    let outcome = r.state.restore(config_with_last_set(tmp.path(), &gone));
    assert!(matches!(outcome, Startup::NotFound { .. }));
    assert_eq!(r.state.get_startup(), outcome);
    assert_eq!(phases(&seen).last(), Some(&LoadPhase::NeedsSet));

    // Both files broken.
    let broken = tmp.path().join("broken");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join(kit::SET_FILE), "{ nope").unwrap();
    let r = rig(1024);
    let seen = record(&r.state);
    let outcome = r.state.restore(config_with_last_set(tmp.path(), &broken));
    assert!(matches!(outcome, Startup::Unreadable { .. }));
    assert_eq!(
        phases(&seen),
        [LoadPhase::Settings, LoadPhase::ReadingSet, LoadPhase::NeedsSet]
    );
}

#[test]
fn opening_creating_and_choosing_default_report_their_own_phases() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(4096);
    let seen = record(&r.state);

    let one = tmp.path().join("one");
    save_set(&one, "one", &["a.wav"]);
    r.state.open_set(&one.to_string_lossy()).unwrap();
    assert_eq!(
        phases(&seen),
        [LoadPhase::ReadingSet, LoadPhase::LoadingSamples, LoadPhase::Installing, LoadPhase::Ready]
    );

    // An empty set has no sample work to show.
    seen.lock().unwrap().clear();
    r.state.new_set(&tmp.path().join("empty").to_string_lossy()).unwrap();
    assert_eq!(
        phases(&seen),
        [LoadPhase::ReadingSet, LoadPhase::Installing, LoadPhase::Ready]
    );
    assert!(seen.lock().unwrap().iter().all(|s| s.total.is_none()), "no 0-of-0 count");

    // Default on first run: preparation first, then the ordinary loading.
    seen.lock().unwrap().clear();
    let default_dir = tmp.path().join("Default");
    assert!(!default_dir.exists(), "nothing is created before the explicit choice");
    r.state.open_default_set(&default_dir).unwrap();
    assert_eq!(
        phases(&seen),
        [
            LoadPhase::ReadingSet,
            LoadPhase::PreparingSamples,
            LoadPhase::LoadingSamples,
            LoadPhase::Installing,
            LoadPhase::Ready
        ]
    );
    let prep = seen
        .lock()
        .unwrap()
        .iter()
        .filter(|s| s.phase == LoadPhase::PreparingSamples)
        .next_back()
        .cloned()
        .unwrap();
    assert!(prep.total.unwrap() >= 30 && prep.completed == prep.total.unwrap());

    // Reopening an existing default set is an ordinary open: no preparation, edits kept.
    let mut edited = kit::load(&default_dir).unwrap().set;
    edited.banks[0].name = "MINE".into();
    kit::save(&default_dir, &edited).unwrap();
    seen.lock().unwrap().clear();
    let again = r.state.open_default_set(&default_dir).unwrap();
    assert_eq!(again.set.banks[0].name, "MINE");
    assert!(!phases(&seen).contains(&LoadPhase::PreparingSamples));
}

#[test]
fn a_fatal_error_fails_the_request_with_a_reason_and_keeps_the_current_set() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(1024);
    let one = tmp.path().join("one");
    save_set(&one, "one", &[]);
    r.state.open_set(&one.to_string_lossy()).unwrap();
    let seen = record(&r.state);

    let error = r.state.open_set(&tmp.path().join("nope").to_string_lossy()).unwrap_err();
    let status = r.state.get_load_status();
    assert_eq!(status.phase, LoadPhase::Failed);
    assert_eq!(status.error.as_deref(), Some(error.as_str()));
    assert_eq!(phases(&seen), [LoadPhase::ReadingSet, LoadPhase::Failed]);
    assert_eq!(r.state.get_current_set().banks[0].name, "one");
}

#[test]
fn some_samples_failing_still_ends_ready_with_the_failures_counted() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(1024);
    let dir = tmp.path().join("kit");
    save_set(&dir, "kit", &["ok.wav", "missing.wav"]);
    r.state.open_set(&dir.to_string_lossy()).unwrap();
    let status = r.state.get_load_status();
    assert_eq!((status.phase, status.failed), (LoadPhase::Ready, 1));
    assert_eq!(r.state.missing_samples().len(), 1);
}

// ---- ownership ------------------------------------------------------------------------------

/// Run a restore of `last` on a thread; its loader stops after the first progress report, tells
/// the test, and waits to be released. `fail` makes the loader error once released.
struct PausedRestore {
    resume: mpsc::Sender<()>,
    handle: std::thread::JoinHandle<Startup>,
}

fn paused_restore(state: &AppState, ticket: u64, fail: bool) -> PausedRestore {
    let (started_tx, started_rx) = mpsc::channel();
    let (resume, resume_rx) = mpsc::channel::<()>();
    let state = state.clone();
    let handle = std::thread::spawn(move || {
        state.run_restore_with(ticket, move |dir, progress| {
            progress(0, 1, 0);
            started_tx.send(()).unwrap();
            resume_rx.recv().unwrap();
            if fail {
                anyhow::bail!("injected failure");
            }
            KitManager::load_from_dir_with_progress(dir, progress)
        })
    });
    started_rx.recv_timeout(Duration::from_secs(10)).expect("restore paused");
    PausedRestore { resume, handle }
}

fn finish(p: PausedRestore) -> Startup {
    p.resume.send(()).unwrap();
    p.handle.join().unwrap()
}

#[test]
fn a_restore_that_finishes_after_a_newer_open_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = (tmp.path().join("A"), tmp.path().join("B"));
    save_set(&a, "A", &["a.wav"]);
    save_set(&b, "B", &["b.wav"]);
    let r = rig(4096);
    r.state.init_config(config_with_last_set(tmp.path(), &a));
    let ticket = r.state.begin_restore();
    let paused = paused_restore(&r.state, ticket, false);

    // The user opens B while A is still loading.
    r.state.open_set(&b.to_string_lossy()).unwrap();
    let outcome = finish(paused);

    assert!(matches!(outcome, Startup::Loaded { ref name, .. } if name == "B"), "{outcome:?}");
    assert_eq!(r.state.get_current_set().banks[0].name, "B");
    assert!(matches!(r.state.get_startup(), Startup::Loaded { ref name, .. } if name == "B"));
    assert_eq!(
        r.state.get_config().last_set.as_deref(),
        Some(b.display().to_string().as_str())
    );
    let status = r.state.get_load_status();
    assert_eq!(status.phase, LoadPhase::Ready);
    assert_ne!(status.request_id, ticket);
}

#[test]
fn a_restore_that_fails_after_a_newer_open_cannot_fail_the_ui() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = (tmp.path().join("A"), tmp.path().join("B"));
    save_set(&a, "A", &["a.wav"]);
    save_set(&b, "B", &["b.wav"]);
    let r = rig(4096);
    r.state.init_config(config_with_last_set(tmp.path(), &a));
    let ticket = r.state.begin_restore();
    let paused = paused_restore(&r.state, ticket, true);

    r.state.open_set(&b.to_string_lossy()).unwrap();
    let seen = record(&r.state);
    let outcome = finish(paused);

    assert!(matches!(outcome, Startup::Loaded { ref name, .. } if name == "B"), "{outcome:?}");
    assert!(matches!(r.state.get_startup(), Startup::Loaded { ref name, .. } if name == "B"));
    let status = r.state.get_load_status();
    assert_eq!((status.phase, status.error), (LoadPhase::Ready, None));
    assert!(seen.lock().unwrap().is_empty(), "the stale failure published nothing");
}

#[test]
fn a_failing_restore_with_no_newer_request_ends_needing_a_set() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("A");
    save_set(&a, "A", &["a.wav"]);
    let r = rig(1024);
    r.state.init_config(config_with_last_set(tmp.path(), &a));
    let ticket = r.state.begin_restore();
    let outcome = finish(paused_restore(&r.state, ticket, true));
    assert!(matches!(outcome, Startup::Unreadable { .. }));
    assert_eq!(r.state.get_startup(), outcome);
    assert_eq!(r.state.get_load_status().phase, LoadPhase::NeedsSet);
    assert!(r.state.save_set().is_err(), "saving stays off without a set");
}

#[test]
fn progress_callbacks_and_the_notifier_can_read_state_without_deadlock() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("A");
    save_set(&a, "A", &["a.wav", "b.wav"]);
    let r = rig(1024);
    r.state.init_config(config_with_last_set(tmp.path(), &a));
    let inner = r.state.clone();
    r.state.set_load_notifier(Arc::new(move |s| {
        // Everything a UI handler might ask for while a notification is delivered.
        let _ = (inner.get_load_status(), inner.get_config(), inner.get_current_set());
        let _ = (inner.get_startup(), s.revision);
    }));
    let state = r.state.clone();
    let ticket = state.begin_restore();
    let probe = state.clone();
    let outcome = within(20, move || {
        state.run_restore_with(ticket, move |dir, progress| {
            let read = |done, total, failed| {
                let _ = (probe.get_config(), probe.get_current_set(), probe.missing_samples());
                progress(done, total, failed);
            };
            KitManager::load_from_dir_with_progress(dir, read)
        })
    });
    assert!(matches!(outcome, Startup::Loaded { .. }));
}

#[test]
fn audio_can_start_while_the_loader_is_paused_and_the_set_lands_in_its_queues() {
    use lpd_core::audio::{Command, Event};
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("A");
    save_set(&a, "A", &["a.wav"]);
    let r = rig(1024);
    r.state.init_config(config_with_last_set(tmp.path(), &a));
    let ticket = r.state.begin_restore();
    let paused = paused_restore(&r.state, ticket, false);

    // The stream comes up while the set is still loading: new queues replace the placeholders.
    let (tx, mut commands) = rtrb::RingBuffer::<Command>::new(1024);
    let (_events_tx, rx) = rtrb::RingBuffer::<Event>::new(256);
    r.state.router.lock().unwrap().replace_audio(tx, rx);

    let outcome = finish(paused);
    assert!(matches!(outcome, Startup::Loaded { .. }));
    let sent = common::drain(&mut commands);
    assert!(
        sent.iter().any(|c| matches!(c, Command::StopAll)),
        "the installed set reached the new engine's queue: {} commands",
        sent.len()
    );
    assert_eq!(r.state.get_current_set().banks[0].name, "A");
}

#[test]
fn a_backup_restore_still_schedules_the_recovery_save() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("kit");
    kit::save(&dir, &Set::new()).unwrap();
    kit::save(&dir, &Set::new()).unwrap(); // rotates a good copy into the backup
    std::fs::write(dir.join(kit::SET_FILE), "{ broken").unwrap();
    let r = rig(1024);
    let outcome = r.state.restore(config_with_last_set(tmp.path(), &dir));
    assert!(matches!(outcome, Startup::Loaded { from_backup: true, .. }));
    // Autosave writes set.json back once its debounce is over.
    r.state.autosave_tick(Instant::now() + Duration::from_secs(60));
    assert!(dir.join(kit::SET_FILE).exists());
    assert!(kit::load(&dir).is_ok());
}

// ---- retry ----------------------------------------------------------------------------------

#[test]
fn retry_is_refused_while_loading_and_reopens_the_configured_set_afterwards() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("A");
    let r = rig(1024);
    r.state.init_config(config_with_last_set(tmp.path(), &a)); // not on disk yet
    let ticket = r.state.begin_restore();

    // Still in the restore: a second one is refused with a clear message.
    let busy = r.state.retry_startup().unwrap_err();
    assert!(busy.contains("Already loading"), "{busy}");
    assert_eq!(r.state.get_load_status().request_id, ticket, "the refused retry changed nothing");

    assert!(matches!(r.state.run_restore(ticket), Startup::NotFound { .. }));
    assert_eq!(r.state.get_load_status().phase, LoadPhase::NeedsSet);

    // The folder appears; retrying uses the config already loaded.
    save_set(&a, "A", &["a.wav"]);
    let outcome = r.state.retry_startup().unwrap();
    assert!(matches!(outcome, Startup::Loaded { ref name, .. } if name == "A"));
    let status = r.state.get_load_status();
    assert_eq!(status.phase, LoadPhase::Ready);
    assert!(status.request_id > ticket);
    assert_eq!(r.state.get_current_set().banks[0].name, "A");
}

// ---- waveform envelope cache ----------------------------------------------------------------

#[test]
fn the_waveform_envelope_is_cached_by_sample_identity_and_bucket_count() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("kit");
    save_set(&dir, "kit", &["a.wav"]);
    let r = rig(1024);
    r.state.open_set(&dir.to_string_lossy()).unwrap();

    let first = r.state.sample_peaks("a.wav", 64).unwrap();
    let again = r.state.sample_peaks("a.wav", 64).unwrap();
    assert_eq!(first.peaks, again.peaks);
    assert_eq!(r.state.peak_scans(), 1, "the second request came from the cache");

    r.state.sample_peaks("a.wav", 128).unwrap();
    assert_eq!(r.state.peak_scans(), 2, "another bucket count is another envelope");

    // The same path with different audio is a different loaded sample: it must be scanned again.
    common::write_wav(&dir.join("a.wav"), 4800);
    r.state.open_set(&dir.to_string_lossy()).unwrap();
    let replaced = r.state.sample_peaks("a.wav", 128).unwrap();
    assert_eq!(r.state.peak_scans(), 3);
    assert!(replaced.length_ms > 90.0, "the new, longer recording: {}", replaced.length_ms);

    assert!(r.state.sample_peaks("missing.wav", 64).is_none());
}

#[test]
fn the_cache_does_not_keep_a_dropped_sample_alive() {
    let tmp = tempfile::tempdir().unwrap();
    let (one, two) = (tmp.path().join("one"), tmp.path().join("two"));
    save_set(&one, "one", &["a.wav"]);
    save_set(&two, "two", &[]);
    let r = rig(1024);
    r.state.open_set(&one.to_string_lossy()).unwrap();
    r.state.sample_peaks("a.wav", 64).unwrap();
    assert!(r.state.peaks_cache_holds_audio());

    r.state.open_set(&two.to_string_lossy()).unwrap(); // the old samples are retired…
    let freed = r.state.router.lock().unwrap().drain_reclaimable(); // …and reclaimed
    drop(freed);
    assert!(!r.state.peaks_cache_holds_audio(), "only a Weak reference remains");
}

#[test]
fn a_long_recording_is_scanned_off_the_calling_thread_and_the_state_stays_usable() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("kit");
    save_set(&dir, "kit", &[]);
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "long.wav", [0, 0, 0]));
    kit::save(&dir, &set).unwrap();
    common::write_wav(&dir.join("long.wav"), 48_000 * 60 * 5); // five minutes
    let r = rig(1024);
    r.state.open_set(&dir.to_string_lossy()).unwrap();
    let scanning = r.state.clone();
    let worker = std::thread::spawn(move || scanning.sample_peaks("long.wav", 512));
    // While it runs, ordinary state reads are not blocked behind the scan.
    within(5, {
        let s = r.state.clone();
        move || (s.get_current_set(), s.get_load_status())
    });
    assert!(worker.join().unwrap().is_some());
}
