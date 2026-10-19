//! Task 8: the telemetry thread's per-tick logic, the dropout counter and the status fields.

mod common;

use common::rig;
use launchpad_drummer_lib::audio::AudioStats;
use launchpad_drummer_lib::telemetry::Telemetry;
use lpd_core::audio::{Command, Engine, PlayParams, Sample};
use std::sync::Arc;
use std::time::{Duration, Instant};

const SR: u32 = 48_000;

fn play_dc(engine: &mut Engine, group: u8, value: f32) {
    let sample = Arc::new(Sample {
        rate: SR,
        data: vec![value; SR as usize * 2],
    });
    engine.handle(Command::Trigger {
        pad: 1,
        sample: sample.clone(),
        params: PlayParams {
            gain: 1.0,
            pitch_semitones: 0.0,
            start_frame: 0,
            end_frame: sample.frames(),
            choke_group: None,
            mix_group: group,
            hold: false,
        },
    });
}

fn render(engine: &mut Engine, frames: usize) {
    let mut out = vec![0.0; frames * 2];
    engine.render(&mut out);
}

#[test]
fn a_tick_with_the_feed_on_emits_a_frame_with_the_documented_shape() {
    let r = rig(16);
    let (mut engine, _events, taps) = Engine::with_taps(SR);
    r.state.set_taps(Some(taps));
    r.state.set_telemetry(true);
    play_dc(&mut engine, 3, 0.5);
    render(&mut engine, 4096);

    let mut t = Telemetry::new();
    let tick = t.tick(&r.state);
    let frame = tick.frame.expect("feed is on");
    assert_eq!(frame.scope.len(), 1024);
    assert_eq!(frame.bands.len(), 28);
    assert_eq!(frame.groups.len(), 8);
    assert!(frame.groups[3] > 0.0 && frame.groups[3] <= 1.0);
    assert_eq!(frame.groups[0], 0.0);
    // DC 0.5 on both channels is about -6 dBFS.
    assert!((frame.master[0] + 6.02).abs() < 0.1, "{:?}", frame.master);
    assert!((frame.master[1] + 6.02).abs() < 0.1);
    assert!(frame.scope.iter().all(|s| (s - 0.5).abs() < 1e-3));

    // JSON keys the UI reads.
    let json = serde_json::to_value(&frame).unwrap();
    for key in ["scope", "bands", "peakHz", "master", "groups"] {
        assert!(json.get(key).is_some(), "missing {key}");
    }
}

#[test]
fn scope_values_are_rounded_to_three_decimals() {
    let r = rig(16);
    let (mut engine, _events, taps) = Engine::with_taps(SR);
    r.state.set_taps(Some(taps));
    r.state.set_telemetry(true);
    play_dc(&mut engine, 0, 0.123456);
    render(&mut engine, 4096);
    let frame = Telemetry::new().tick(&r.state).frame.unwrap();
    assert!(frame.scope.iter().all(|s| *s == 0.123));
}

#[test]
fn with_the_feed_off_nothing_is_emitted_but_the_queues_are_still_drained() {
    let r = rig(16);
    let (mut engine, _events, taps) = Engine::with_taps(SR);
    r.state.set_taps(Some(taps));
    play_dc(&mut engine, 0, 0.5);
    render(&mut engine, 256);
    engine.handle(Command::StopAll);
    render(&mut engine, 2048);

    let mut t = Telemetry::new();
    assert!(t.tick(&r.state).frame.is_none());
    // Switching it on later shows only what arrives afterwards: the loud block was drained
    // and its peak discarded while the feed was off.
    r.state.set_telemetry(true);
    render(&mut engine, 128);
    let frame = t.tick(&r.state).frame.unwrap();
    assert_eq!(frame.master, [-100.0, -100.0]);
    assert_eq!(frame.groups, vec![0.0; 8]);
}

#[test]
fn per_channel_peaks_are_the_max_since_the_last_frame_then_reset() {
    let r = rig(16);
    let (mut engine, _events, taps) = Engine::with_taps(SR);
    r.state.set_taps(Some(taps));
    r.state.set_telemetry(true);
    play_dc(&mut engine, 2, 0.25);
    render(&mut engine, 256);
    engine.handle(Command::StopAll);
    // Long enough for the voice to fade out and be silent.
    render(&mut engine, 2048);
    let mut t = Telemetry::new();
    let first = t.tick(&r.state).frame.unwrap();
    assert!(first.groups[2] > 0.2, "peak of the loud block is kept");
    let second = t.tick(&r.state).frame.unwrap();
    assert_eq!(second.groups[2], 0.0, "reset after a frame");
    assert_eq!(second.master, [-100.0, -100.0]);
}

#[test]
fn without_taps_a_tick_is_harmless() {
    let r = rig(16);
    r.state.set_telemetry(true);
    let tick = Telemetry::new().tick(&r.state);
    assert!(tick.frame.is_some());
    assert!(!tick.dropouts_changed);
}

#[test]
fn dropouts_are_counted_reported_once_per_change_and_reset() {
    let r = rig(16);
    let stats = Arc::new(AudioStats::new());
    r.state.set_audio_stats(stats.clone());
    let mut t = Telemetry::new();
    assert!(!t.tick(&r.state).dropouts_changed);

    stats.record_dropout(Instant::now());
    stats.record_dropout(Instant::now());
    assert!(t.tick(&r.state).dropouts_changed);
    assert!(!t.tick(&r.state).dropouts_changed);

    let status = r.state.get_audio_status();
    assert_eq!(status.dropouts, 2);
    assert!(status.last_dropout_ago_ms.is_some());
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["dropouts"], 2);
    assert!(json["lastDropoutAgoMs"].is_u64());

    r.state.reset_dropouts();
    assert!(t.tick(&r.state).dropouts_changed);
    let status = r.state.get_audio_status();
    assert_eq!(status.dropouts, 0);
    assert_eq!(status.last_dropout_ago_ms, None);
}

#[test]
fn last_dropout_age_counts_up_from_the_event() {
    let stats = AudioStats::new();
    assert_eq!(stats.last_dropout_ago_ms(Instant::now()), None);
    let at = Instant::now();
    stats.record_dropout(at);
    let ago = stats.last_dropout_ago_ms(at + Duration::from_millis(2500));
    assert_eq!(ago, Some(2500));
}

#[test]
fn a_lost_stream_is_reported_by_one_tick_with_state_lost() {
    let r = rig(16);
    r.state
        .set_audio_status(launchpad_drummer_lib::audio::AudioStatus {
            state: "running".into(),
            ..Default::default()
        });
    let stats = Arc::new(AudioStats::new());
    r.state.set_audio_stats(stats.clone());
    let mut t = Telemetry::new();
    assert!(t.tick(&r.state).lost.is_none());

    stats.report_lost();
    let lost = t.tick(&r.state).lost.expect("the flag is picked up");
    assert_eq!(lost.state, "lost");
    assert!(t.tick(&r.state).lost.is_none(), "reported once");
}
