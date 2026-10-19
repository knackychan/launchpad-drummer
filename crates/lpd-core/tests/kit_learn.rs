use lpd_core::kit::learn::*;
use lpd_core::kit::ops::{clear_pad, move_pad};
use lpd_core::kit::{Pad, Set};

fn cc(device: &str, number: u8, id: &str) -> Binding {
    Binding {
        device: device.into(),
        kind: LearnKind::Cc,
        number,
        target: Target::Param(id.into()),
    }
}

fn note(device: &str, number: u8, bank: u8, x: u8, y: u8) -> Binding {
    Binding {
        device: device.into(),
        kind: LearnKind::Note,
        number,
        target: Target::Pad { bank, x, y },
    }
}

#[test]
fn targets_serialize_as_in_the_plan() {
    let pad = serde_json::to_string(&Target::Pad {
        bank: 0,
        x: 0,
        y: 0,
    })
    .unwrap();
    assert_eq!(pad, r#"{"pad":{"bank":0,"x":0,"y":0}}"#);
    let param = serde_json::to_string(&Target::Param("group.2.volume".into())).unwrap();
    assert_eq!(param, r#"{"param":"group.2.volume"}"#);
    let back: Target = serde_json::from_str(&param).unwrap();
    assert_eq!(back, Target::Param("group.2.volume".into()));
    let b = serde_json::to_value(cc("Keystation 49", 21, "master")).unwrap();
    assert_eq!(b["kind"], "cc");
    assert_eq!(b["device"], "Keystation 49");
    assert_eq!(b["number"], 21);
}

#[test]
fn an_old_set_without_learn_loads_with_none() {
    let mut value = serde_json::to_value(Set::new()).unwrap();
    value.as_object_mut().unwrap().remove("learn");
    let set = Set::from_json(&value.to_string()).unwrap();
    assert!(set.learn.is_empty());
}

#[test]
fn bindings_round_trip_through_the_set_file() {
    let mut set = Set::new();
    set.learn.push(note("K", 36, 1, 2, 3));
    set.learn.push(cc("K", 7, "group.0.volume"));
    let back = Set::from_json(&set.to_json()).unwrap();
    assert_eq!(back.learn, set.learn);
}

#[test]
fn bind_moves_a_reused_source_to_the_new_target() {
    let mut learn = vec![cc("K", 7, "group.0.volume")];
    bind(&mut learn, cc("K", 7, "group.1.volume"));
    assert_eq!(learn, vec![cc("K", 7, "group.1.volume")]);
}

#[test]
fn bind_replaces_the_old_source_of_a_bound_target() {
    let mut learn = vec![cc("K", 7, "group.0.volume")];
    bind(&mut learn, cc("K", 8, "group.0.volume"));
    assert_eq!(learn, vec![cc("K", 8, "group.0.volume")]);
    // A note and a CC with the same number are different sources.
    bind(&mut learn, note("K", 8, 0, 0, 0));
    assert_eq!(learn.len(), 2);
    // The same number on another device is a different source.
    bind(&mut learn, cc("Other", 8, "master"));
    assert_eq!(learn.len(), 3);
}

#[test]
fn unbind_removes_by_index() {
    let mut learn = vec![cc("K", 1, "master"), cc("K", 2, "stopAll")];
    assert!(unbind(&mut learn, 0));
    assert_eq!(learn, vec![cc("K", 2, "stopAll")]);
    assert!(!unbind(&mut learn, 5));
}

#[test]
fn param_ids() {
    for id in [
        "group.0.volume",
        "group.7.pan",
        "group.3.pitch",
        "group.3.lowpass",
        "group.3.mute",
        "group.3.solo",
        "master",
        "bank.7.humanize",
        "bank.0.flash",
        "bank.0.playState",
        "bank.0.ripple",
        "keys.volume",
        "keys.mod",
        "seq.play",
        "seq.bpm",
        "seq.swing",
        "stopAll",
    ] {
        assert!(is_param(id), "{id}");
    }
    for id in [
        "group.8.volume",
        "group.0.flash",
        "bank.8.humanize",
        "bank.0.volume",
        "keys.pan",
        "seq.stop",
        "",
        "group..volume",
        "master.x",
    ] {
        assert!(!is_param(id), "{id}");
    }
}

#[test]
fn cc_values_map_to_parameter_values() {
    assert_eq!(
        param_value("group.0.volume", 127),
        Some(ParamValue::Continuous(6.0))
    );
    assert_eq!(
        param_value("group.0.pan", 0),
        Some(ParamValue::Continuous(-1.0))
    );
    assert_eq!(
        param_value("seq.bpm", 127),
        Some(ParamValue::Continuous(200.0))
    );
    assert_eq!(
        param_value("seq.bpm", 0),
        Some(ParamValue::Continuous(60.0))
    );
    assert_eq!(
        param_value("seq.swing", 127),
        Some(ParamValue::Continuous(75.0))
    );
    assert_eq!(
        param_value("keys.volume", 0),
        Some(ParamValue::Continuous(-40.0))
    );
    assert_eq!(
        param_value("keys.volume", 127),
        Some(ParamValue::Continuous(6.0))
    );
    assert_eq!(
        param_value("keys.mod", 127),
        Some(ParamValue::Continuous(1.0))
    );
    assert_eq!(
        param_value("master", 127),
        Some(ParamValue::Continuous(6.0))
    );
    assert_eq!(
        param_value("bank.2.humanize", 127),
        Some(ParamValue::Continuous(6.0))
    );
    assert_eq!(
        param_value("bank.2.humanize", 0),
        Some(ParamValue::Continuous(0.0))
    );
    assert_eq!(param_value("group.3.mute", 100), Some(ParamValue::Toggle));
    assert_eq!(param_value("stopAll", 1), Some(ParamValue::Toggle));
    assert_eq!(param_value("seq.play", 1), Some(ParamValue::Toggle));
    assert_eq!(param_value("bank.1.ripple", 1), Some(ParamValue::Toggle));
    assert_eq!(param_value("group.8.volume", 5), None);
}

#[test]
fn humanize_steps_by_half_a_decibel() {
    for v in 0..=127u8 {
        let Some(ParamValue::Continuous(db)) = param_value("bank.0.humanize", v) else {
            panic!()
        };
        assert!((0.0..=6.0).contains(&db));
        assert_eq!((db * 2.0).fract(), 0.0, "{db}");
    }
}

#[test]
fn mixer_params_apply_to_the_mixer() {
    let mut set = Set::new();
    let m = &mut set.mixer;
    assert!(apply_mixer_param(
        m,
        "group.2.volume",
        ParamValue::Continuous(-6.0)
    ));
    assert_eq!(m.groups[2].volume_db, -6.0);
    assert!(!apply_mixer_param(
        m,
        "group.2.volume",
        ParamValue::Continuous(-6.0)
    ));
    assert!(apply_mixer_param(m, "group.2.mute", ParamValue::Toggle));
    assert!(m.groups[2].mute);
    assert!(apply_mixer_param(m, "group.2.mute", ParamValue::Toggle));
    assert!(!m.groups[2].mute);
    assert!(apply_mixer_param(m, "group.2.solo", ParamValue::Toggle));
    assert!(apply_mixer_param(m, "master", ParamValue::Continuous(3.0)));
    assert_eq!(m.master_db, 3.0);
    assert!(apply_mixer_param(
        m,
        "group.4.lowpass",
        ParamValue::Continuous(1000.0)
    ));
    assert!(apply_mixer_param(
        m,
        "group.4.pitch",
        ParamValue::Continuous(-3.0)
    ));
    assert!(apply_mixer_param(
        m,
        "group.4.pan",
        ParamValue::Continuous(0.5)
    ));
    assert_eq!(
        (
            m.groups[4].lowpass_hz,
            m.groups[4].pitch_semitones,
            m.groups[4].pan
        ),
        (1000.0, -3.0, 0.5)
    );
    // Not a mixer parameter.
    assert!(!apply_mixer_param(
        m,
        "bank.0.humanize",
        ParamValue::Continuous(1.0)
    ));
}

#[test]
fn bank_params_apply_to_the_set() {
    let mut set = Set::new();
    assert!(apply_set_param(
        &mut set,
        "bank.3.humanize",
        ParamValue::Continuous(2.5)
    ));
    assert_eq!(set.banks[3].humanize_db, 2.5);
    let flash = set.banks[3].lights.flash;
    assert!(apply_set_param(
        &mut set,
        "bank.3.flash",
        ParamValue::Toggle
    ));
    assert_eq!(set.banks[3].lights.flash, !flash);
    let play = set.banks[3].lights.play_state;
    assert!(apply_set_param(
        &mut set,
        "bank.3.playState",
        ParamValue::Toggle
    ));
    assert_eq!(set.banks[3].lights.play_state, !play);
    assert!(apply_set_param(
        &mut set,
        "bank.3.ripple",
        ParamValue::Toggle
    ));
    assert!(set.banks[3].lights.ripple);
    assert!(!apply_set_param(
        &mut set,
        "group.0.mute",
        ParamValue::Toggle
    ));
}

#[test]
fn sequencer_params_apply_to_the_pattern() {
    let mut set = Set::new();
    assert!(apply_set_param(
        &mut set,
        "seq.bpm",
        ParamValue::Continuous(140.0)
    ));
    assert_eq!(set.pattern.bpm, 140.0);
    assert!(apply_set_param(
        &mut set,
        "seq.swing",
        ParamValue::Continuous(60.0)
    ));
    assert_eq!(set.pattern.swing, 60.0);
    assert!(!apply_set_param(
        &mut set,
        "seq.bpm",
        ParamValue::Continuous(140.0)
    ));
    assert!(!apply_set_param(&mut set, "seq.play", ParamValue::Toggle));
}

fn set_with_pads() -> Set {
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "samples/a.wav", [63, 0, 0]));
    set.banks[0].set_pad(Pad::new(1, 0, "samples/b.wav", [0, 63, 0]));
    set.banks[1].set_pad(Pad::new(0, 0, "samples/c.wav", [0, 0, 63]));
    set
}

