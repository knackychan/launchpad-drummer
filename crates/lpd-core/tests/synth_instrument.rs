use lpd_core::instrument::Instrument;
use lpd_core::synth::params::*;
use lpd_core::synth::Synth;

const RATE: u32 = 48_000;

fn render(s: &mut Synth, seconds: f32) -> (Vec<f32>, Vec<f32>) {
    let n = (seconds * RATE as f32) as usize;
    let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
    for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        s.render_add(cl, cr);
    }
    (l, r)
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

/// A synth with `f` applied to INIT, after the 5 ms load fade.
fn with(f: impl FnOnce(&mut SynthParams)) -> Synth {
    let mut p = SynthParams::default();
    f(&mut p);
    let mut s = Synth::new(RATE);
    s.load_params(&p);
    render(&mut s, 0.01);
    assert_eq!(*s.params(), p);
    s
}

#[test]
fn a_note_sounds_and_ends_after_its_release() {
    let mut s = Synth::new(RATE);
    s.note_on(60, 100);
    assert!(peak(&render(&mut s, 0.2).0) > 0.01);
    s.note_off(60);
    render(&mut s, 0.5);
    assert_eq!(s.active_voices(), 0);
}

#[test]
fn the_oldest_released_voice_is_stolen_first() {
    let mut s = Synth::new(RATE);
    for n in 60..=75 {
        s.note_on(n, 100);
    }
    render(&mut s, 0.05);
    s.note_off(60);
    s.note_off(61);
    render(&mut s, 0.01);
    s.note_on(76, 100);
    assert!(!s.is_sounding(60));
    assert!(s.is_sounding(61));
    assert!(s.is_sounding(76));
    assert_eq!(s.active_voices(), 16);
}

#[test]
fn without_released_voices_the_oldest_held_is_stolen() {
    let mut s = Synth::new(RATE);
    for n in 60..=76 {
        s.note_on(n, 100);
    }
    assert!(!s.is_sounding(60));
    assert!(s.is_sounding(61) && s.is_sounding(76));
}

#[test]
fn a_held_note_retriggers_its_own_voice() {
    let mut s = Synth::new(RATE);
    s.note_on(60, 100);
    render(&mut s, 0.01);
    s.note_on(60, 90);
    assert_eq!(s.active_voices(), 1);
}

#[test]
fn mono_returns_to_the_previous_note() {
    let mut s = with(|p| p.voice.mode = VoiceMode::Mono);
    s.note_on(60, 100);
    s.note_on(64, 100);
    render(&mut s, 0.01);
    assert_eq!(s.sounding_notes(), vec![64]);
    s.note_off(64);
    render(&mut s, 0.01);
    assert_eq!(s.sounding_notes(), vec![60]);
    s.note_off(60);
    render(&mut s, 0.5);
    assert!(s.sounding_notes().is_empty());
}

#[test]
fn legato_keeps_the_envelope_running() {
    let mut s = with(|p| {
        p.voice.mode = VoiceMode::Legato;
        p.env[0].attack = 0.5;
    });
    s.note_on(60, 100);
    render(&mut s, 0.1);
    let before = s.monitor().unwrap().env[0][0];
    s.note_on(62, 100);
    render(&mut s, 0.01);
    let m = s.monitor().unwrap();
    assert!(m.env[0][0] > before, "{} after {}", m.env[0][0], before);
    assert_eq!(m.env[0][1], 1.0, "still in the attack");
}

#[test]
fn glide_takes_its_time() {
    let mut s = with(|p| {
        p.voice.mode = VoiceMode::Mono;
        p.voice.glide = 0.1;
    });
    s.note_on(60, 100);
    render(&mut s, 0.2);
    s.note_on(72, 100);
    render(&mut s, 0.05);
    let mid = s.voice_pitch(72).unwrap();
    assert!((mid - 66.0).abs() < 0.6, "{mid}");
    render(&mut s, 0.06);
    assert!((s.voice_pitch(72).unwrap() - 72.0).abs() < 0.01);
}

#[test]
fn the_sustain_pedal_holds_released_notes() {
    let mut s = Synth::new(RATE);
    s.sustain(true);
    s.note_on(60, 100);
    s.note_off(60);
    render(&mut s, 1.0);
    assert!(s.is_sounding(60));
    s.sustain(false);
    render(&mut s, 0.5);
    assert!(!s.is_sounding(60));
}

