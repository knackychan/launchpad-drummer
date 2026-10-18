use lpd_core::kit::{ops::*, Pad, PadMode, Set};
use std::path::Path;

#[test]
fn positions_follow_reading_order_and_skip_filled_pads() {
    let mut set = Set::new();
    let b = &mut set.banks[0];
    assert_eq!(drop_positions(b, 6, 0, 4), [(6, 0), (7, 0), (0, 1), (1, 1)]);
    b.set_pad(Pad::new(7, 0, "a", [0; 3]));
    b.set_pad(Pad::new(0, 1, "b", [0; 3]));
    assert_eq!(drop_positions(b, 6, 0, 3), [(6, 0), (1, 1), (2, 1)]);
    b.set_pad(Pad::new(2, 2, "c", [0; 3]));
    assert_eq!(drop_positions(b, 2, 2, 2), [(2, 2), (3, 2)]);
    assert_eq!(drop_positions(b, 7, 7, 3), [(7, 7)]);
    assert!(drop_positions(b, 8, 0, 1).is_empty());
    assert!(drop_positions(b, 0, 0, 0).is_empty());
}

#[test]
fn defaults_extensions_and_replacement() {
    for (y, color) in [
        (0, [63, 0, 0]),
        (1, [63, 20, 0]),
        (2, [63, 63, 0]),
        (7, [63, 10, 30]),
    ] {
        assert_eq!(row_default_color(y), color);
    }
    assert!(is_audio_path(Path::new("A.WAV")));
    assert!(!is_audio_path(Path::new("a.txt")));
    assert!(!is_audio_path(Path::new("noext")));
    let mut set = Set::new();
    let b = &mut set.banks[0];
    let mut p = Pad::new(1, 2, "old", [0, 0, 63]);
    p.trim_start_ms = 100.0;
    p.trim_end_ms = Some(400.0);
    p.volume_db = -6.0;
    p.pitch_semitones = 3.0;
    p.mode = PadMode::Hold;
    p.choke_group = Some(2);
    p.mix_group = 5;
    b.set_pad(p.clone());
    assign_sample(b, 1, 2, "new".into());
    p.sample = "new".into();
    p.trim_start_ms = 0.0;
    p.trim_end_ms = None;
    assert_eq!(b.pad(1, 2), Some(&p));
    assign_sample(b, 0, 7, "empty".into());
    assert_eq!(
        b.pad(0, 7),
        Some(&Pad::new(0, 7, "empty", row_default_color(7)))
    );
}

#[test]
fn moves_swaps_and_copies_rewrite_coordinates_and_cross_banks() {
    let mut s = Set::new();
    let mut expected = Pad::new(0, 0, "a", [1, 2, 3]);
    s.banks[0].set_pad(Pad::new(0, 0, "a", [1, 2, 3]));
    assert_eq!(
        move_pad(&mut s, (0, 0, 0), (0, 1, 1), false),
        Some(PadMove::Move)
    );
    assert!(s.banks[0].pad(0, 0).is_none());
    expected.x = 1;
    expected.y = 1;
    assert_eq!(s.banks[0].pad(1, 1).unwrap(), &expected);
    s.banks[1].set_pad(Pad::new(2, 2, "b", [3, 2, 1]));
    assert_eq!(
        move_pad(&mut s, (0, 1, 1), (1, 2, 2), false),
        Some(PadMove::Swap)
    );
    assert_eq!(s.banks[0].pad(1, 1).unwrap().sample, "b");
    expected.x = 2;
    expected.y = 2;
    assert_eq!(s.banks[1].pad(2, 2).unwrap(), &expected);
    assert_eq!(
        move_pad(&mut s, (1, 2, 2), (0, 1, 1), true),
        Some(PadMove::Copy)
    );
    expected.x = 1;
    expected.y = 1;
    assert_eq!(s.banks[0].pad(1, 1).unwrap(), &expected);
    assert!(s.banks[1].pad(2, 2).is_some());
    assert_eq!(move_pad(&mut s, (0, 7, 7), (0, 1, 1), false), None);
    assert_eq!(move_pad(&mut s, (0, 1, 1), (0, 1, 1), false), None);
    assert_eq!(
        move_pad(&mut s, (0, 1, 1), (1, 7, 7), false),
        Some(PadMove::Move)
    );
    assert!(clear_pad(&mut s, 1, 7, 7));
    assert!(!clear_pad(&mut s, 1, 7, 7));
}
