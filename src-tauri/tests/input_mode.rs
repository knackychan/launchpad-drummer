//! Finding 7: a game that owns the Launchpad must not play or navigate the sampler.

mod common;

use common::{drain, rig, Rig};
use launchpad_drummer_lib::state::InputMode;
use lpd_core::audio::{Command, Sample};
use lpd_core::devices::launchpad::LpEvent;
use lpd_core::kit::{Pad, PadMode};
use std::sync::Arc;

fn sounding_kit() -> Rig {
    let mut r = rig(4096);
    r.state.edit(None, |set| {
        let mut hold = Pad::new(0, 0, "samples/a.wav", [60, 0, 0]);
        hold.mode = PadMode::Hold;
        set.banks[0].set_pad(hold);
        set.banks[0].set_pad(Pad::new(1, 0, "samples/a.wav", [0, 60, 0]));
    });
    r.state.router.lock().unwrap().install_sample(
        "samples/a.wav".into(),
        Arc::new(Sample {
            rate: 48_000,
            data: vec![0.5; 4_800],
        }),
    );
    drain(&mut r.commands);
    r
}

#[test]
fn game_input_never_reaches_the_sampler() {
    let mut r = sounding_kit();
    r.state.select_bank(3);
    assert_eq!(r.state.set_input_mode(InputMode::Game), 3);
    drain(&mut r.commands);

    for ev in [
        LpEvent::PadDown { x: 0, y: 0 },
        LpEvent::PadUp { x: 0, y: 0 },
        LpEvent::PadDown { x: 1, y: 0 },
        LpEvent::SideDown(1),
        LpEvent::SideDown(6),
        LpEvent::TopDown(7), // Mixer = stop all
    ] {
        r.state.on_lp_event(ev);
    }
    assert!(
        drain(&mut r.commands).is_empty(),
        "no trigger, release or stop"
    );
    assert_eq!(r.state.current_bank(), 3, "no bank change");

    assert_eq!(r.state.set_input_mode(InputMode::Sampler), 3);
    r.state.on_lp_event(LpEvent::SideDown(0));
    assert_eq!(r.state.current_bank(), 0, "sampler input works again");
    r.state.on_lp_event(LpEvent::PadDown { x: 1, y: 0 });
    assert!(drain(&mut r.commands)
        .iter()
        .any(|c| matches!(c, Command::Trigger { .. })));
}

#[test]
fn entering_a_game_releases_a_held_pad_first() {
    let mut r = sounding_kit();
    r.state.on_lp_event(LpEvent::PadDown { x: 0, y: 0 });
    drain(&mut r.commands);

    r.state.set_input_mode(InputMode::Game);
    let queued = drain(&mut r.commands);
    assert!(
        queued
            .iter()
            .any(|c| matches!(c, Command::Release { pad: 0 })),
        "{queued:?}"
    );
    // The late button-up from the hardware does not release twice.
    r.state.on_lp_event(LpEvent::PadUp { x: 0, y: 0 });
    assert!(drain(&mut r.commands).is_empty());
}

#[test]
fn mode_changes_are_idempotent() {
    let mut r = sounding_kit();
    r.state.on_lp_event(LpEvent::PadDown { x: 0, y: 0 });
    r.state.set_input_mode(InputMode::Game);
    drain(&mut r.commands);
    r.state.set_input_mode(InputMode::Game);
    assert!(drain(&mut r.commands).is_empty());
    assert_eq!(r.state.input_mode(), InputMode::Game);
}
