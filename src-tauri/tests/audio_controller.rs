//! The audio owner thread: restarts, failures and reconnect, with a fake starter (no device).

mod common;

use anyhow::anyhow;
use common::rig;
use launchpad_drummer_lib::audio::{AudioController, AudioStats, AudioStatus, AudioSystem};
use launchpad_drummer_lib::config::AudioPrefs;
use lpd_core::audio::{Command, Engine};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// A starter that records the prefs it was asked for and hands the new command queue to the test.
struct Fake {
    prefs: Arc<Mutex<Vec<AudioPrefs>>>,
    queues: mpsc::Receiver<rtrb::Consumer<Command>>,
}

fn fake_starter(fail_on: Option<&'static str>) -> (launchpad_drummer_lib::audio::Starter, Fake) {
    let prefs = Arc::new(Mutex::new(Vec::new()));
    let (queue_tx, queues) = mpsc::channel();
    let seen = prefs.clone();
    let starter: launchpad_drummer_lib::audio::Starter = Box::new(move |p: &AudioPrefs| {
        seen.lock().unwrap().push(p.clone());
        if fail_on.is_some_and(|d| p.device.as_deref() == Some(d)) {
            return Err(anyhow!("no such device"));
        }
        let (_engine, events, taps) = Engine::with_taps(44_100);
        let (commands, rx) = rtrb::RingBuffer::new(1024);
        queue_tx.send(rx).unwrap();
        Ok(AudioSystem {
            stream: None,
            commands,
            events,
            status: AudioStatus {
                state: "running".into(),
                device_name: p.device.clone().unwrap_or_else(|| "Default".into()),
                sample_rate: 44_100,
                buffer_size: p.buffer_frames,
                ..Default::default()
            },
            taps,
            stats: Arc::new(AudioStats::new()),
            trash: None,
        })
    });
    (starter, Fake { prefs, queues })
}

fn prefs(device: Option<&str>) -> AudioPrefs {
    AudioPrefs {
        device: device.map(String::from),
        ..Default::default()
    }
}