#[test]
fn moving_a_pad_moves_its_binding() {
    let mut set = set_with_pads();
    set.learn.push(note("K", 36, 0, 0, 0));
    move_pad(&mut set, (0, 0, 0), (0, 5, 5), false).unwrap();
    assert_eq!(set.learn, vec![note("K", 36, 0, 5, 5)]);
}

#[test]
fn moving_onto_a_bound_empty_pad_keeps_one_binding_per_target() {
    let mut set = set_with_pads();
    set.learn.push(note("K", 36, 0, 0, 0));
    set.learn.push(note("K", 37, 0, 5, 5)); // bound, but the pad has no sample
    move_pad(&mut set, (0, 0, 0), (0, 5, 5), false).unwrap();
    assert_eq!(set.learn, vec![note("K", 36, 0, 5, 5)]);
}

#[test]
fn swapping_exchanges_the_bindings() {
    let mut set = set_with_pads();
    set.learn.push(note("K", 36, 0, 0, 0));
    set.learn.push(note("K", 37, 0, 1, 0));
    move_pad(&mut set, (0, 0, 0), (0, 1, 0), false).unwrap();
    assert!(set.learn.contains(&note("K", 36, 0, 1, 0)));
    assert!(set.learn.contains(&note("K", 37, 0, 0, 0)));
    assert_eq!(set.learn.len(), 2);
}

