use lpd_core::audio::*;
use std::sync::Arc;

#[test]
fn audition_bypasses_groups_but_obeys_master_and_emits_no_pad_events() {
    let (mut engine, mut events) = Engine::new(SR);
    for group in 0..8 {
        engine.handle(Command::SetGroup {
            group,
            params: GroupParams {
                mute: true,
                ..Default::default()
            },
        });
    }
    let sample = constant(100, 0.5);
    engine.handle(Command::Audition {
        sample: sample.clone(),
    });
    assert!(render(&mut engine, 100).iter().any(|v| *v > 0.0));
    engine.handle(Command::SetMasterGain(0.0));
    engine.handle(Command::Audition {
        sample: sample.clone(),
    });
    assert!(render(&mut engine, 100).iter().all(|v| *v == 0.0));
    assert!(events.pop().is_err());
}

#[test]
fn audition_replacement_and_stop_fade_without_clicks() {
    let (mut engine, mut events) = Engine::new(SR);
    let a = constant(48000, 0.5);
    let b = constant(48000, -0.5);
    engine.handle(Command::Audition { sample: a.clone() });
    let mut out = left(&render(&mut engine, 1000));
    engine.handle(Command::Audition { sample: b.clone() });
    out.extend(left(&render(&mut engine, 1000)));
    engine.handle(Command::StopAudition);
    out.extend(left(&render(&mut engine, 1000)));
    let max_step = out
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max);
    assert!(max_step < 0.05, "step {max_step}");
    assert!(out[out.len() - 100..].iter().all(|v| *v == 0.0));
    assert!(events.pop().is_err());
}

const SR: u32 = 48_000;

fn constant(frames: usize, value: f32) -> Arc<Sample> {
    Arc::new(Sample {
        rate: SR,
        data: vec![value; frames * 2],
    })
}

fn sine(freq: f32, frames: usize, amp: f32) -> Arc<Sample> {
    let data = (0..frames)
        .flat_map(|i| {
            let x = amp * (2.0 * std::f32::consts::PI * freq * i as f32 / SR as f32).sin();
            [x, x]
        })
        .collect();
    Arc::new(Sample { rate: SR, data })
}

fn params(sample: &Sample) -> PlayParams {
    PlayParams {
        gain: 1.0,
        pitch_semitones: 0.0,
        start_frame: 0,
        end_frame: sample.frames(),
        choke_group: None,
        mix_group: 0,
        hold: false,
    }
}

fn trigger(e: &mut Engine, pad: u16, sample: &Arc<Sample>, f: impl FnOnce(&mut PlayParams)) {
    let mut p = params(sample);
    f(&mut p);
    e.handle(Command::Trigger {
        pad,
        sample: sample.clone(),
        params: p,
    });
}

fn render(e: &mut Engine, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * 2];
    e.render(&mut out);
    out
}

fn left(out: &[f32]) -> Vec<f32> {
    out.iter().step_by(2).copied().collect()
}

fn events(rx: &mut rtrb::Consumer<Event>) -> Vec<Event> {
    let mut v = Vec::new();
    while let Ok(e) = rx.pop() {
        v.push(e);
    }
    v
}

#[test]
fn trigger_plays_at_unity_and_reports_start() {
    let (mut e, mut rx) = Engine::new(SR);
    let s = constant(48_000, 0.5);
    trigger(&mut e, 1, &s, |_| {});
    let out = render(&mut e, 64);
    assert!(out.iter().all(|&x| (x - 0.5).abs() < 1e-4));
    assert_eq!(events(&mut rx), vec![Event::VoiceStarted { pad: 1 }]);
}

#[test]
fn choke_silences_same_group_within_5ms() {
    let (mut e, mut rx) = Engine::new(SR);
    let s = constant(48_000, 0.5);
    let silent = constant(48_000, 0.0);
    trigger(&mut e, 1, &s, |p| p.choke_group = Some(1));
    trigger(&mut e, 3, &s, |p| p.choke_group = Some(2));
    render(&mut e, 100);
    trigger(&mut e, 2, &silent, |p| p.choke_group = Some(1));
    let out = left(&render(&mut e, 480));
    // pad 3 (other choke group) keeps playing at 0.5; pad 1 is gone after 240 frames
    assert!(
        out[245..].iter().all(|&x| (x - 0.5).abs() < 1e-4),
        "{}",
        out[245]
    );
    assert!(events(&mut rx).contains(&Event::VoiceEnded { pad: 1 }));
}

