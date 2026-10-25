use lpd_core::instrument::{gm_program, KeysSettings, KeysSound, SoundFontKeys};
const ALL: [KeysSound; 6] = [
    KeysSound::Piano,
    KeysSound::EPiano,
    KeysSound::Organ,
    KeysSound::SynthPad,
    KeysSound::SynthLead,
    KeysSound::SynthBass,
];

#[test]
fn gm_programs() {
    assert_eq!(
        ALL.map(gm_program),
        [0, 4, 16, 89, 81, 38],
        "piano, e-piano, organ, pad, lead, bass"
    );
}

#[test]
fn settings_default_and_json() {
    use lpd_core::instrument::KeysMix;
    use lpd_core::synth::patch::PatchSource;
    let s = KeysSettings::default();
    assert_eq!(
        (s.gm_sound, s.volume_db, s.output, s.modified),
        (KeysSound::EPiano, -6.0, None, false)
    );
    assert_eq!(s.patch.name, "E.PIANO");
    assert_eq!(s.patch_source, PatchSource::Factory("ePiano".into()));
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["gmSound"], "ePiano");
    assert_eq!(
        json["patchSource"],
        serde_json::json!({ "factory": "ePiano" })
    );
    assert_eq!(json["patch"]["name"], "E.PIANO");
    let mix: KeysMix =
        serde_json::from_str(r#"{"gmSound":"organ","volumeDb":-3,"output":7}"#).unwrap();
    assert_eq!(
        mix,
        KeysMix {
            gm_sound: KeysSound::Organ,
            volume_db: -3.0,
            output: Some(7)
        }
    );
}

#[test]
fn a_missing_soundfont_is_an_error() {
    assert!(SoundFontKeys::load(std::path::Path::new("does-not-exist.sf2"), 48_000).is_err());
}
