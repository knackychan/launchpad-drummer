use lpd_core::audio::fft::fft_complex;
use lpd_core::synth::params::TableId;
use lpd_core::synth::wavetable::{build_all, table, Wavetable, LEVELS};

fn spectrum(signal: &[f32]) -> Vec<f32> {
    let mut re = signal.to_vec();
    let mut im = vec![0.0; signal.len()];
    fft_complex(&mut re, &mut im, false);
    re.iter()
        .zip(&im)
        .take(signal.len() / 2)
        .map(|(r, i)| r.hypot(*i))
        .collect()
}

#[test]
fn complex_fft_round_trips() {
    let mut re: Vec<f32> = (0..64).map(|i| (i as f32 * 0.37).sin()).collect();
    let original = re.clone();
    let mut im = vec![0.0; 64];
    fft_complex(&mut re, &mut im, false);
    fft_complex(&mut re, &mut im, true);
    for (a, b) in re.iter().zip(&original) {
        assert!((a - b).abs() < 1e-5);
    }
}

#[test]
fn every_level_is_band_limited() {
    for &id in TableId::ALL {
        let t = table(id);
        for frame in [0, t.frames() - 1] {
            for level in 0..LEVELS {
                let mags = spectrum(t.level_data(level, frame));
                let peak = mags.iter().cloned().fold(0.0, f32::max);
                let above = mags
                    .iter()
                    .skip(Wavetable::harmonics(level) + 1)
                    .cloned()
                    .fold(0.0, f32::max);
                assert!(
                    above <= 1e-4 * peak,
                    "{id:?} frame {frame} level {level}: {above} vs {peak}"
                );
            }
        }
    }
}

#[test]
fn frames_are_normalized() {
    for &id in TableId::ALL {
        let t = table(id);
        for frame in 0..t.frames() {
            let peak = t
                .level_data(0, frame)
                .iter()
                .fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(
                (0.99..=1.01).contains(&peak),
                "{id:?} frame {frame}: {peak}"
            );
        }
    }
}

#[test]
fn basic_starts_with_a_pure_sine() {
    let mags = spectrum(table(TableId::Basic).level_data(0, 0));
    let fundamental = mags[1];
    assert!(mags
        .iter()
        .enumerate()
        .filter(|&(k, _)| k != 1)
        .all(|(_, &m)| m < 1e-4 * fundamental));
}

#[test]
fn high_notes_do_not_alias() {
    // A saw at exactly bin 171 of a 2048-point FFT at 48 kHz (about 4 kHz).
    let rate = 48_000.0;
    let hz = rate * 171.0 / 2048.0;
    let t = table(TableId::Basic);
    let level = Wavetable::level_for(hz, rate);
    let mut phase = 0.0f32;
    let mut x = vec![0.0f32; 2048];
    for (i, v) in x.iter_mut().enumerate() {
        let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / 2048.0).cos();
        *v = t.read(level, 0.667, phase) * w;
        phase = (phase + hz / rate).fract();
    }
    let mags = spectrum(&x);
    let total: f32 = mags.iter().map(|m| m * m).sum();
    let stray: f32 = mags
        .iter()
        .enumerate()
        .filter(|&(k, _)| (1..12).all(|h| (k as i64 - (171 * h) as i64).abs() > 3))
        .map(|(_, m)| m * m)
        .sum();
    // A naive saw would leave aliases around −20 dB; the band-limited one stays under −50 dB.
    assert!(stray < 1e-5 * total, "aliasing energy {}", stray / total);
}

#[test]
fn level_for_picks_the_highest_safe_level() {
    assert_eq!(Wavetable::level_for(20.0, 48_000.0), 0);
    assert_eq!(Wavetable::level_for(440.0, 48_000.0), 4);
    assert_eq!(Wavetable::level_for(4_000.0, 48_000.0), 7);
    assert_eq!(Wavetable::level_for(30_000.0, 48_000.0), LEVELS - 1);
}

#[test]
fn display_frames_have_the_requested_size() {
    let frames = table(TableId::Pwm).display(128);
    assert_eq!(frames.len(), 32);
    assert!(frames.iter().all(|f| f.len() == 128));
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_tables_build_within_200_ms() {
    let t0 = std::time::Instant::now();
    let tables = build_all();
    assert_eq!(tables.len(), 8);
    assert!(t0.elapsed().as_millis() < 200, "{:?}", t0.elapsed());
}
