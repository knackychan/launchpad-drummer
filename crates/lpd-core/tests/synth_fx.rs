use lpd_core::audio::fft::fft_complex;
use lpd_core::instrument::Instrument;
use lpd_core::synth::fx::FxChain;
use lpd_core::synth::params::*;
use lpd_core::synth::Synth;

const RATE: f32 = 48_000.0;

fn run(chain: &mut FxChain, p: &FxParams, l: &mut [f32], r: &mut [f32]) {
    for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        chain.process(cl, cr, p, 120.0);
    }
}

fn impulse(seconds: f32) -> (Vec<f32>, Vec<f32>) {
    let n = (seconds * RATE) as usize;
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    l[0] = 1.0;
    r[0] = 1.0;
    (l, r)
}

fn argmax(x: &[f32]) -> usize {
    x.iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .unwrap()
        .0
}

#[test]
fn off_units_pass_the_signal_through() {
    let mut chain = FxChain::new(RATE);
    let p = FxParams::default();
    let mut rng = 5u32;
    let input: Vec<f32> = (0..4096)
        .map(|_| lpd_core::synth::osc::next_bipolar(&mut rng))
        .collect();
    let (mut l, mut r) = (input.clone(), input.clone());
    run(&mut chain, &p, &mut l, &mut r);
    assert_eq!(l, input);
    assert_eq!(r, input);
}

#[test]
fn the_delay_echo_lands_on_the_sample() {
    let mut chain = FxChain::new(RATE);
    let p = FxParams {
        delay: DelayFx {
            on: true,
            time: 0.1,
            feedback: 0.0,
            mix: 1.0,
            ..DelayFx::default()
        },
        ..FxParams::default()
    };
    let (mut l, mut r) = impulse(0.2);
    run(&mut chain, &p, &mut l, &mut r);
    assert_eq!(argmax(&l), 4800);
    assert!((l[4800] - 1.0).abs() < 1e-3, "{}", l[4800]);
}

#[test]
fn delay_feedback_stays_bounded() {
    let mut chain = FxChain::new(RATE);
    let p = FxParams {
        delay: DelayFx {
            on: true,
            time: 0.05,
            feedback: 0.95,
            mix: 0.5,
            ..DelayFx::default()
        },
        ..FxParams::default()
    };
    let n = (10.0 * RATE) as usize;
    let mut rng = 3u32;
    let mut l: Vec<f32> = (0..n)
        .map(|i| {
            if i < 4800 {
                lpd_core::synth::osc::next_bipolar(&mut rng)
            } else {
                0.0
            }
        })
        .collect();
    let mut r = l.clone();
    run(&mut chain, &p, &mut l, &mut r);
    assert!(l.iter().chain(&r).all(|v| v.is_finite() && v.abs() < 8.0));
    let early = l[24_000..72_000].iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let late = l[n - 48_000..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(late < early);
}

#[test]
fn ping_pong_alternates_sides() {
    let mut chain = FxChain::new(RATE);
    let p = FxParams {
        delay: DelayFx {
            on: true,
            time: 0.1,
            feedback: 0.5,
            ping_pong: true,
            mix: 1.0,
            ..DelayFx::default()
        },
        ..FxParams::default()
    };
    let (mut l, mut r) = impulse(0.25);
    run(&mut chain, &p, &mut l, &mut r);
    assert!(l[4800] > 0.9 && r[4800].abs() < 1e-3, "first echo left");
    // The 6 kHz feedback low-pass scales this impulse's first sample by 0.544062.
    assert!(
        (r[9600] - 0.272_031).abs() < 1e-5 && l[9600].abs() < 1e-3,
        "second echo right: {}",
        r[9600]
    );
}

#[test]
fn the_reverb_decays_in_its_rt60() {
    let mut chain = FxChain::new(RATE);
    let p = FxParams {
        reverb: ReverbFx {
            on: true,
            size: 0.6,
            decay: 2.0,
            damp: 0.0,
            pre_delay: 0.0,
            mix: 1.0,
        },
        ..FxParams::default()
    };
    let (mut l, mut r) = impulse(6.0);
    run(&mut chain, &p, &mut l, &mut r);
    // Schroeder backward integration, then the −5 → −25 dB slope × 3.
    let mut edc = vec![0.0f64; l.len()];
    let mut acc = 0.0f64;
    for i in (0..l.len()).rev() {
        acc += f64::from(l[i] * l[i] + r[i] * r[i]);
        edc[i] = acc;
    }
    let db = |i: usize| 10.0 * (edc[i] / edc[0]).log10();
    let t5 = (0..l.len()).find(|&i| db(i) <= -5.0).unwrap() as f64 / f64::from(RATE);
    let t25 = (0..l.len()).find(|&i| db(i) <= -25.0).unwrap() as f64 / f64::from(RATE);
    let rt60 = 3.0 * (t25 - t5);
    assert!((1.6..=2.4).contains(&rt60), "rt60 {rt60}");
}

#[test]
fn the_chorus_adds_no_dc() {
    let mut chain = FxChain::new(RATE);
    let p = FxParams {
        chorus: ChorusFx {
            on: true,
            feedback: 0.5,
            ..ChorusFx::default()
        },
        ..FxParams::default()
    };
    let n = (2.0 * RATE) as usize;
    let mut l: Vec<f32> = (0..n)
        .map(|i| 0.5 * (std::f32::consts::TAU * 200.0 * i as f32 / RATE).sin())
        .collect();
    let mut r = l.clone();
    run(&mut chain, &p, &mut l, &mut r);
    let tail = &l[n / 2..];
    assert!((tail.iter().sum::<f32>() / tail.len() as f32).abs() < 1e-3);
}

#[test]
fn drive_saturates_and_stays_bounded() {
    for &kind in DriveType::ALL {
        let mut chain = FxChain::new(RATE);
        let p = FxParams {
            drive: DriveFx {
                on: true,
                kind,
                amount: 1.0,
                mix: 1.0,
                ..DriveFx::default()
            },
            ..FxParams::default()
        };
        let mut l: Vec<f32> = (0..48_000)
            .map(|i| (std::f32::consts::TAU * 1_000.0 * i as f32 / RATE).sin())
            .collect();
        let mut r = l.clone();
        run(&mut chain, &p, &mut l, &mut r);
        assert!(
            l.iter().all(|v| v.is_finite() && v.abs() <= 1.1),
            "{kind:?}"
        );
    }
    // SOFT at half drive adds a clear third harmonic to a sine on bin 43 of a 2048-point FFT.
    let hz = RATE * 43.0 / 2048.0;
    let mut chain = FxChain::new(RATE);
    let p = FxParams {
        drive: DriveFx {
            on: true,
            amount: 0.5,
            ..DriveFx::default()
        },
        ..FxParams::default()
    };
    let mut l: Vec<f32> = (0..8192)
        .map(|i| 0.5 * (std::f32::consts::TAU * hz * i as f32 / RATE).sin())
        .collect();
    let mut r = l.clone();
    run(&mut chain, &p, &mut l, &mut r);
    let mut re: Vec<f32> = l[6144..]
        .iter()
        .enumerate()
        .map(|(i, v)| v * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / 2048.0).cos()))
        .collect();
    let mut im = vec![0.0; 2048];
    fft_complex(&mut re, &mut im, false);
    let mag = |k: usize| re[k].hypot(im[k]);
    assert!(
        mag(129) > 0.01 * mag(43),
        "third harmonic {} vs {}",
        mag(129),
        mag(43)
    );
}

