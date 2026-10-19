use lpd_core::kit::ops::{clear_pad, move_pad};
use lpd_core::kit::{Pad, Set};
use lpd_core::seq::*;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn step_timing() {
    assert!(close(step_duration(120.0), 0.125));
    assert!(close(step_time(1, 120.0, 50.0), 0.125));
    assert!(close(step_time(1, 120.0, 75.0), 0.1875));
    assert!(close(step_time(2, 120.0, 75.0), 0.25));
    assert!(close(step_time(0, 120.0, 75.0), 0.0));
    assert!(close(loop_duration(16, 120.0), 2.0));
    assert!(close(loop_duration(4, 120.0), 0.5));
}

#[test]
fn recording_goes_to_the_nearest_step() {
    // 120 BPM, 16 steps: a step is 0.125 s, half a step 0.0625 s.
    assert_eq!(nearest_step(0.01, 16, 120.0, 50.0), (0, false));
    assert_eq!(nearest_step(0.07, 16, 120.0, 50.0), (1, true));
    assert_eq!(nearest_step(0.124, 16, 120.0, 50.0), (1, true));
    assert_eq!(nearest_step(0.126, 16, 120.0, 50.0), (1, false));
    // Step 15 starts at 1.875; 0.115 s later is closer to the next pass's step 0.
    assert_eq!(nearest_step(1.99, 16, 120.0, 50.0), (0, true));
    assert_eq!(nearest_step(1.9, 16, 120.0, 50.0), (15, false));
}

#[test]
fn due_steps_in_a_window() {
    assert_eq!(due_steps(-1e-9, 0.26, 16, 120.0, 50.0), vec![0, 1, 2]);
    assert_eq!(due_steps(0.0, 0.26, 16, 120.0, 50.0), vec![1, 2]);
    assert_eq!(due_steps(0.26, 0.26, 16, 120.0, 50.0), Vec::<usize>::new());
    // The window crosses the loop end (2.0 s): step 0 of the next pass is due.
    assert_eq!(due_steps(1.9, 2.01, 16, 120.0, 50.0), vec![0]);
    assert_eq!(due_steps(1.8, 2.13, 16, 120.0, 50.0), vec![15, 0, 1]);
    // Swing delays the odd steps.
    assert_eq!(due_steps(0.13, 0.18, 16, 120.0, 75.0), Vec::<usize>::new());
    assert_eq!(due_steps(0.13, 0.19, 16, 120.0, 75.0), vec![1]);
    // Only the steps inside the length fire.
    assert_eq!(
        due_steps(0.0, 0.99, 4, 120.0, 50.0),
        vec![1, 2, 3, 0, 1, 2, 3]
    );
}

fn lane_with(steps: &[(usize, u8)]) -> Lane {
    let mut lane = Lane {
        bank: 0,
        x: 0,
        y: 0,
        mute: false,
        steps: vec![0; MAX_STEPS],
    };
    for &(i, v) in steps {
        lane.steps[i] = v;
    }
    lane
}

#[test]
fn growing_repeats_an_empty_tail_and_keeps_a_used_one() {
    let mut p = Pattern::default();
    p.lanes.push(lane_with(&[(0, 1), (4, 2), (8, 1)]));
    set_length(&mut p, 32);
    assert_eq!(p.length, 32);
    let s = &p.lanes[0].steps;
    for i in 0..16 {
        assert_eq!(s[i], s[i + 16], "step {i}");
    }
    assert_eq!(s[4], 2);
    assert_eq!(s[20], 2);

    // 32 → 16 → 32 keeps the hidden half as it was.
    let mut q = Pattern::default();
    q.length = 32;
    q.lanes.push(lane_with(&[(0, 1), (20, 2)]));
    set_length(&mut q, 16);
    assert_eq!(q.lanes[0].steps[20], 2, "hidden steps are kept");
    set_length(&mut q, 32);
    assert_eq!(q.lanes[0].steps[20], 2);
    assert_eq!(q.lanes[0].steps[16], 0, "a used tail is not overwritten");

    // Lanes are judged one by one.
    let mut r = Pattern::default();
    r.lanes.push(lane_with(&[(0, 1)]));
    r.lanes.push(lane_with(&[(0, 1), (17, 1)]));
    set_length(&mut r, 32);
    assert_eq!(r.lanes[0].steps[16], 1);
    assert_eq!(r.lanes[1].steps[16], 0);
}