#[test]
fn restart_does_not_click() {
    let (mut e, _rx) = Engine::new(SR);
    let s = sine(200.0, 48_000, 0.5);
    trigger(&mut e, 1, &s, |_| {});
    let mut out = left(&render(&mut e, 1000));
    trigger(&mut e, 1, &s, |_| {});
    out.extend(left(&render(&mut e, 1000)));
    let max_step = out
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max);
    // An abrupt cut would jump ~0.43 here; two overlapping 200 Hz sines plus the fade stay well under 0.05.
    assert!(max_step < 0.05, "max step {max_step}");
}

#[test]
fn hold_release_fades_within_10ms_and_one_shot_ignores_release() {
    let (mut e, _rx) = Engine::new(SR);
    let s = constant(48_000, 0.5);
    trigger(&mut e, 1, &s, |p| p.hold = true);
    trigger(&mut e, 2, &s, |_| {});
    render(&mut e, 100);
    e.handle(Command::Release { pad: 1 });
    e.handle(Command::Release { pad: 2 });
    render(&mut e, 500);
    assert_eq!(e.active_voices(), 1);
}

#[test]
fn voice_limit_fades_oldest() {
    let (mut e, mut rx) = Engine::new(SR);
    let s = constant(48_000, 0.01);
    for pad in 0..33 {
        trigger(&mut e, pad, &s, |_| {});
    }
    render(&mut e, 200);
    assert_eq!(e.active_voices(), MAX_VOICES);
    assert!(events(&mut rx).contains(&Event::VoiceEnded { pad: 0 }));
}

#[test]
fn soft_clipper_never_exceeds_full_scale() {
    let (mut e, _rx) = Engine::new(SR);
    let s = constant(48_000, 0.5);
    for pad in 0..32 {
        trigger(&mut e, pad, &s, |_| {});
    }
    let out = render(&mut e, 64);
    assert!(out.iter().all(|&x| x <= 1.0 && x > 0.95));
}

#[test]
fn mute_and_solo() {
    let (mut e, _rx) = Engine::new(SR);
    trigger(&mut e, 1, &constant(48_000, 0.1), |p| p.mix_group = 0);
    trigger(&mut e, 2, &constant(48_000, 0.2), |p| p.mix_group = 1);
    e.handle(Command::SetGroup {
        group: 1,
        params: GroupParams {
            solo: true,
            ..GroupParams::default()
        },
    });
    assert!((render(&mut e, 8)[0] - 0.2).abs() < 1e-4);
    e.handle(Command::SetGroup {
        group: 1,
        params: GroupParams {
            mute: true,
            ..GroupParams::default()
        },
    });
    assert!((render(&mut e, 8)[0] - 0.1).abs() < 1e-4);
}

#[test]
fn hard_left_pan() {
    let (mut e, _rx) = Engine::new(SR);
    trigger(&mut e, 1, &constant(48_000, 0.5), |_| {});
    e.handle(Command::SetGroup {
        group: 0,
        params: GroupParams {
            pan: -1.0,
            ..GroupParams::default()
        },
    });
    let out = render(&mut e, 8);
    assert!((out[0] - 0.5 * std::f32::consts::SQRT_2).abs() < 1e-4);
    assert!(out[1].abs() < 1e-6);
}

#[test]
fn group_pitch_offset_changes_speed() {
    let (mut e, mut rx) = Engine::new(SR);
    e.handle(Command::SetGroup {
        group: 0,
        params: GroupParams {
            pitch_semitones: 12.0,
            ..GroupParams::default()
        },
    });
    trigger(&mut e, 1, &constant(100, 0.5), |_| {});
    let out = left(&render(&mut e, 100));
    assert!(out[10] > 0.0);
    assert_eq!(out[55], 0.0);
    assert!(events(&mut rx).contains(&Event::VoiceEnded { pad: 1 }));
}

#[test]
fn resamples_to_output_rate() {
    let (mut e, _rx) = Engine::new(SR);
    let s = Arc::new(Sample {
        rate: 44_100,
        data: vec![0.5; 4410 * 2],
    });
    trigger(&mut e, 1, &s, |_| {});
    let out = left(&render(&mut e, 4900));
    assert!(out[4790] > 0.0);
    assert_eq!(out[4805], 0.0);
}

#[test]
fn stop_all_silences_within_5ms() {
    let (mut e, _rx) = Engine::new(SR);
    let s = constant(48_000, 0.5);
    for pad in 0..4 {
        trigger(&mut e, pad, &s, |_| {});
    }
    e.handle(Command::StopAll);
    render(&mut e, 300);
    assert_eq!(e.active_voices(), 0);
}

