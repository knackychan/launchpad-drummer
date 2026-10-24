use lpd_core::instrument::Instrument;
use lpd_core::kit::Set;
use lpd_core::synth::factory::{all, factory, FACTORY_IDS};
use lpd_core::synth::params::*;
use lpd_core::synth::patch::*;
use lpd_core::synth::Synth;

#[test]
fn patches_round_trip_through_json() {
    let p = factory("moogBass").unwrap();
    assert_eq!(Patch::from_json(&p.to_json()).unwrap(), p);
}

#[test]
fn missing_fields_take_defaults_and_names_are_cleaned() {
    let p = Patch::from_json(r#"{"name":"  my lead  "}"#).unwrap();
    assert_eq!(
        (p.name.as_str(), p.category, p.patch_version),
        ("MY LEAD", Category::Keys, PATCH_VERSION)
    );
    assert_eq!(p.params, SynthParams::default());
    let long = Patch::from_json(r#"{"name":"abcdefghijklmnopqrstuvwxyz0123"}"#).unwrap();
    assert_eq!(long.name, "ABCDEFGHIJKLMNOPQRSTUVWX");
    assert_eq!(clean_name("   "), "UNTITLED");
}

#[test]
fn values_out_of_range_are_clamped() {
    let p =
        Patch::from_json(r#"{"params":{"filter":{"cutoff":99999},"oscA":{"unison":12,"pan":-3}}}"#)
            .unwrap();
    assert_eq!(
        (
            p.params.filter.cutoff,
            p.params.osc_a.unison,
            p.params.osc_a.pan
        ),
        (20_000.0, 7, -1.0)
    );
}

#[test]
fn a_newer_patch_is_refused() {
    let err = Patch::from_json(r#"{"patchVersion":2,"name":"X"}"#).unwrap_err();
    assert!(err.to_string().contains("newer version"), "{err}");
    assert!(Patch::from_json("not json").is_err());
}

#[test]
fn file_names_are_safe_on_windows() {
    assert_eq!(file_name("a/b:c*"), "A_B_C_.lbpatch");
    assert_eq!(file_name("con"), "CON_.lbpatch");
    assert_eq!(file_name("lpt1"), "LPT1_.lbpatch");
    assert_eq!(file_name(""), "UNTITLED.lbpatch");
    assert_eq!(file_name("dots..."), "DOTS.lbpatch");
    assert_eq!(file_name("Moog Bass 2"), "MOOG BASS 2.lbpatch");
}

#[test]
fn every_factory_patch_exists_with_its_name_and_category() {
    let want = [
        ("piano", "PIANO", Category::Piano),
        ("ePiano", "E.PIANO", Category::Keys),
        ("organ", "ORGAN", Category::Organ),
        ("synthPad", "SYNTH PAD", Category::Pad),
        ("synthLead", "SYNTH LEAD", Category::Lead),
        ("synthBass", "SYNTH BASS", Category::Bass),
        ("moogBass", "MOOG BASS", Category::Bass),
        ("acidBass", "ACID BASS", Category::Bass),
        ("wobbleBass", "WOBBLE BASS", Category::Bass),
        ("supersaw", "SUPERSAW", Category::Lead),
        ("syncLead", "SYNC LEAD", Category::Lead),
        ("pluck", "PLUCK", Category::Pluck),
        ("brass", "BRASS", Category::Lead),
        ("strings", "STRINGS", Category::Pad),
        ("vowelPad", "VOWEL PAD", Category::Pad),
        ("fmBell", "FM BELL", Category::Keys),
    ];
    assert_eq!(FACTORY_IDS.len(), 16);
    assert_eq!(all().len(), 16);
    for (id, name, category) in want {
        let p = factory(id).unwrap();
        assert_eq!((p.name.as_str(), p.category), (name, category), "{id}");
    }
    assert!(factory("nope").is_none());
}

#[test]
fn every_factory_patch_keeps_the_wheel_vibrato() {
    for p in all() {
        let ok = p.params.routes.iter().any(|r| {
            matches!(r.source, Source::Lfo1 | Source::Lfo2)
                && r.dest == Some(Dest::OscPitch)
                && (r.amount - 0.021).abs() < 1e-6
                && r.aux == Some(Source::Wheel)
        });
        assert!(ok, "{} has no wheel vibrato", p.name);
    }
}

#[test]
fn every_factory_patch_renders_a_finite_chord_below_full_scale() {
    for p in all() {
        let mut s = Synth::new(48_000);
        s.load_params(&p.params);
        let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
        s.render_add(&mut l, &mut r);
        for n in [60, 64, 67] {
            s.note_on(n, 110);
        }
        let (mut l, mut r) = (vec![0.0f32; 48_000], vec![0.0f32; 48_000]);
        for (cl, cr) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
            s.render_add(cl, cr);
        }
        assert!(l.iter().chain(&r).all(|v| v.is_finite()), "{}", p.name);
        let peak = l.iter().chain(&r).fold(0.0f32, |m, v| m.max(v.abs()));
        let rms = (l.iter().map(|v| v * v).sum::<f32>() / l.len() as f32).sqrt();
        assert!(peak < 1.0, "{} peaks at {peak}", p.name);
        assert!(rms > 1e-3, "{} is silent", p.name);
    }
}

#[test]
fn version_1_sets_migrate_to_version_2() {
    let mut v: serde_json::Value = serde_json::from_str(&Set::new().to_json()).unwrap();
    v["version"] = serde_json::json!(1);
    v["keys"] = serde_json::json!({ "sound": "organ", "volumeDb": -3.0, "output": 2 });
    let set = Set::from_json(&v.to_string()).unwrap();
    assert_eq!(set.version, 2);
    assert_eq!(set.keys.gm_sound, lpd_core::instrument::KeysSound::Organ);
    assert_eq!(set.keys.patch.name, "ORGAN");
    assert_eq!(set.keys.patch_source, PatchSource::Factory("organ".into()));
    assert_eq!(
        (set.keys.volume_db, set.keys.output, set.keys.modified),
        (-3.0, Some(2), false)
    );
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(lpd_core::kit::SET_FILE), v.to_string()).unwrap();
    let loaded = lpd_core::kit::load(dir.path()).unwrap();
    lpd_core::kit::save(dir.path(), &loaded.set).unwrap();
    assert_eq!(lpd_core::kit::load(dir.path()).unwrap().set, set);
    let saved: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(lpd_core::kit::SET_FILE)).unwrap(),
    )
    .unwrap();
    assert_eq!(saved["version"], 2);
    let backup: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(lpd_core::kit::BACKUP_FILE)).unwrap(),
    )
    .unwrap();
    assert_eq!(backup["version"], 1);
    v.as_object_mut().unwrap().remove("keys");
    let bare = Set::from_json(&v.to_string()).unwrap();
    assert_eq!(bare.keys.patch.name, "E.PIANO");
    assert_eq!(bare.keys.volume_db, -6.0);
}
