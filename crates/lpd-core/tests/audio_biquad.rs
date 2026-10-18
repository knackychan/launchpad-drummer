use lpd_core::audio::LowPass;

const FS: f32 = 48_000.0;

/// Gain in dB of a sine through the filter, measured after it settles.
fn gain_db(lp: &mut LowPass, freq: f32) -> f32 {
    let n = 48_000;
    let (mut sum_in, mut sum_out) = (0.0f64, 0.0f64);
    for i in 0..n {
        let x = (2.0 * std::f32::consts::PI * freq * i as f32 / FS).sin();
        let y = lp.process(x);
        if i >= 4800 {
            sum_in += (x * x) as f64;
            sum_out += (y * y) as f64;
        }
    }
    10.0 * (sum_out / sum_in).log10() as f32
}

#[test]
fn bypassed_at_20khz() {
    let mut lp = LowPass::new();
    lp.set_cutoff(20_000.0, FS);
    assert_eq!(lp.process(0.3), 0.3);
    assert_eq!(lp.process(-0.7), -0.7);
}

#[test]
fn passes_low_frequencies() {
    let mut lp = LowPass::new();
    lp.set_cutoff(1_000.0, FS);
    assert!(gain_db(&mut lp, 100.0).abs() < 0.5);
}

#[test]
fn minus_3db_at_cutoff() {
    let mut lp = LowPass::new();
    lp.set_cutoff(1_000.0, FS);
    let g = gain_db(&mut lp, 1_000.0);
    assert!((g + 3.0).abs() < 0.5, "got {g}");
}

#[test]
fn attenuates_high_frequencies() {
    let mut lp = LowPass::new();
    lp.set_cutoff(1_000.0, FS);
    let g = gain_db(&mut lp, 5_000.0);
    assert!(g < -24.0, "got {g}");
}
