//! Findings 5 and 8: saving and loading never block input, are ordered, and report failures.

mod common;

use common::{rig, within, write_wav};
use launchpad_drummer_lib::saver::SaveState;
use launchpad_drummer_lib::state::AppState;
use lpd_core::kit::Set;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Log = Arc<Mutex<Vec<PathBuf>>>;

/// A state whose disk writes are recorded and can be made to fail or wait.
fn state_with_writer(
    behave: impl Fn(&Path, &Set) -> anyhow::Result<()> + Send + Sync + 'static,
) -> (AppState, Log, tempfile::TempDir) {
    let log: Log = Arc::default();
    let seen = log.clone();
    let (tx, _rx) = rtrb::RingBuffer::new(4096);
    let (_etx, erx) = rtrb::RingBuffer::new(16);
    let state = AppState::with_writer(
        Set::new(),
        tx,
        erx,
        Arc::new(move |dir, set| {
            seen.lock().unwrap().push(dir.to_path_buf());
            behave(dir, set)
        }),
    );
    let tmp = tempfile::tempdir().unwrap();
    state
        .new_set_dir(tmp.path().join("kit").to_str().unwrap())
        .unwrap();
    // `new_set_dir` writes through lpd_core directly, not through the injected writer.
    (state, log, tmp)
}

fn status(state: &AppState) -> SaveState {
    state.save_status().state
}

#[test]
fn input_and_painting_continue_while_a_save_is_blocked() {
    let (release, gate) = mpsc::channel::<()>();
    let gate = Mutex::new(gate);
    let (entered_tx, entered) = mpsc::channel::<()>();
    let entered_tx = Mutex::new(entered_tx);
    let (state, _log, _tmp) = state_with_writer(move |_, _| {
        entered_tx.lock().unwrap().send(()).unwrap();
        gate.lock().unwrap().recv().unwrap();
        Ok(())
    });
    state.rename_bank(0, "Edited".into());

    let saving = {
        let s = state.clone();
        std::thread::spawn(move || s.save_set())
    };
    entered
        .recv_timeout(Duration::from_secs(5))
        .expect("save started");

    let s = state.clone();
    within(5, move || {
        s.trigger_pad(0, 0);
        s.release_pad(0, 0);
        s.on_lp_event(lpd_core::devices::launchpad::LpEvent::SideDown(2));
        let _ = s.paint_frame(Instant::now());
        s.rename_bank(1, "Edited during save".into());
        s.get_current_set()
    });

    release.send(()).unwrap();
    within(5, move || saving.join().unwrap().unwrap());
    assert_eq!(
        status(&state),
        SaveState::Pending,
        "the edit made during the save is still unsaved"
    );
}

#[test]
fn only_the_saved_revision_is_acknowledged() {
    let (state, log, _tmp) = state_with_writer(|_, _| Ok(()));
    let t0 = Instant::now();
    state.rename_bank(0, "A".into());
    state.autosave_tick(t0 + Duration::from_millis(500));
    assert!(log.lock().unwrap().is_empty(), "debounced: 1 s not elapsed");
    state.autosave_tick(t0 + Duration::from_secs(2));
    assert_eq!(log.lock().unwrap().len(), 1);
    assert_eq!(status(&state), SaveState::Saved);
    // Nothing more to write.
    state.autosave_tick(t0 + Duration::from_secs(60));
    assert_eq!(log.lock().unwrap().len(), 1);
}

#[test]
fn a_write_failure_stays_visible_and_dirty_until_a_retry_succeeds() {
    let fail = Arc::new(Mutex::new(true));
    let switch = fail.clone();
    let (state, log, _tmp) = state_with_writer(move |_, _| {
        if *switch.lock().unwrap() {
            anyhow::bail!("disk is full")
        }
        Ok(())
    });
    let t0 = Instant::now();
    state.rename_bank(0, "A".into());

    let error = state.save_set().unwrap_err();
    assert!(error.contains("disk is full"), "{error}");
    let st = state.save_status();
    assert_eq!(st.state, SaveState::Failed);
    assert!(st.reason.unwrap().contains("disk is full"));

    // Autosave retries by itself, still failing: the notice stays.
    state.autosave_tick(t0 + Duration::from_secs(10));
    assert_eq!(status(&state), SaveState::Failed);
    let attempts = log.lock().unwrap().len();
    assert!(attempts >= 2);

    // The disk recovers: the next retry clears it.
    *fail.lock().unwrap() = false;
    state.rename_bank(0, "B".into()); // a new edit keeps the failed notice until written
    assert_eq!(status(&state), SaveState::Failed);
    state.autosave_tick(t0 + Duration::from_secs(20));
    assert_eq!(status(&state), SaveState::Saved);
    assert_eq!(state.save_status().reason, None);
}