#[test]
fn group_lowpass_is_applied() {
    let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
    let s = sine(5_000.0, 48_000, 0.5);
    let (mut dry, _rx1) = Engine::new(SR);
    trigger(&mut dry, 1, &s, |_| {});
    let dry_rms = rms(&left(&render(&mut dry, 4800))[480..]);
    let (mut wet, _rx2) = Engine::new(SR);
    wet.handle(Command::SetGroup {
        group: 0,
        params: GroupParams {
            lowpass_hz: 1_000.0,
            ..GroupParams::default()
        },
    });
    trigger(&mut wet, 1, &s, |_| {});
    let wet_rms = rms(&left(&render(&mut wet, 4800))[480..]);
    assert!(wet_rms < dry_rms * 0.1, "dry {dry_rms} wet {wet_rms}");
}

#[test]
fn faders_at_the_floor_render_exact_silence() {
    use lpd_core::kit::{group_params, Set};
    let sample = constant(4_800, 0.5);
    let mut set = Set::new();
    let sound = |e: &mut Engine| {
        trigger(e, 1, &sample, |_| {});
        render(e, 480)
    };

    // Group fader at the floor.
    let (mut e, _rx) = Engine::new(SR);
    set.mixer.groups[0].volume_db = -60.0;
    e.handle(Command::SetGroup {
        group: 0,
        params: group_params(&set.mixer.groups[0]),
    });
    assert!(sound(&mut e).iter().all(|s| *s == 0.0));

    // Master fader at the floor.
    let (mut e, _rx) = Engine::new(SR);
    e.handle(Command::SetMasterGain(lpd_core::audio::fader_gain(-60.0)));
    assert!(sound(&mut e).iter().all(|s| *s == 0.0));

    // Just above the floor still sounds.
    let (mut e, _rx) = Engine::new(SR);
    e.handle(Command::SetMasterGain(lpd_core::audio::fader_gain(-40.0)));
    assert!(sound(&mut e).iter().any(|s| *s != 0.0));
}

// ---- keys instrument (Task 15) ----

