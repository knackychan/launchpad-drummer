use lpd_core::audio::MAX_BLOCK;
use lpd_core::instrument::{
    gm_program, BasicKeys, Instrument, KeysSettings, KeysSound, SoundFontKeys,
};

const RATE: u32 = 48_000;
const ALL: [KeysSound; 6] = [
    KeysSound::Piano,
    KeysSound::EPiano,
    KeysSound::Organ,
    KeysSound::SynthPad,
    KeysSound::SynthLead,
    KeysSound::SynthBass,
];

fn ms(ms: u32) -> usize {
    (ms as usize * RATE as usize) / 1000
}

/// Render `frames` mono frames (left channel) in blocks the engine would use.
fn render(k: &mut dyn Instrument, frames: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(frames);
    while out.len() < frames {
        let n = (frames - out.len()).min(MAX_BLOCK);
        let mut l = vec![0.0; n];
        let mut r = vec![0.0; n];
        k.render_add(&mut l, &mut r);
        assert_eq!(l, r, "the same signal on both sides");
        out.extend_from_slice(&l);
    }
    out
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

fn keys(sound: KeysSound) -> BasicKeys {
    let mut k = BasicKeys::new(RATE);
    k.set_sound(sound);
    k
}

#[test]
fn every_sound_is_audible_from_the_first_50_ms() {
    for sound in ALL {
        let mut k = keys(sound);
        k.note_on(60, 100);
        let first = render(&mut k, ms(50));
        assert!(rms(&first) > 0.01, "{sound:?}: {}", rms(&first));
        assert!(
            first.iter().all(|v| v.is_finite() && v.abs() < 1.0),
            "{sound:?}"
        );
    }
}

#[test]
fn piano_decays_while_held() {
    let mut k = keys(KeysSound::Piano);
    k.note_on(60, 100);
    let all = render(&mut k, ms(2000));
    let first = rms(&all[..ms(50)]);
    let last = rms(&all[all.len() - ms(50)..]);
    assert!(last < first * 0.1, "first {first}, last {last}");
}

#[test]
fn organ_stops_at_once_on_note_off() {
    let mut k = keys(KeysSound::Organ);
    k.note_on(60, 100);
    let held = render(&mut k, ms(100));
    let level = rms(&held[held.len() - ms(50)..]);
    assert!(level > 0.05);
    k.note_off(60);
    let after = render(&mut k, ms(20));
    assert!(
        rms(&after[ms(6)..ms(16)]) < level * 0.01,
        "{}",
        rms(&after[ms(6)..ms(16)])
    );
}

#[test]
fn synth_pad_rings_on_after_note_off() {
    let mut k = keys(KeysSound::SynthPad);
    k.note_on(60, 100);
    let held = render(&mut k, ms(1500));
    let level = rms(&held[held.len() - ms(50)..]);
    k.note_off(60);
    let after = render(&mut k, ms(250));
    let ringing = rms(&after[ms(200)..ms(250)]);
    assert!(
        ringing > level * 0.1,
        "held {level}, 200 ms later {ringing}"
    );
}

#[test]
fn every_sound_ends_after_its_release() {
    for sound in ALL {
        let mut k = keys(sound);
        k.note_on(60, 100);
        render(&mut k, ms(100));
        k.note_off(60);
        render(&mut k, ms(1500));
        assert_eq!(k.active_voices(), 0, "{sound:?}");
    }
}

#[test]
fn the_17th_note_steals_the_oldest_voice() {
    let mut k = keys(KeysSound::Organ);
    for note in 60..77 {
        k.note_on(note, 100);
        render(&mut k, 64);
    }
    assert!(k.active_voices() <= 16);
    assert!(!k.is_sounding(60), "note 60 lost its voice");
    assert!(k.is_sounding(76));
    assert!(k.is_sounding(61));
}

#[test]
fn hitting_a_held_note_again_reuses_its_voice() {
    let mut k = keys(KeysSound::Organ);
    k.note_on(60, 100);
    k.note_on(64, 100);
    k.note_on(60, 90);
    assert_eq!(k.active_voices(), 2);
}

fn crossings(x: &[f32]) -> usize {
    x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count()
}

#[test]
fn pitch_bend_plus_two_semitones_raises_the_pitch() {
    let mut k = keys(KeysSound::Organ);
    k.note_on(60, 100);
    render(&mut k, ms(100));
    let before = crossings(&render(&mut k, ms(1000)));
    k.pitch_bend(2.0);
    render(&mut k, ms(20));
    let after = crossings(&render(&mut k, ms(1000)));
    let ratio = after as f32 / before as f32;
    assert!(
        (ratio - 1.1225).abs() < 0.012,
        "ratio {ratio} ({before} → {after})"
    );
}

#[test]
fn modulation_adds_vibrato() {
    let mut k = keys(KeysSound::Organ);
    k.note_on(60, 100);
    render(&mut k, ms(100));
    let plain = crossings(&render(&mut k, ms(1000)));
    k.modulation(1.0);
    let wobble = crossings(&render(&mut k, ms(1000)));
    // The vibrato is centred: the average pitch stays within a fraction of a percent.
    assert!(
        (wobble as f32 / plain as f32 - 1.0).abs() < 0.02,
        "{plain} vs {wobble}"
    );
}

#[test]
fn velocity_scales_the_level() {
    let level = |vel| {
        let mut k = keys(KeysSound::Organ);
        k.note_on(60, vel);
        let x = render(&mut k, ms(100));
        rms(&x[ms(50)..])
    };
    assert!(level(127) > level(30) * 4.0);
}

#[test]
fn all_notes_off_is_fast() {
    let mut k = keys(KeysSound::SynthPad);
    k.note_on(60, 100);
    k.note_on(64, 100);
    render(&mut k, ms(1000));
    k.all_notes_off();
    let tail = render(&mut k, ms(20));
    assert!(rms(&tail[ms(8)..]) < 1e-4, "{}", rms(&tail[ms(8)..]));
    assert_eq!(k.active_voices(), 0);
}

#[test]
fn render_adds_into_the_buffers() {
    let mut k = keys(KeysSound::Organ);
    k.note_on(60, 100);
    let mut l = vec![1.0; 256];
    let mut r = vec![1.0; 256];
    k.render_add(&mut l, &mut r);
    assert!(l.iter().any(|v| *v != 1.0));
}

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
