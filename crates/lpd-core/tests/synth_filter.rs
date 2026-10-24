use lpd_core::synth::filter::*;
use lpd_core::synth::params::FilterType;
use std::f32::consts::PI;

const RATE: f32 = 48_000.0;
const ALL: [FilterType; 5] = [
    FilterType::Lp24,
    FilterType::Lp12,
    FilterType::Hp12,
    FilterType::Bp12,
    FilterType::Notch,
];

fn res(kind: FilterType, reso: f32) -> f32 {
    if kind == FilterType::Lp24 {
        ladder_k(reso)
    } else {
        svf_q(reso)
    }
}

/// Gain in dB of a small sine at `hz` through the filter (steady state).
fn measured_db(kind: FilterType, cutoff: f32, reso: f32, hz: f32) -> f32 {
    let mut f = VoiceFilter::default();
    f.set_kind(kind);
    let g = g_for(cutoff, RATE);
    let (mut sum_in, mut sum_out) = (0.0f64, 0.0f64);
    for i in 0..28_800 {
        let x = 0.01 * (2.0 * PI * hz * i as f32 / RATE).sin();
        let (y, _) = f.process(x, x, g, res(kind, reso), 1.0);
        if i >= 14_400 {
            sum_in += f64::from(x * x);
            sum_out += f64::from(y * y);
        }
    }
    (10.0 * (sum_out / sum_in).log10()) as f32
}

#[test]
fn each_type_matches_its_analytic_curve() {
    let fc = 1_000.0;
    for kind in ALL {
        for hz in [100.0, 300.0, 700.0, 1_000.0, 1_500.0, 3_000.0, 6_000.0] {
            // The bilinear transform maps `hz` onto this analog frequency.
            let analog = fc * (PI * hz / RATE).tan() / (PI * fc / RATE).tan();
            let want = response_db(kind, fc, 0.3, 0.0, &[analog])[0];
            if want < -40.0 {
                continue;
            }
            let got = measured_db(kind, fc, 0.3, hz);
            assert!(
                (got - want).abs() <= 1.0,
                "{kind:?} at {hz} Hz: {got:.2} dB vs {want:.2} dB"
            );
        }
    }
}

#[test]
fn the_ladder_self_oscillates_at_full_resonance() {
    let mut l = Ladder::default();
    let (g, k) = (g_for(1_000.0, RATE), ladder_k(1.0));
    let mut out = Vec::with_capacity(28_800);
    for i in 0..28_800 {
        out.push(l.process(if i == 0 { 0.5 } else { 0.0 }, g, k));
    }
    let tail = &out[24_000..];
    let rms = (tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt();
    assert!(rms > 0.05, "rms {rms}");
    let crossings = tail
        .windows(2)
        .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
        .count() as f32;
    let hz = crossings / (tail.len() as f32 / RATE);
    assert!((hz - 1_000.0).abs() < 50.0, "{hz}");
}

#[test]
fn fast_cutoff_modulation_stays_finite() {
    let mut rng = 1u32;
    for kind in ALL {
        let mut f = VoiceFilter::default();
        f.set_kind(kind);
        for i in 0..48_000 {
            let cutoff =
                50.0 * 300f32.powf(0.5 + 0.5 * (2.0 * PI * 2_000.0 * i as f32 / RATE).sin());
            let x = 0.5 * lpd_core::synth::osc::next_bipolar(&mut rng);
            let (y, _) = f.process(x, x, g_for(cutoff, RATE), res(kind, 0.9), 1.0);
            assert!(y.is_finite() && y.abs() < 10.0, "{kind:?}: {y}");
        }
    }
}

#[test]
fn drive_shifts_the_drawn_level() {
    let base = response_db(FilterType::Lp12, 1_000.0, 0.0, 0.0, &[100.0])[0];
    let driven = response_db(FilterType::Lp12, 1_000.0, 0.0, 0.5, &[100.0])[0];
    assert!((driven - base - 6.0).abs() < 1e-3);
}

#[test]
fn resonance_ranges() {
    assert_eq!(svf_q(0.0), 0.5);
    assert!((svf_q(1.0) - 25.0).abs() < 1e-4);
    assert!((ladder_k(1.0) - 4.1).abs() < 1e-6);
    assert_eq!(drive_gain(0.0), 1.0);
    assert!((drive_gain(1.0) - 15.848_93).abs() < 1e-3);
    assert!(
        (g_for(30_000.0, RATE) - (PI * 20_000.0 / RATE).tan()).abs() < 1e-6,
        "clamped to 20 kHz"
    );
}
