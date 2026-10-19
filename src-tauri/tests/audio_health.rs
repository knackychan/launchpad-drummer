//! Finding 13: a dead output stream must show up in the application status, once.

mod common;

use common::rig;
use launchpad_drummer_lib::audio::{poll_stream_health, AudioStatus, StreamHealth};

fn running() -> AudioStatus {
    AudioStatus {
        state: "running".into(),
        device_name: "Speakers".into(),
        sample_rate: 48_000,
        buffer_size: Some(128),
        channels: 2,
        dropouts: 0,
        last_dropout_ago_ms: None,
    }
}

#[test]
fn a_stream_error_turns_running_into_error_exactly_once() {
    let r = rig(16);
    r.state.set_audio_status(running());
    let health = StreamHealth::default();
    assert!(
        poll_stream_health(&health, &r.state).is_none(),
        "healthy: no event"
    );

    // A burst of errors, as a failing device produces.
    for _ in 0..50 {
        health.report_failure();
    }
    let status = poll_stream_health(&health, &r.state).expect("first failure is reported");
    assert_eq!(status.state, "error");
    assert_eq!(
        status.device_name, "Speakers",
        "the rest of the status is kept"
    );
    assert_eq!(r.state.get_audio_status().state, "error");

    for _ in 0..50 {
        health.report_failure();
    }
    assert!(
        poll_stream_health(&health, &r.state).is_none(),
        "no repeat event"
    );
    assert_eq!(r.state.get_audio_status().state, "error");
}

#[test]
fn a_restart_recovers_and_the_old_stream_cannot_fail_the_new_one() {
    let r = rig(16);
    r.state.set_audio_status(running());
    let old = StreamHealth::default();
    old.report_failure();
    assert!(poll_stream_health(&old, &r.state).is_some());

    // The audio owner builds a new stream with its own health flag.
    r.state.set_audio_status(running());
    let new = StreamHealth::default();
    assert_eq!(r.state.get_audio_status().state, "running");

    // The dying old stream keeps complaining; only its own flag is touched.
    old.report_failure();
    assert!(poll_stream_health(&new, &r.state).is_none());
    assert_eq!(r.state.get_audio_status().state, "running");

    new.report_failure();
    assert!(poll_stream_health(&new, &r.state).is_some());
    assert_eq!(r.state.get_audio_status().state, "error");
}

#[test]
fn a_stream_that_never_started_is_not_reported_again() {
    let r = rig(16);
    let mut status = running();
    status.state = "error".into();
    r.state.set_audio_status(status);
    let health = StreamHealth::default();
    health.report_failure();
    assert!(poll_stream_health(&health, &r.state).is_none());
}