#[test]
fn all_notes_off_ends_every_voice_quickly() {
    let mut s = with(|p| p.env[0].release = 10.0);
    s.note_on(60, 100);
    s.note_on(64, 100);
    render(&mut s, 0.05);
    s.all_notes_off();
    render(&mut s, 0.02);
    assert_eq!(s.active_voices(), 0);
}

#[test]
fn a_patch_load_fades_out_then_applies() {
    let mut s = Synth::new(RATE);
    s.note_on(60, 127);
    render(&mut s, 0.2);
    let p = SynthParams {
        filter: FilterParams {
            cutoff: 300.0,
            ..FilterParams::default()
        },
        ..SynthParams::default()
    };
    s.load_params(&p);
    assert_eq!(
        s.params().filter.cutoff,
        12_000.0,
        "pending until the fade is over"
    );
    let (l, _) = render(&mut s, 0.01);
    assert_eq!(s.params().filter.cutoff, 300.0);
    assert_eq!(s.active_voices(), 0);
    assert_eq!(peak(&l[l.len() - 96..]), 0.0, "silent after the fade");
}

#[test]
fn notes_played_during_the_fade_start_after_it() {
    let mut s = Synth::new(RATE);
    s.load_params(&SynthParams::default());
    s.note_on(64, 100);
    render(&mut s, 0.02);
    assert!(s.is_sounding(64));
}

#[test]
fn an_unstable_voice_is_reset() {
    let mut s = Synth::new(RATE);
    s.note_on(60, 100);
    render(&mut s, 0.01);
    s.debug_poison_voice(0);
    let (l, r) = render(&mut s, 0.01);
    assert!(l.iter().chain(&r).all(|v| v.is_finite()));
    assert_eq!(s.monitor().unwrap().resets, 1);
    assert_eq!(s.active_voices(), 0);
}

#[test]
fn a_loud_chord_stays_finite_and_below_full_scale() {
    let mut s = Synth::new(RATE);
    for n in [48, 60, 64, 67, 72] {
        s.note_on(n, 127);
    }
    let (l, r) = render(&mut s, 1.0);
    assert!(l.iter().chain(&r).all(|v| v.is_finite()));
    assert!(peak(&l).max(peak(&r)) < 1.0);
}

#[test]
fn the_monitor_follows_the_newest_voice() {
    let mut s = Synth::new(RATE);
    s.note_on(60, 100);
    s.note_on(72, 100);
    render(&mut s, 0.01);
    let m = s.monitor().unwrap();
    assert!((m.sources[Source::Note as usize] - 0.2).abs() < 1e-6);
    assert_eq!(m.voices, 2);
}

#[test]
fn set_param_moves_the_live_cutoff_smoothly() {
    let mut s = Synth::new(RATE);
    s.note_on(60, 100);
    render(&mut s, 0.05);
    s.set_param(ParamId::FilterCutoff, 300.0);
    render(&mut s, 0.002);
    assert!(s.monitor().unwrap().cutoff_hz > 450.0, "still on its way");
    render(&mut s, 0.06);
    let hz = s.monitor().unwrap().cutoff_hz;
    assert!((hz / 300.0 - 1.0).abs() < 0.02, "{hz}");
}

#[test]
fn routes_modulate_the_cutoff() {
    let mut s = Synth::new(RATE);
    s.set_route(
        0,
        Some(Route {
            source: Source::Lfo1,
            dest: Some(Dest::Cutoff),
            amount: 0.5,
            aux: None,
            bypass: false,
        }),
    );
    s.note_on(60, 100);
    let (mut lo, mut hi) = (f32::MAX, 0.0f32);
    for _ in 0..40 {
        render(&mut s, 0.01);
        let hz = s.monitor().unwrap().cutoff_hz;
        lo = lo.min(hz);
        hi = hi.max(hz);
    }
    assert!(lo < 1_000.0 && hi > 19_000.0, "{lo} … {hi}");
}

#[test]
fn wheel_and_bend_reach_the_sources() {
    let mut s = Synth::new(RATE);
    s.modulation(0.5);
    s.pitch_bend(-1.0);
    render(&mut s, 0.01);
    let m = s.monitor().unwrap();
    assert_eq!(m.sources[Source::Wheel as usize], 0.5);
    assert_eq!(m.sources[Source::Bend as usize], -1.0);
}
