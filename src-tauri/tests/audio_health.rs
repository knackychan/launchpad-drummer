//! A dead output stream must show up in the application status as "lost", once, and a restart
//! recovers. The stream's error callback only sets `AudioStats.lost`.

mod common;

use common::rig;
use launchpad_drummer_lib::audio::{AudioStats, AudioStatus};
use std::sync::Arc;

fn running() -> AudioStatus {
    AudioStatus {
        state: "running".into(),
        device_name: "Speakers".into(),
        buffer_size: Some(128),
        ..Default::default()
    }
}

#[test]
fn a_stream_error_turns_running_into_lost_exactly_once() {
    let r = rig(16);
    r.state.set_audio_status(running());
    let stats = Arc::new(AudioStats::new());
    r.state.set_audio_stats(stats.clone());
    assert!(r.state.poll_stream_lost().is_none(), "healthy: no event");

    // A burst of errors, as a failing device produces.
    for _ in 0..50 {
        stats.report_lost();
    }
    let status = r
        .state
        .poll_stream_lost()
        .expect("first failure is reported");
    assert_eq!(status.state, "lost");
    assert_eq!(
        status.device_name, "Speakers",
        "the rest of the status is kept"
    );
    assert_eq!(r.state.get_audio_status().state, "lost");

    for _ in 0..50 {
        stats.report_lost();
    }
    assert!(r.state.poll_stream_lost().is_none(), "no repeat event");
    assert_eq!(r.state.get_audio_status().state, "lost");
}

#[test]
fn a_restart_recovers_and_the_old_stream_cannot_fail_the_new_one() {
    let r = rig(16);
    r.state.set_audio_status(running());
    let old = Arc::new(AudioStats::new());
    r.state.set_audio_stats(old.clone());
    old.report_lost();
    assert!(r.state.poll_stream_lost().is_some());

    // The audio owner builds a new stream with its own stats.
    r.state.set_audio_status(running());
    let new = Arc::new(AudioStats::new());
    r.state.set_audio_stats(new.clone());
    assert_eq!(r.state.get_audio_status().state, "running");

    // The dying old stream keeps complaining; only its own flag is touched.
    old.report_lost();
    assert!(r.state.poll_stream_lost().is_none());
    assert_eq!(r.state.get_audio_status().state, "running");

    new.report_lost();
    assert!(r.state.poll_stream_lost().is_some());
    assert_eq!(r.state.get_audio_status().state, "lost");
}

#[test]
fn a_stream_that_never_started_is_not_reported_as_lost() {
    let r = rig(16);
    let mut status = running();
    status.state = "error".into();
    r.state.set_audio_status(status);
    let stats = Arc::new(AudioStats::new());
    r.state.set_audio_stats(stats.clone());
    stats.report_lost();
    assert!(r.state.poll_stream_lost().is_none());
    assert_eq!(r.state.get_audio_status().state, "error");
}

#[test]
fn the_status_carries_latency_from_the_live_counters() {
    let r = rig(16);
    r.state.set_audio_status(running());
    let stats = Arc::new(AudioStats::new());
    r.state.set_audio_stats(stats.clone());
    stats.set_callback_info(128, Some(std::time::Duration::from_micros(10_000)));
    let l = r.state.get_audio_status().latency;
    assert_eq!(l.midi_to_buffer_ms, 2.7, "buffer 128 at 48 kHz");
    assert_eq!(l.output_ms, 10.0);
    assert_eq!(l.total_ms, 12.7);

    // Driver-chosen buffer: the callback's measured frames stand in.
    let mut auto = running();
    auto.buffer_size = None;
    r.state.set_audio_status(auto);
    stats.set_callback_info(480, None);
    assert_eq!(r.state.get_audio_status().latency.midi_to_buffer_ms, 10.0);
}
