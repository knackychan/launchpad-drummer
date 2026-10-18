#![allow(dead_code)]
use std::path::Path;

/// Mono 16-bit WAV where sample i = i * step.
pub fn write_wav_mono_ramp(path: &Path, rate: u32, frames: usize, step: i16) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        w.write_sample((i as i16).wrapping_mul(step)).unwrap();
    }
    w.finalize().unwrap();
}

/// Stereo 32-bit float WAV with constant left/right values.
pub fn write_wav_stereo(path: &Path, rate: u32, frames: usize, left: f32, right: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..frames {
        w.write_sample(left).unwrap();
        w.write_sample(right).unwrap();
    }
    w.finalize().unwrap();
}