#[test]
fn growing_from_eight_repeats_the_eight() {
    let mut p = Pattern::default();
    p.length = 8;
    p.lanes.push(lane_with(&[(0, 1), (3, 2)]));
    set_length(&mut p, 16);
    let s = &p.lanes[0].steps;
    assert_eq!((s[8], s[11]), (1, 2));
    assert_eq!(s[16], 0, "only the new steps are filled");
}

#[test]
fn lanes_are_unique_per_pad() {
    let mut p = Pattern::default();
    assert!(add_lane(&mut p, 0, 1, 2).is_ok());
    assert_eq!(add_lane(&mut p, 0, 1, 2), Err("Lane already there"));
    assert!(add_lane(&mut p, 1, 1, 2).is_ok());
    assert_eq!(p.lanes.len(), 2);
    assert_eq!(p.lanes[0].steps.len(), MAX_STEPS);
}

#[test]
fn clear_steps_keeps_the_lanes() {
    let mut p = Pattern::default();
    p.lanes.push(lane_with(&[(0, 1), (5, 2)]));
    clear_steps(&mut p);
    assert_eq!(p.lanes.len(), 1);
    assert!(p.lanes[0].steps.iter().all(|&v| v == 0));
}

#[test]
fn an_old_set_without_a_pattern_loads_the_default() {
    let mut value = serde_json::to_value(Set::new()).unwrap();
    value.as_object_mut().unwrap().remove("pattern");
    let set = Set::from_json(&value.to_string()).unwrap();
    assert_eq!(set.pattern, Pattern::default());
    assert_eq!(set.pattern.length, 16);
    assert_eq!(set.pattern.bpm, 100.0);
}

#[test]
fn the_pattern_round_trips_through_the_set_file() {
    let mut set = Set::new();
    set.pattern.lanes.push(lane_with(&[(0, 2)]));
    set.pattern.bpm = 128.0;
    set.pattern.swing = 60.0;
    set.pattern.length = 8;
    let back = Set::from_json(&set.to_json()).unwrap();
    assert_eq!(back.pattern, set.pattern);
    let json = serde_json::to_value(&set.pattern).unwrap();
    assert_eq!(json["lanes"][0]["bank"], 0);
    assert_eq!(json["length"], 8);
}

fn set_with_lane() -> Set {
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "samples/a.wav", [63, 0, 0]));
    set.banks[0].set_pad(Pad::new(1, 0, "samples/b.wav", [0, 63, 0]));
    set
}

#[test]
fn a_lane_follows_its_pad_when_it_moves() {
    let mut set = set_with_lane();
    add_lane(&mut set.pattern, 0, 0, 0).unwrap();
    move_pad(&mut set, (0, 0, 0), (0, 5, 5), false).unwrap();
    let lane = &set.pattern.lanes[0];
    assert_eq!((lane.bank, lane.x, lane.y), (0, 5, 5));
}

#[test]
fn moving_onto_an_empty_pad_with_a_lane_keeps_one_lane() {
    let mut set = set_with_lane();
    add_lane(&mut set.pattern, 0, 0, 0).unwrap();
    add_lane(&mut set.pattern, 0, 5, 5).unwrap();
    set.pattern.lanes[0].steps[3] = 1;
    move_pad(&mut set, (0, 0, 0), (0, 5, 5), false).unwrap();
    assert_eq!(set.pattern.lanes.len(), 1);
    assert_eq!(
        set.pattern.lanes[0].steps[3], 1,
        "the moved pad's lane wins"
    );
}

#[test]
fn swapping_pads_swaps_their_lanes() {
    let mut set = set_with_lane();
    add_lane(&mut set.pattern, 0, 0, 0).unwrap();
    add_lane(&mut set.pattern, 0, 1, 0).unwrap();
    set.pattern.lanes[0].steps[0] = 1; // the lane of (0,0)
    move_pad(&mut set, (0, 0, 0), (0, 1, 0), false).unwrap();
    let first = &set.pattern.lanes[0];
    assert_eq!((first.x, first.y), (1, 0));
    assert_eq!(first.steps[0], 1, "the steps stay with the lane");
    let second = &set.pattern.lanes[1];
    assert_eq!((second.x, second.y), (0, 0));
}

#[test]
fn copying_adds_no_lane_and_clearing_keeps_it() {
    let mut set = set_with_lane();
    add_lane(&mut set.pattern, 0, 0, 0).unwrap();
    move_pad(&mut set, (0, 0, 0), (0, 5, 5), true).unwrap();
    assert_eq!(set.pattern.lanes.len(), 1);
    assert_eq!(set.pattern.lanes[0].x, 0);
    assert!(clear_pad(&mut set, 0, 0, 0));
    assert_eq!(set.pattern.lanes.len(), 1, "the lane shows EMPTY in the UI");
}