#[test]
fn silent_units_go_idle_and_wake_up() {
    let mut chain = FxChain::new(RATE);
    let p = FxParams {
        reverb: ReverbFx {
            on: true,
            decay: 0.5,
            ..ReverbFx::default()
        },
        ..FxParams::default()
    };
    let (mut l, mut r) = impulse(10.0);
    run(&mut chain, &p, &mut l, &mut r);
    assert!(chain.idle()[3], "the reverb tail has died away");
    let mut l = vec![0.5; 256];
    let mut r = vec![0.5; 256];
    chain.process(&mut l, &mut r, &p, 120.0);
    assert!(!chain.idle()[3]);
}

#[test]
fn clear_empties_every_tail() {
    let mut chain = FxChain::new(RATE);
    let p = FxParams {
        delay: DelayFx {
            on: true,
            ..DelayFx::default()
        },
        reverb: ReverbFx {
            on: true,
            ..ReverbFx::default()
        },
        ..FxParams::default()
    };
    let (mut l, mut r) = impulse(0.05);
    run(&mut chain, &p, &mut l, &mut r);
    chain.clear();
    let (mut l, mut r) = (vec![0.0; 24_000], vec![0.0; 24_000]);
    run(&mut chain, &p, &mut l, &mut r);
    assert!(l.iter().chain(&r).all(|&v| v == 0.0));
}

#[test]
fn the_synth_runs_its_fx_chain() {
    let mut s = Synth::new(48_000);
    let mut p = SynthParams::default();
    p.fx.reverb = ReverbFx {
        on: true,
        decay: 3.0,
        mix: 0.5,
        ..ReverbFx::default()
    };
    s.load_params(&p);
    let mut l = vec![0.0; 480];
    let mut r = vec![0.0; 480];
    s.render_add(&mut l, &mut r);
    s.note_on(60, 120);
    let mut l = vec![0.0; 24_000];
    let mut r = vec![0.0; 24_000];
    s.render_add(&mut l[..1024], &mut r[..1024]);
    s.note_off(60);
    for (cl, cr) in l.chunks_mut(1024).zip(r.chunks_mut(1024)).skip(1) {
        s.render_add(cl, cr);
    }
    assert_eq!(s.active_voices(), 0, "the voice is over");
    let tail = l[20_000..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(tail > 1e-4, "the reverb still rings");
}