#[test]
fn start_swaps_queues_taps_and_status_and_resends_the_mixer() {
    let r = rig(16);
    let (starter, fake) = fake_starter(None);
    let controller = AudioController::spawn_with(r.state.clone(), starter, Box::new(|_| {}));
    assert!(r.state.taps.lock().unwrap().is_none());

    let status = controller.start(prefs(Some("Speakers"))).unwrap();
    assert_eq!(status.state, "running");
    assert_eq!(status.device_name, "Speakers");
    assert_eq!(r.state.get_audio_status().sample_rate, 44_100);
    assert_eq!(r.state.sample_rate(), 44_100);
    assert!(r.state.taps.lock().unwrap().is_some());

    let mut queue = fake.queues.recv().unwrap();
    let mut mixer = 0;
    let mut keys = 0;
    while let Ok(c) = queue.pop() {
        match c {
            Command::SetGroup { .. } | Command::SetMasterGain(_) => mixer += 1,
            // The keys' sound, route and wheels follow the mixer (Task 15).
            Command::KeysSound(_)
            | Command::KeysRoute { .. }
            | Command::KeysPitch(_)
            | Command::KeysMod(_) => keys += 1,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(mixer, 9);
    assert_eq!(keys, 4);
}

#[test]
fn a_second_start_replaces_the_first_and_uses_the_new_prefs() {
    let r = rig(16);
    let (starter, fake) = fake_starter(None);
    let controller = AudioController::spawn_with(r.state.clone(), starter, Box::new(|_| {}));
    controller.start(prefs(Some("A"))).unwrap();
    controller.start(prefs(Some("B"))).unwrap();
    assert_eq!(r.state.get_audio_status().device_name, "B");
    let seen = fake.prefs.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[1].device.as_deref(), Some("B"));
}

#[test]
fn a_failed_start_reports_the_error_and_leaves_an_error_status() {
    let r = rig(16);
    let (starter, _fake) = fake_starter(Some("Gone"));
    let controller = AudioController::spawn_with(r.state.clone(), starter, Box::new(|_| {}));
    controller.start(prefs(Some("A"))).unwrap();
    let err = controller.start(prefs(Some("Gone"))).unwrap_err();
    assert!(err.contains("no such device"), "{err}");
    let status = r.state.get_audio_status();
    assert_eq!(status.state, "error");
    assert!(
        r.state.taps.lock().unwrap().is_none(),
        "no taps without a stream"
    );

    // A later good start recovers.
    controller.start(prefs(Some("A"))).unwrap();
    assert_eq!(r.state.get_audio_status().state, "running");
}

#[test]
fn reconnect_restarts_only_when_lost_stopped_or_failed() {
    let r = rig(16);
    r.state.set_audio_prefs(prefs(Some("Saved"))).unwrap();
    let (starter, fake) = fake_starter(None);
    let controller = AudioController::spawn_with(r.state.clone(), starter, Box::new(|_| {}));

    // Never started: "stopped" -> starts with the saved prefs.
    let status = controller.reconnect(&r.state).unwrap();
    assert_eq!(status.state, "running");
    assert_eq!(fake.prefs.lock().unwrap().len(), 1);
    assert_eq!(
        fake.prefs.lock().unwrap()[0].device.as_deref(),
        Some("Saved")
    );

    // Running: nothing restarts.
    let status = controller.reconnect(&r.state).unwrap();
    assert_eq!(status.state, "running");
    assert_eq!(fake.prefs.lock().unwrap().len(), 1);

    // Lost: restarts.
    let stats = Arc::new(AudioStats::new());
    r.state.set_audio_stats(stats.clone());
    stats.report_lost();
    assert_eq!(r.state.poll_stream_lost().unwrap().state, "lost");
    let status = controller.reconnect(&r.state).unwrap();
    assert_eq!(status.state, "running");
    assert_eq!(fake.prefs.lock().unwrap().len(), 2);
}

#[test]
fn apply_saves_the_prefs_before_restarting() {
    let r = rig(16);
    let (starter, _fake) = fake_starter(Some("Gone"));
    let controller = AudioController::spawn_with(r.state.clone(), starter, Box::new(|_| {}));
    let mut p = prefs(Some("Gone"));
    p.buffer_frames = Some(128);
    assert!(controller.apply(&r.state, p.clone()).is_err());
    assert_eq!(r.state.get_config().audio, p, "kept so a retry can use it");
}

/// A system with no stream, for starters written inline in a test.
fn healthy(p: &AudioPrefs) -> AudioSystem {
    let (_engine, events, taps) = Engine::with_taps(44_100);
    let (commands, _rx) = rtrb::RingBuffer::new(1024);
    AudioSystem {
        stream: None,
        commands,
        events,
        status: AudioStatus {
            state: "running".into(),
            buffer_size: p.buffer_frames,
            ..Default::default()
        },
        taps,
        stats: Arc::new(AudioStats::new()),
        trash: None,
    }
}

#[test]
fn a_rejected_fixed_buffer_is_retried_once_with_auto() {
    let r = rig(16);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let starter: launchpad_drummer_lib::audio::Starter = Box::new(move |p: &AudioPrefs| {
        log.lock().unwrap().push(p.buffer_frames);
        if p.buffer_frames.is_some() {
            return Err(anyhow!("buffer size not supported"));
        }
        Ok(healthy(p))
    });
    let controller = AudioController::spawn_with(r.state.clone(), starter, Box::new(|_| {}));
    let mut p = prefs(None);
    p.buffer_frames = Some(96);
    let status = controller.apply(&r.state, p.clone()).unwrap();
    assert_eq!(status.state, "running");
    assert_eq!(status.buffer_size, None, "AUTO is running");
    assert_eq!(*seen.lock().unwrap(), vec![Some(96), None]);
    assert_eq!(r.state.get_config().audio, p, "the saved prefs are kept");

    // A failure that is not about the buffer is reported after the single retry.
    let failing: launchpad_drummer_lib::audio::Starter =
        Box::new(|_: &AudioPrefs| Err(anyhow!("no device")));
    let controller = AudioController::spawn_with(r.state.clone(), failing, Box::new(|_| {}));
    assert!(controller.start(p).is_err());
}

#[test]
fn a_failed_start_forgets_the_old_streams_voices() {
    let mut r = rig(16);
    r.events
        .push(lpd_core::audio::Event::VoiceStarted { pad: 0 })
        .unwrap();
    r.state.maintain();
    assert!(r.state.router.lock().unwrap().is_pad_sounding(0, 0, 0));

    let failing: launchpad_drummer_lib::audio::Starter =
        Box::new(|_: &AudioPrefs| Err(anyhow!("gone")));
    let controller = AudioController::spawn_with(r.state.clone(), failing, Box::new(|_| {}));
    assert!(controller.start(prefs(None)).is_err());
    assert!(!r.state.router.lock().unwrap().is_pad_sounding(0, 0, 0));
}

#[test]
fn a_panicking_start_is_an_error_and_the_thread_survives() {
    let r = rig(16);
    let first = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let starter: launchpad_drummer_lib::audio::Starter = Box::new(move |p: &AudioPrefs| {
        if first.swap(false, std::sync::atomic::Ordering::SeqCst) {
            panic!("driver blew up");
        }
        Ok(healthy(p))
    });
    let controller = AudioController::spawn_with(r.state.clone(), starter, Box::new(|_| {}));
    let err = controller.start(prefs(None)).unwrap_err();
    assert!(err.contains("panicked"), "{err}");
    assert_eq!(r.state.get_audio_status().state, "error");
    assert_eq!(controller.start(prefs(None)).unwrap().state, "running");
}

#[test]
fn every_finished_start_is_announced_even_without_a_waiting_reply() {
    let r = rig(16);
    let (starter, _fake) = fake_starter(Some("Gone"));
    let (tx, announced) = mpsc::channel();
    let controller = AudioController::spawn_with(
        r.state.clone(),
        starter,
        Box::new(move |s| {
            let _ = tx.send(s.state);
        }),
    );
    controller.start_detached(prefs(Some("A")));
    assert_eq!(
        announced
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap(),
        "running"
    );
    controller.start_detached(prefs(Some("Gone")));
    assert_eq!(
        announced
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap(),
        "error"
    );
}