#[test]
fn a_cross_bank_swap_exchanges_the_bindings_too() {
    let mut set = set_with_pads();
    set.learn.push(note("K", 36, 0, 0, 0));
    set.learn.push(note("K", 37, 1, 0, 0));
    move_pad(&mut set, (0, 0, 0), (1, 0, 0), false).unwrap();
    assert!(set.learn.contains(&note("K", 36, 1, 0, 0)));
    assert!(set.learn.contains(&note("K", 37, 0, 0, 0)));
}

#[test]
fn copying_leaves_one_binding() {
    let mut set = set_with_pads();
    set.learn.push(note("K", 36, 0, 0, 0));
    move_pad(&mut set, (0, 0, 0), (0, 5, 5), true).unwrap();
    assert_eq!(set.learn, vec![note("K", 36, 0, 0, 0)]);
}

#[test]
fn clearing_a_pad_removes_its_binding() {
    let mut set = set_with_pads();
    set.learn.push(note("K", 36, 0, 0, 0));
    set.learn.push(cc("K", 7, "master"));
    assert!(clear_pad(&mut set, 0, 0, 0));
    assert_eq!(set.learn, vec![cc("K", 7, "master")]);
}

#[test]
fn retarget_pad_moves_or_removes() {
    let mut learn = vec![note("K", 36, 0, 0, 0), cc("K", 7, "master")];
    retarget_pad(&mut learn, (0, 0, 0), Some((2, 3, 4)));
    assert_eq!(learn[0], note("K", 36, 2, 3, 4));
    retarget_pad(&mut learn, (2, 3, 4), None);
    assert_eq!(learn, vec![cc("K", 7, "master")]);
}
