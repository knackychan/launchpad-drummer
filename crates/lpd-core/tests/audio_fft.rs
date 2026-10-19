use lpd_core::audio::fft::{bands, to_dbfs, trigger_index, Fft, BANDS, FFT_SIZE};
use std::f32::consts::PI;

const SR: u32 = 48_000;

fn sine(freq: f32, phase: f32, amp: f32) -> Box<[f32; FFT_SIZE]> {
    let mut buf = Box::new([0.0f32; FFT_SIZE]);
    for (i, s) in buf.iter_mut().enumerate() {
        *s = amp * (phase + 2.0 * PI * freq * i as f32 / SR as f32).sin();
    }
    buf
}

#[test]
fn a_one_khz_sine_peaks_near_one_khz_in_the_right_band() {
    let mut fft = Fft::new();
    let input = sine(1000.0, 0.0, 1.0);
    let mags = *fft.magnitudes(&input);
    let (b, peak_hz) = bands(&mags, SR);
    assert!((peak_hz - 1000.0).abs() <= 25.0, "peak {peak_hz}");
    let loudest = (0..BANDS)
        .max_by(|&x, &y| b[x].partial_cmp(&b[y]).unwrap())
        .unwrap();
    // Band edges are 40 Hz * 400^(i/28): band 15 spans about 991..1229 Hz.
    let lo = 40.0 * 400f32.powf(loudest as f32 / BANDS as f32);
    let hi = 40.0 * 400f32.powf((loudest + 1) as f32 / BANDS as f32);
    assert!(lo <= 1000.0 && 1000.0 < hi, "band {loudest}: {lo}..{hi}");
    assert!(b[loudest] > -6.0, "level {}", b[loudest]);
}

#[test]
fn a_full_scale_sine_reads_close_to_one() {
    let mut fft = Fft::new();
    // Exactly on a bin: 64 * 48000 / 2048 = 1500 Hz.
    let input = sine(1500.0, 0.0, 1.0);
    let mags = fft.magnitudes(&input);
    let top = mags.iter().cloned().fold(0.0, f32::max);
    assert!((top - 1.0).abs() < 0.02, "top {top}");
}

#[test]
fn silence_is_the_floor_with_no_peak() {
    let mut fft = Fft::new();
    let input = Box::new([0.0f32; FFT_SIZE]);
    let mags = *fft.magnitudes(&input);
    let (b, peak_hz) = bands(&mags, SR);
    assert!(b.iter().all(|&v| v == -100.0), "{b:?}");
    assert_eq!(peak_hz, 0.0);
}

#[test]
fn trigger_index_finds_the_rising_zero_crossing() {
    // 960 Hz at 48 kHz is 50 samples per cycle; starting at phase pi/2 it is a cosine, negative
    // between samples 12.5 and 37.5, so the first sample back at or above zero is 38.
    let buf: Vec<f32> = (0..400)
        .map(|i| (PI / 2.0 + 2.0 * PI * 960.0 * i as f32 / SR as f32).sin())
        .collect();
    assert_eq!(trigger_index(&buf), 38);
    assert_eq!(trigger_index(&[0.5; 64]), 0);
    assert_eq!(trigger_index(&[]), 0);
}

#[test]
fn dbfs_has_a_floor() {
    assert_eq!(to_dbfs(1.0), 0.0);
    assert_eq!(to_dbfs(0.0), -100.0);
    assert!((to_dbfs(0.5) + 6.0206).abs() < 0.001);
}
