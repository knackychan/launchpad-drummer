use lpd_core::audio::*;
use std::sync::Arc;

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
