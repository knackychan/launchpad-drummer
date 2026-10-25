use lpd_core::audio::fft::fft_complex;
use lpd_core::synth::osc::*;
use lpd_core::synth::params::{NoiseColor, SubShape, TableId, Warp};
use lpd_core::synth::wavetable::table;

const RATE: f32 = 48_000.0;

fn rising_crossing_hz(x: &[f32]) -> f32 {
    let mut times = Vec::new();
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            times.push((i - 1) as f32 + x[i - 1] / (x[i - 1] - x[i]));
        }
    }
    (times.len() - 1) as f32 / (times[times.len() - 1] - times[0]) * RATE
}

fn tick(hz: f32, pos: f32, unison: usize, warp: WarpShape) -> OscTick {
    OscTick::new(
        table(TableId::Basic),
        RATE,
        hz,
        pos,
        unison,
        0.25,
        0.75,
        0.0,
        0.0,
        1.0,
        warp,
    )
}

#[test]
fn a4_is_in_tune() {
    let t = tick(440.0, 0.0, 1, WarpShape::new(Warp::Off, 0.0));
    let mut s = OscState::default();
    let x: Vec<f32> = (0..48_000).map(|_| s.sample(&t, 0.0).0).collect();
    let hz = rising_crossing_hz(&x);
    assert!((hz - 440.0).abs() < 0.0254, "{hz}");
}

#[test]
fn unison_spreads_symmetrically() {
    let c = unison_cents(7, 0.5);
    let want = [-25.0, -16.6667, -8.3333, 0.0, 8.3333, 16.6667, 25.0];
    for (a, b) in c.iter().zip(want) {
        assert!((a - b).abs() < 1e-3, "{c:?}");
    }
    assert_eq!(unison_cents(1, 1.0)[0], 0.0);
    assert_eq!(&unison_cents(2, 1.0)[..2], &[-100.0, 100.0]);
    assert_eq!((centre(1), centre(2), centre(4), centre(7)), (0, 0, 1, 3));
}

#[test]
fn unison_gains_keep_unit_power() {
    for n in 1..=7 {
        for blend in [0.0, 0.5, 1.0] {
            let g = unison_gains(n, blend);
            let power: f32 = g.iter().map(|v| v * v).sum();
            assert!((power - 1.0).abs() < 1e-5, "n {n} blend {blend}");
        }
    }
    assert!((unison_gains(3, 0.75)[1] - 0.685_994).abs() < 1e-5);
    assert_eq!(&unison_pans(3, 1.0)[..3], &[-1.0, 0.0, 1.0]);
    assert_eq!(unison_pans(1, 1.0)[0], 0.0);
    let (l, r) = pan_gains(0.0);
    assert!((l - 1.0).abs() < 1e-6 && (r - 1.0).abs() < 1e-6);
}

#[test]
fn every_warp_stays_bounded() {
    for &mode in Warp::ALL {
        for amt in [0.0, 0.5, 1.0] {
            let t = tick(220.0, 0.667, 1, WarpShape::new(mode, amt));
            let mut s = OscState::default();
            for i in 0..4800 {
                let m = (std::f32::consts::TAU * 330.0 * i as f32 / RATE).sin();
                let (l, r) = s.sample(&t, m);
                assert!(l.abs() <= 1.5 && r.abs() <= 1.5, "{mode:?} {amt}: {l}");
            }
        }
    }
}

#[test]
fn warp_shapes_bend_the_phase() {
    let pwm = WarpShape::new(Warp::Pwm, 0.5);
    assert_eq!(pwm.phase(0.9), None);
    assert!((pwm.phase(0.2).unwrap() - 0.2 / 0.51).abs() < 1e-6);
    let sync = WarpShape::new(Warp::Sync, 0.25);
    assert!((sync.freq_factor() - 2.0).abs() < 1e-5);
    assert!((sync.phase(0.75).unwrap() - 0.5).abs() < 1e-5);
    assert!((WarpShape::new(Warp::Bend, 0.25).phase(0.5).unwrap() - 0.25).abs() < 1e-6);
    assert_eq!(WarpShape::new(Warp::Fm, 1.0).freq_factor(), 1.0);
}

#[test]
fn the_sub_plays_the_frequency_it_is_given() {
    for shape in [SubShape::Sine, SubShape::Square] {
        let mut s = SubState::default();
        let x: Vec<f32> = (0..48_000).map(|_| s.sample(shape, 110.0 / RATE)).collect();
        assert!((rising_crossing_hz(&x) - 110.0).abs() < 0.05, "{shape:?}");
    }
}

#[test]
fn pink_noise_falls_3_db_per_octave() {
    let mut n = NoiseState::new(7);
    let mut power = vec![0.0f64; 1024];
    for _ in 0..64 {
        let mut re: Vec<f32> = (0..2048)
            .map(|i| {
                n.sample(NoiseColor::Pink)
                    * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / 2048.0).cos())
            })
            .collect();
        let mut im = vec![0.0; 2048];
        fft_complex(&mut re, &mut im, false);
        for k in 0..1024 {
            power[k] += f64::from(re[k] * re[k] + im[k] * im[k]);
        }
    }
    let band = |lo: usize, hi: usize| power[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64;
    // Bins of 23.4 Hz: 177–354 Hz and 2828–5657 Hz, four octaves apart.
    let ratio_db = 10.0 * (band(121, 241) / band(8, 15)).log10();
    assert!((-14.0..=-10.0).contains(&ratio_db), "{ratio_db}");
}

#[test]
fn white_noise_is_bounded_and_centred() {
    let mut n = NoiseState::new(3);
    let x: Vec<f32> = (0..48_000).map(|_| n.sample(NoiseColor::White)).collect();
    assert!(x.iter().all(|v| v.abs() <= 1.0));
    assert!((x.iter().sum::<f32>() / x.len() as f32).abs() < 0.01);
}
