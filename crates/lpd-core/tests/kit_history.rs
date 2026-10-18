use lpd_core::kit::*;
use std::time::{Duration, Instant};

fn set_named(name: &str) -> Set {
    let mut set = Set::new();
    set.banks[0].name = name.into();
    set
}

#[test]
fn record_then_undo_returns_earlier_set_and_redo_the_later() {
    let mut h = History::new();
    let t = Instant::now();
    let a = set_named("A");
    let b = set_named("B");
    assert!(!h.can_undo() && !h.can_redo());
    h.record(&a, None, t);
    assert!(h.can_undo());
    let undone = h.undo(&b).expect("one step");
    assert_eq!(undone, a);
    assert!(!h.can_undo() && h.can_redo());
    let redone = h.redo(&undone).expect("redo step");
    assert_eq!(redone, b);
    assert!(h.can_undo() && !h.can_redo());
    assert_eq!(h.redo(&redone), None);
}

#[test]
fn new_record_clears_redo() {
    let mut h = History::new();
    let t = Instant::now();
    h.record(&set_named("A"), None, t);
    h.undo(&set_named("B")).unwrap();
    assert!(h.can_redo());
    h.record(&set_named("A"), None, t);
    assert!(!h.can_redo());
}

#[test]
fn keeps_at_most_fifty_steps() {
    let mut h = History::new();
    let t = Instant::now();
    for i in 0..=UNDO_STEPS {
        h.record(&set_named(&format!("S{i}")), None, t);
    }
    let mut current = set_named("now");
    let mut steps = 0;
    while let Some(prev) = h.undo(&current) {
        current = prev;
        steps += 1;
    }
    assert_eq!(steps, UNDO_STEPS);
    // The oldest record (S0) was dropped.
    assert_eq!(current.banks[0].name, "S1");
}

#[test]
fn same_key_within_one_second_coalesces() {
    let mut h = History::new();
    let t = Instant::now();
    h.record(&set_named("A"), Some("knob"), t);
    h.record(
        &set_named("B"),
        Some("knob"),
        t + Duration::from_millis(600),
    );
    // Refreshed time: still within 1 s of the previous record.
    h.record(
        &set_named("C"),
        Some("knob"),
        t + Duration::from_millis(1200),
    );
    let undone = h.undo(&set_named("D")).unwrap();
    assert_eq!(undone.banks[0].name, "A");
    assert!(!h.can_undo());
}

#[test]
fn same_key_after_one_second_does_not_coalesce() {
    let mut h = History::new();
    let t = Instant::now();
    h.record(&set_named("A"), Some("knob"), t);
    h.record(
        &set_named("B"),
        Some("knob"),
        t + Duration::from_millis(1100),
    );
    let undone = h.undo(&set_named("C")).unwrap();
    assert_eq!(undone.banks[0].name, "B");
    assert!(h.can_undo());
}

#[test]
fn different_key_or_none_never_coalesces() {
    let mut h = History::new();
    let t = Instant::now();
    h.record(&set_named("A"), Some("knob"), t);
    h.record(&set_named("B"), Some("other"), t);
    h.record(&set_named("C"), None, t);
    h.record(&set_named("D"), None, t);
    let mut current = set_named("E");
    let mut steps = 0;
    while let Some(prev) = h.undo(&current) {
        current = prev;
        steps += 1;
    }
    assert_eq!(steps, 4);
}

#[test]
fn clear_drops_everything() {
    let mut h = History::new();
    let t = Instant::now();
    h.record(&set_named("A"), None, t);
    h.record(&set_named("B"), None, t);
    h.undo(&set_named("C")).unwrap();
    h.clear();
    assert!(!h.can_undo() && !h.can_redo());
}

#[test]
fn restore_keeping_mixer_keeps_current_values_but_snapshot_names() {
    let mut snapshot = Set::new();
    snapshot.mixer.groups[0].name = "Kicks".into();
    snapshot.mixer.groups[0].volume_db = -12.0;
    snapshot.mixer.master_db = -6.0;
    snapshot.banks[0].name = "Old".into();
    let mut current = Set::new();
    current.mixer.groups[0].name = "Renamed".into();
    current.mixer.groups[0].volume_db = 3.0;
    current.mixer.groups[0].pan = 0.5;
    current.mixer.groups[0].mute = true;
    current.mixer.master_db = 1.5;
    current.banks[0].name = "New".into();

    let restored = restore_keeping_mixer(snapshot, &current);
    assert_eq!(restored.banks[0].name, "Old");
    assert_eq!(restored.mixer.groups[0].name, "Kicks");
    assert_eq!(restored.mixer.groups[0].volume_db, 3.0);
    assert_eq!(restored.mixer.groups[0].pan, 0.5);
    assert!(restored.mixer.groups[0].mute);
    assert_eq!(restored.mixer.master_db, 1.5);
}

#[test]
fn undo_keeps_current_mixer_values() {
    let mut h = History::new();
    let before = Set::new();
    h.record(&before, None, Instant::now());
    let mut current = set_named("after");
    current.mixer.groups[2].volume_db = -9.0;
    let undone = h.undo(&current).unwrap();
    assert_eq!(undone.banks[0].name, "Bank 1");
    assert_eq!(undone.mixer.groups[2].volume_db, -9.0);
}
