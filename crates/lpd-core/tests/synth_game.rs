//! GameEngineVoice (Dustline ship engine spec, sections 5.1 and 7).
use lpd_core::synth::game::*;
use lpd_core::synth::params::*;

const RATE: u32 = 48_000;
const TICK: usize = 16;

fn frame(bend: f32, wheel: f32, gain: f32) -> GameFrame {
    GameFrame {
        on: true,
        bend,
        wheel,
        gain,
    }
}

fn render(v: &mut GameEngineVoice, samples: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (vec![0.0; samples], vec![0.0; samples]);
    for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        v.render_add(cl, cr);
    }
    (l, r)
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

fn engine_patch() -> SynthParams {
    let mut p = SynthParams::default();
    p.osc_a.on = true;
    p.sub.on = true;
    p
}

#[test]
fn legato_and_a_24_semitone_bend_are_forced_and_the_rest_of_the_patch_is_kept() {
    let mut p = engine_patch();
    p.voice.mode = VoiceMode::Poly;
    p.voice.bend_range = 2;
    p.filter.cutoff = 900.0;
    let v = GameEngineVoice::new(RATE, &p);
    assert_eq!(v.params().voice.mode, VoiceMode::Legato);
    assert_eq!(v.params().voice.bend_range, 24);
    let mut want = p;
    want.voice.mode = VoiceMode::Legato;
    want.voice.bend_range = 24;
    assert_eq!(*v.params(), want);
}

#[test]
fn silent_until_the_first_frame_then_the_note_fades_in_over_25_ms() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    assert_eq!(peak(&render(&mut v, 4800).0), 0.0);
    assert!(!v.current().on);
    v.frame(frame(0.0, 0.5, 1.0));
    let (l, _) = render(&mut v, 4800);
    assert!(v.current().on);
    assert!(peak(&l[..48]) < peak(&l[2400..]) * 0.1, "fades in");
    assert!(peak(&l[2400..]) > 0.01);
    assert_eq!(v.current().gain, 1.0);
}

#[test]
fn bend_and_wheel_glide_over_40_ms_and_the_gain_ramps_over_25_ms_in_16_sample_ticks() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    v.frame(frame(0.0, 0.2, 1.0));
    render(&mut v, 4800);
    // 375 cents in one frame: 3.125 cents per tick, the target reached after 120 ticks (40 ms at 48 kHz); the gain
    // lands after 75 ticks (25 ms)
    v.frame(frame(3.75 / 24.0, 0.85, 0.5));
    let (mut l, mut r) = ([0.0f32; TICK], [0.0f32; TICK]);
    let mut last = v.current();
    for tick in 1..=125 {
        v.render_add(&mut l, &mut r);
        let now = v.current();
        let cents = (now.bend - last.bend) * 24.0 * 100.0;
        if tick <= 120 {
            assert!((cents - 3.125).abs() < 0.01, "tick {tick}: {cents} cents");
            assert!(now.wheel > last.wheel);
        } else {
            assert_eq!(cents, 0.0, "settled after 120 ticks");
        }
        assert_eq!(
            now.gain == 0.5,
            tick >= 75,
            "tick {tick}: gain {}",
            now.gain
        );
        last = now;
    }
    assert_eq!(last.bend, 3.75 / 24.0);
    assert_eq!(last.wheel, 0.85);
}

#[test]
fn the_watchdog_fades_out_after_250_ms_without_frames_and_a_new_frame_restarts_the_note() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    v.frame(frame(0.0, 0.5, 1.0));
    let (l, _) = render(&mut v, 12_000); // 250 ms
    assert!(peak(&l[9_600..]) > 0.01, "still sounding at 200-250 ms");
    let (l, _) = render(&mut v, 2_400 + 512); // the 50 ms fade, then silence
    assert!(peak(&l[2_400 + 256..]) == 0.0, "silent after the fade");
    assert!(!v.current().on, "the note is released");
    v.frame(frame(0.0, 0.5, 1.0));
    let (l, _) = render(&mut v, 4_800);
    assert!(
        v.current().on && peak(&l[2_400..]) > 0.01,
        "a new frame restarts the note"
    );
}

#[test]
fn on_false_fades_out_over_one_ramp_and_releases_the_note() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    v.frame(frame(0.0, 0.5, 1.0));
    render(&mut v, 4_800);
    v.frame(GameFrame {
        on: false,
        bend: 0.0,
        wheel: 0.5,
        gain: 1.0,
    });
    let (l, _) = render(&mut v, 1_200 + 256);
    assert!(peak(&l[1_200 + 16..]) == 0.0);
    assert!(!v.current().on);
}

#[test]
fn nan_and_out_of_range_values_are_clamped_or_ignored() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    v.frame(frame(0.25, 0.5, 0.5));
    render(&mut v, 4_800);
    v.frame(frame(f32::NAN, 7.0, -3.0));
    render(&mut v, 4_800);
    let c = v.current();
    assert_eq!((c.bend, c.wheel, c.gain), (0.25, 1.0, 0.0));
    v.frame(frame(2.0, f32::INFINITY, f32::NAN));
    render(&mut v, 4_800);
    let c = v.current();
    assert_eq!((c.bend, c.wheel, c.gain), (1.0, 1.0, 0.0));
    let (l, r) = render(&mut v, 4_800);
    assert!(l.iter().chain(&r).all(|s| s.is_finite()));
}

#[test]
fn stop_fades_out_over_50_ms_then_finishes_and_ignores_later_frames() {
    let mut v = GameEngineVoice::new(RATE, &engine_patch());
    v.frame(frame(0.0, 0.5, 1.0));
    render(&mut v, 4_800);
    v.stop();
    assert!(!v.finished());
    render(&mut v, 2_400);
    assert!(v.finished());
    v.frame(frame(0.0, 0.5, 1.0));
    assert_eq!(peak(&render(&mut v, 4_800).0), 0.0);
}

#[test]
fn glides_take_40_ms_at_44_1_khz_too() {
    let mut v = GameEngineVoice::new(44_100, &engine_patch());
    v.frame(frame(0.0, 0.2, 1.0));
    render(&mut v, 4_410);
    v.frame(frame(0.5, 0.2, 1.0));
    let (mut l, mut r) = ([0.0f32; TICK], [0.0f32; TICK]);
    let mut ticks = 0;
    while v.current().bend < 0.5 {
        v.render_add(&mut l, &mut r);
        ticks += 1;
    }
    assert_eq!(ticks, 110, "round(0.040 x 44100 / 16)");
}
