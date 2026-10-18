//! Finding 11: a drag on one control is one undo step.

mod common;

use common::rig;
use lpd_core::kit::Pad;
use std::time::{Duration, Instant};

fn state_with_pad() -> launchpad_drummer_lib::state::AppState {
    let r = rig(4096);
    r.state
        .update_pad(2, Pad::new(3, 2, "samples/a.wav", [1, 2, 3]), None);
    // Keep the queue alive for the test's lifetime.
    std::mem::forget(r.commands);
    r.state
}

fn volume(state: &launchpad_drummer_lib::state::AppState) -> f32 {
    state.get_current_set().banks[2]
        .pad(3, 2)
        .unwrap()
        .volume_db
}

fn drag(
    state: &launchpad_drummer_lib::state::AppState,
    key: &str,
    from: Instant,
    updates: u32,
    f: impl Fn(&mut Pad, f32),
) {
    for i in 0..updates {
        let at = from + Duration::from_millis(i as u64 * 5);
        state.edit_at(Some(key), at, |set| {
            f(set.banks[2].pad_mut(3, 2), i as f32 + 1.0)
        });
    }
}

trait PadMut {
    fn pad_mut(&mut self, x: u8, y: u8) -> &mut Pad;
}
impl PadMut for lpd_core::kit::Bank {
    fn pad_mut(&mut self, x: u8, y: u8) -> &mut Pad {
        self.pads.iter_mut().find(|p| p.x == x && p.y == y).unwrap()
    }
}

#[test]
fn seventy_updates_of_one_control_undo_in_one_step() {
    let state = state_with_pad();
    let t0 = Instant::now();
    drag(&state, "pad:2:3:2:volume", t0, 70, |p, v| {
        p.volume_db = -v * 0.1
    });
    assert_eq!(volume(&state), -7.0);

    let undone = state.undo().unwrap();
    assert_eq!(
        undone.banks[2].pad(3, 2).unwrap().volume_db,
        0.0,
        "back to the start"
    );
    // The next undo removes the pad creation, not a leftover slice of the drag.
    let undone = state.undo().unwrap();
    assert!(undone.banks[2].pad(3, 2).is_none());
    assert!(state.undo().is_none());
}

#[test]
fn a_different_control_a_discrete_action_or_a_pause_starts_a_new_step() {
    let state = state_with_pad();
    let t0 = Instant::now();
    drag(&state, "pad:2:3:2:volume", t0, 10, |p, v| p.volume_db = -v);
    drag(
        &state,
        "pad:2:3:2:pitch",
        t0 + Duration::from_millis(100),
        10,
        |p, v| p.pitch_semitones = v,
    );
    state.edit_at(None, t0 + Duration::from_millis(200), |set| {
        set.banks[2].pad_mut(3, 2).mix_group = 5
    });
    drag(
        &state,
        "pad:2:3:2:pitch",
        t0 + Duration::from_millis(300),
        3,
        |p, v| p.pitch_semitones = 50.0 + v,
    );
    drag(
        &state,
        "pad:2:3:2:pitch",
        t0 + Duration::from_secs(5),
        3,
        |p, v| p.pitch_semitones = 80.0 + v,
    );

    // 5 steps: volume drag, pitch drag, mix-group change, pitch drag again, pitch after a pause.
    let steps = std::iter::from_fn(|| state.undo()).count();
    assert_eq!(steps, 5 + 1, "plus the pad creation");
}

#[test]
fn a_new_edit_after_undo_discards_redo() {
    let state = state_with_pad();
    let t0 = Instant::now();
    drag(&state, "pad:2:3:2:volume", t0, 5, |p, v| p.volume_db = -v);
    state.undo().unwrap();
    assert!(state.history_state().can_redo);
    drag(
        &state,
        "pad:2:3:2:volume",
        t0 + Duration::from_secs(9),
        2,
        |p, v| p.volume_db = -v * 2.0,
    );
    assert!(!state.history_state().can_redo);
}

#[test]
fn bank_humanize_is_coalesced_by_its_own_key() {
    let r = rig(256);
    for i in 0..70 {
        r.state.update_bank_humanize(0, (i % 7) as f32 * 0.5);
    }
    let undone = r.state.undo().unwrap();
    assert_eq!(undone.banks[0].humanize_db, 0.0);
    assert!(r.state.undo().is_none(), "the whole drag was one step");
}