fn peak(out: &[f32]) -> f32 {
    out.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

fn play_keys(engine: &mut Engine) -> Vec<f32> {
    engine.handle(Command::KeysNoteOn {
        note: 60,
        velocity: 110,
    });
    // Skip the attack.
    render(engine, 4800);
    render(engine, 2400)
}

fn mute_group(engine: &mut Engine, group: u8) {
    engine.handle(Command::SetGroup {
        group,
        params: GroupParams {
            mute: true,
            ..Default::default()
        },
    });
}

#[test]
fn keys_on_the_master_are_audible_even_with_every_group_muted() {
    let (mut engine, _) = Engine::new(SR);
    for g in 0..8 {
        mute_group(&mut engine, g);
    }
    assert!(peak(&play_keys(&mut engine)) > 0.01);
}

#[test]
fn keys_routed_to_a_muted_channel_are_silent_but_other_channels_pass_them() {
    let (mut engine, _) = Engine::new(SR);
    engine.handle(Command::KeysRoute {
        gain: 1.0,
        group: Some(2),
    });
    mute_group(&mut engine, 2);
    assert_eq!(peak(&play_keys(&mut engine)), 0.0);
    // Unmute: the keys come through the channel again.
    engine.handle(Command::SetGroup {
        group: 2,
        params: GroupParams::default(),
    });
    assert!(peak(&render(&mut engine, 2400)) > 0.01);
}

#[test]
fn a_solo_on_another_channel_silences_keys_routed_to_a_channel() {
    let (mut engine, _) = Engine::new(SR);
    engine.handle(Command::KeysRoute {
        gain: 1.0,
        group: Some(2),
    });
    engine.handle(Command::SetGroup {
        group: 5,
        params: GroupParams {
            solo: true,
            ..Default::default()
        },
    });
    assert_eq!(peak(&play_keys(&mut engine)), 0.0);
}

#[test]
fn keys_gain_zero_is_silent() {
    let (mut engine, _) = Engine::new(SR);
    engine.handle(Command::KeysRoute {
        gain: 0.0,
        group: None,
    });
    assert_eq!(peak(&play_keys(&mut engine)), 0.0);
}

#[test]
fn keys_obey_the_master_gain() {
    let (mut engine, _) = Engine::new(SR);
    engine.handle(Command::SetMasterGain(0.0));
    assert_eq!(peak(&play_keys(&mut engine)), 0.0);
}

#[test]
fn keys_group_ignores_the_group_pitch() {
    // D5: the pitch offset of a channel does not retune the keys routed into it.
    let crossings = |pitch: f32| {
        let (mut engine, _) = Engine::new(SR);
        engine.handle(Command::KeysRoute {
            gain: 1.0,
            group: Some(1),
        });
        engine.handle(Command::SetGroup {
            group: 1,
            params: GroupParams {
                pitch_semitones: pitch,
                ..Default::default()
            },
        });
        engine.handle(Command::KeysNoteOn {
            note: 60,
            velocity: 100,
        });
        render(&mut engine, 4800);
        let x = left(&render(&mut engine, 24_000));
        x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count()
    };
    assert_eq!(crossings(0.0), crossings(7.0));
}

#[test]
fn keys_show_in_the_level_tap_of_their_channel() {
    let (mut engine, _events, mut taps) = Engine::with_taps(SR);
    engine.handle(Command::KeysRoute {
        gain: 1.0,
        group: Some(4),
    });
    play_keys(&mut engine);
    let mut group4 = 0.0f32;
    let mut others = 0.0f32;
    while let Ok(l) = taps.levels.pop() {
        group4 = group4.max(l.groups[4]);
        others = others.max(l.groups[0]).max(l.groups[7]);
    }
    assert!(group4 > 0.0);
    assert_eq!(others, 0.0);
}

#[derive(Debug)]
struct Silent;
impl lpd_core::instrument::Instrument for Silent {
    fn note_on(&mut self, _: u8, _: u8) {}
    fn note_off(&mut self, _: u8) {}
    fn pitch_bend(&mut self, _: f32) {}
    fn modulation(&mut self, _: f32) {}
    fn set_sound(&mut self, _: lpd_core::instrument::KeysSound) {}
    fn all_notes_off(&mut self) {}
    fn render_add(&mut self, _: &mut [f32], _: &mut [f32]) {}
}

#[test]
fn set_instrument_hands_the_old_one_to_the_trash() {
    let (mut engine, _) = Engine::new(SR);
    let mut trash = engine.take_trash().expect("the consumer is available once");
    assert!(engine.take_trash().is_none());
    engine.handle(Command::SetInstrument(Box::new(Silent)));
    assert!(
        matches!(trash.pop(), Ok(Trash::Instrument(_))),
        "the replaced Synth came back"
    );
    assert!(trash.pop().is_err());
    // The new instrument is silent.
    assert_eq!(peak(&play_keys(&mut engine)), 0.0);
}

#[test]
fn synth_commands_reach_the_synth_and_its_monitor() {
    let (mut engine, _events, mut taps) = Engine::with_taps(SR);
    engine.handle(Command::SynthParam(
        lpd_core::synth::params::ParamId::FilterCutoff,
        300.0,
    ));
    engine.handle(Command::KeysNoteOn {
        note: 60,
        velocity: 100,
    });
    for _ in 0..12 {
        render(&mut engine, 512);
    }
    let mut last = None;
    while let Ok(m) = taps.synth.pop() {
        last = Some(m);
    }
    let m = last.expect("a monitor per block");
    assert!((m.cutoff_hz / 300.0 - 1.0).abs() < 0.05, "{}", m.cutoff_hz);
    assert_eq!(m.voices, 1);
    assert!(m.cpu > 0.0);
    let mut any = false;
    while let Ok(s) = taps.keys.pop() {
        any |= s.abs() > 1e-3;
    }
    assert!(any, "the keys scope carries the synth");
}

#[test]
fn a_synth_load_returns_its_box() {
    let (mut engine, _) = Engine::new(SR);
    let mut trash = engine.take_trash().unwrap();
    engine.handle(Command::SynthLoad(Box::new(
        lpd_core::synth::params::SynthParams::default(),
    )));
    assert!(matches!(trash.pop(), Ok(Trash::Params(_))));
}

#[test]
fn keys_pitch_is_the_normalized_wheel() {
    use lpd_core::synth::params::SynthParams;
    let crossings = |wheel: f32| {
        let (mut engine, _) = Engine::new(SR);
        let mut p = SynthParams::default();
        p.osc_a.pos = 0.0; // sine
        p.filter.on = false;
        engine.handle(Command::SynthLoad(Box::new(p)));
        render(&mut engine, 960);
        engine.handle(Command::KeysPitch(wheel));
        engine.handle(Command::KeysNoteOn {
            note: 60,
            velocity: 100,
        });
        render(&mut engine, 4800);
        let x = left(&render(&mut engine, 48_000));
        x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count() as f32
    };
    let ratio = crossings(1.0) / crossings(0.0);
    assert!((ratio - 2f32.powf(2.0 / 12.0)).abs() < 0.01, "{ratio}");
}
