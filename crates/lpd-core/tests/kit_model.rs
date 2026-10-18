use lpd_core::kit::*;

#[test]
fn new_set_has_8_banks_and_8_groups_with_defaults() {
    let set = Set::new();
    assert_eq!(set.version, 1);
    assert_eq!(set.banks.len(), 8);
    assert_eq!(set.banks[0].name, "Bank 1");
    assert_eq!(set.banks[7].name, "Bank 8");
    assert!(
        set.banks[0].lights.flash && set.banks[0].lights.play_state && !set.banks[0].lights.ripple
    );
    assert!(set.banks[0].pads.is_empty());
    assert_eq!(set.mixer.groups.len(), 8);
    assert_eq!(set.mixer.groups[0].lowpass_hz, 20_000.0);
}

#[test]
fn new_pad_defaults_mix_group_to_row() {
    let pad = Pad::new(2, 5, "samples/a.wav", [1, 2, 3]);
    assert_eq!(pad.mix_group, 5);
    assert_eq!(pad.mode, PadMode::OneShot);
    assert_eq!(pad.choke_group, None);
}

#[test]
fn json_round_trip() {
    let mut set = Set::new();
    let mut pad = Pad::new(0, 0, "samples/kick.wav", [63, 10, 0]);
    pad.mode = PadMode::Hold;
    pad.choke_group = Some(2);
    pad.trim_end_ms = Some(120.0);
    set.banks[0].set_pad(pad);
    let back = Set::from_json(&set.to_json()).unwrap();
    assert_eq!(back, set);
}

#[test]
fn json_uses_spec_field_names() {
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "samples/kick.wav", [63, 10, 0]));
    let json = set.to_json();
    for key in [
        "\"volumeDb\"",
        "\"pitchSemitones\"",
        "\"trimStartMs\"",
        "\"chokeGroup\"",
        "\"mixGroup\"",
        "\"oneShot\"",
        "\"playState\"",
        "\"humanizeDb\"",
        "\"lowpassHz\"",
        "\"masterDb\"",
    ] {
        assert!(json.contains(key), "missing {key} in {json}");
    }
}

#[test]
fn rejects_wrong_version_and_shape() {
    let mut set = Set::new();
    set.version = 2;
    assert!(Set::from_json(&set.to_json()).is_err());
    let mut set = Set::new();
    set.banks.pop();
    assert!(Set::from_json(&set.to_json()).is_err());
    assert!(Set::from_json("not json").is_err());
}

#[test]
fn set_pad_replaces_pad_at_same_position() {
    let mut bank = Set::new().banks.remove(0);
    bank.set_pad(Pad::new(1, 1, "samples/a.wav", [0, 0, 0]));
    bank.set_pad(Pad::new(1, 1, "samples/b.wav", [0, 0, 0]));
    assert_eq!(bank.pads.len(), 1);
    assert_eq!(bank.pad(1, 1).unwrap().sample, "samples/b.wav");
    assert!(bank.pad(0, 0).is_none());
}