#[test]
fn saving_before_a_set_folder_exists_reports_it() {
    let r = rig(256);
    let error = r.state.save_set().unwrap_err();
    assert!(error.contains("No set folder"), "{error}");
    assert_eq!(r.state.save_status().state, SaveState::Failed);
    r.state.rename_bank(0, "x".into());
    r.state
        .autosave_tick(Instant::now() + Duration::from_secs(60));
    assert_eq!(
        r.state.save_status().state,
        SaveState::Failed,
        "no folder: nothing is silently dropped or retried forever"
    );
}

#[test]
fn a_set_switch_during_a_save_writes_the_right_folder_and_stays_clean() {
    let (release, gate) = mpsc::channel::<()>();
    let gate = Mutex::new(gate);
    let (entered_tx, entered) = mpsc::channel::<()>();
    let entered_tx = Mutex::new(entered_tx);
    let (state, log, tmp) = state_with_writer(move |_, _| {
        entered_tx.lock().unwrap().send(()).unwrap();
        gate.lock().unwrap().recv().unwrap();
        Ok(())
    });
    state.rename_bank(0, "A".into());
    let saving = {
        let s = state.clone();
        std::thread::spawn(move || s.save_set())
    };
    entered.recv_timeout(Duration::from_secs(5)).unwrap();

    let second = tmp.path().join("second");
    state.new_set_dir(second.to_str().unwrap()).unwrap();
    release.send(()).unwrap();
    within(5, move || saving.join().unwrap().unwrap());

    assert_eq!(log.lock().unwrap().as_slice(), [tmp.path().join("kit")]);
    assert_eq!(
        status(&state),
        SaveState::Idle,
        "the old set's save does not mark the new set saved"
    );
    assert_eq!(state.get_current_set().banks[0].name, "Bank 1");
}

#[test]
fn writes_are_serialised() {
    let active = Arc::new(Mutex::new(0usize));
    let overlap = Arc::new(Mutex::new(false));
    let (a, o) = (active.clone(), overlap.clone());
    let (state, log, _tmp) = state_with_writer(move |_, _| {
        {
            let mut n = a.lock().unwrap();
            *n += 1;
            if *n > 1 {
                *o.lock().unwrap() = true;
            }
        }
        std::thread::sleep(Duration::from_millis(30));
        *a.lock().unwrap() -= 1;
        Ok(())
    });
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let s = state.clone();
            std::thread::spawn(move || s.save_set().unwrap())
        })
        .collect();
    within(10, move || {
        for w in workers {
            w.join().unwrap();
        }
    });
    assert_eq!(log.lock().unwrap().len(), 4);
    assert!(!*overlap.lock().unwrap());
}

// ---- opening sets ----------------------------------------------------------------------

#[test]
fn only_the_newest_open_request_commits() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(4096);
    for name in ["one", "two"] {
        let dir = tmp.path().join(name);
        let mut set = Set::new();
        set.banks[0].name = name.to_string();
        lpd_core::kit::save(&dir, &set).unwrap();
    }
    let load = |name: &str| {
        launchpad_drummer_lib::store::KitManager::load_from_dir(&tmp.path().join(name)).unwrap()
    };

    let first = r.state.begin_open();
    let second = r.state.begin_open();
    // The newer request finishes first; the older one arrives late and is discarded.
    r.state.finish_open(second, load("two")).unwrap();
    let error = r.state.finish_open(first, load("one")).unwrap_err();
    assert!(error.contains("newer"), "{error}");
    assert_eq!(r.state.get_current_set().banks[0].name, "two");

    // The folder it saves into is the winner's.
    r.state.rename_bank(0, "two!".into());
    r.state.save_set().unwrap();
    let on_disk = lpd_core::kit::load(&tmp.path().join("two")).unwrap();
    assert_eq!(on_disk.set.banks[0].name, "two!");
    let untouched = lpd_core::kit::load(&tmp.path().join("one")).unwrap();
    assert_eq!(untouched.set.banks[0].name, "one");
}

#[test]
fn loading_keeps_the_active_set_when_the_folder_is_unreadable() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(4096);
    r.state.rename_bank(0, "Mine".into());
    let error = r
        .state
        .load_set_dir(tmp.path().to_str().unwrap())
        .unwrap_err();
    assert!(!error.is_empty());
    assert_eq!(r.state.get_current_set().banks[0].name, "Mine");
}

#[test]
fn opening_a_kit_loads_its_samples_and_reports_missing_ones() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(4096);
    let dir = tmp.path().join("kit");
    let mut set = Set::new();
    set.banks[0].set_pad(lpd_core::kit::Pad::new(0, 0, "samples/ok.wav", [1, 1, 1]));
    set.banks[0].set_pad(lpd_core::kit::Pad::new(1, 0, "samples/gone.wav", [1, 1, 1]));
    lpd_core::kit::save(&dir, &set).unwrap();
    write_wav(&dir.join("samples/ok.wav"), 480);

    r.state.load_set_dir(dir.to_str().unwrap()).unwrap();
    assert!(r.state.sample_peaks("samples/ok.wav", 16).is_some());
    let km = r.state.kit_manager.lock().unwrap();
    assert_eq!(km.failed_samples.len(), 1);
    assert_eq!(km.failed_samples[0].0, "samples/gone.wav");
}
