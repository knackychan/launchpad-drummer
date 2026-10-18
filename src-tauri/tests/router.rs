use launchpad_drummer_lib::router::Router;
use lpd_core::audio::{Command, Event, Sample};
use lpd_core::devices::launchpad::LpEvent;
use lpd_core::kit::{Pad, Set};
use std::collections::HashMap;
use std::sync::Arc;

fn test_router() -> (Router, rtrb::Consumer<Command>, rtrb::Producer<Event>) {
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "samples/kick.wav", [63, 0, 0]));
    set.banks[1].set_pad(Pad::new(0, 0, "samples/snare.wav", [0, 63, 0]));
    let mut samples = HashMap::new();
    samples.insert(
        "samples/kick.wav".into(),
        Arc::new(Sample {
            rate: 48_000,
            data: vec![0.5; 480],
        }),
    );
    samples.insert(
        "samples/snare.wav".into(),
        Arc::new(Sample {
            rate: 48_000,
            data: vec![0.5; 480],
        }),
    );
    let (tx_cmd, mut rx_cmd) = rtrb::RingBuffer::new(64);
    let (tx_evt, rx_evt) = rtrb::RingBuffer::new(64);
    let router = Router::new(set, samples, tx_cmd, rx_evt);
    while rx_cmd.pop().is_ok() {}
    (router, rx_cmd, tx_evt)
}

#[test]
fn trigger_pad_sends_command_and_tracks_held() {
    let (mut router, mut rx_cmd, _tx_evt) = test_router();
    router.on_lp_event(LpEvent::PadDown { x: 0, y: 0 });
    let cmd = rx_cmd.pop().expect("command sent");
    assert!(matches!(cmd, Command::Trigger { pad: 0, .. }));
}

#[test]
fn bank_switch_changes_active_bank() {
    let (mut router, mut rx_cmd, _tx_evt) = test_router();
    router.on_lp_event(LpEvent::SideDown(1));
    assert_eq!(router.bank(), 1);
    router.on_lp_event(LpEvent::PadDown { x: 0, y: 0 });
    let cmd = rx_cmd.pop().expect("command sent");
    assert!(matches!(cmd, Command::Trigger { pad: 64, .. }));
}

#[test]
fn voice_events_update_sounding_status() {
    let (mut router, _rx_cmd, mut tx_evt) = test_router();
    assert!(!router.is_pad_sounding(0, 0, 0));
    tx_evt.push(Event::VoiceStarted { pad: 0 }).unwrap();
    router.poll_audio_events();
    assert!(router.is_pad_sounding(0, 0, 0));
    tx_evt.push(Event::VoiceEnded { pad: 0 }).unwrap();
    router.poll_audio_events();
    assert!(!router.is_pad_sounding(0, 0, 0));
}

#[test]
fn top_mixer_button_stops_all() {
    let (mut router, mut rx_cmd, _tx_evt) = test_router();
    router.on_lp_event(LpEvent::TopDown(7));
    let cmd = rx_cmd.pop().expect("command sent");
    assert!(matches!(cmd, Command::StopAll));
}
