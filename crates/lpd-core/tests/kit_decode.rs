mod common;

use lpd_core::kit::*;
use std::fs;

#[test]
fn decodes_mono_wav_to_stereo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ramp.wav");
    common::write_wav_mono_ramp(&path, 48_000, 480, 50);
    let s = decode_file(&path).unwrap();
    assert_eq!(s.rate, 48_000);
    assert_eq!(s.frames(), 480);
    let (l, r) = s.frame(100);
    assert!((l - 5000.0 / 32768.0).abs() < 1e-4, "got {l}");
    assert_eq!(l, r);
}

#[test]
fn decodes_stereo_wav_keeping_channels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("st.wav");
    common::write_wav_stereo(&path, 44_100, 100, 0.25, -0.5);
    let s = decode_file(&path).unwrap();
    assert_eq!(s.rate, 44_100);
    assert_eq!(s.frames(), 100);
    assert_eq!(s.frame(10), (0.25, -0.5));
}

#[test]
fn frame_outside_sample_is_silent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("st.wav");
    common::write_wav_stereo(&path, 44_100, 10, 0.25, 0.25);
    let s = decode_file(&path).unwrap();
    assert_eq!(s.frame(-1), (0.0, 0.0));
    assert_eq!(s.frame(10), (0.0, 0.0));
}

#[test]
fn rejects_non_audio_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.wav");
    fs::write(&path, b"definitely not audio").unwrap();
    assert!(decode_file(&path).is_err());
}

#[test]
fn load_samples_reports_failures_and_loads_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("samples")).unwrap();
    common::write_wav_stereo(&dir.path().join("samples/ok.wav"), 48_000, 10, 0.1, 0.1);
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "samples/ok.wav", [0, 0, 0]));
    set.banks[1].set_pad(Pad::new(0, 0, "samples/ok.wav", [0, 0, 0]));
    set.banks[0].set_pad(Pad::new(1, 0, "samples/missing.wav", [0, 0, 0]));
    let (ok, failed) = load_samples(dir.path(), &set);
    assert_eq!(ok.len(), 1);
    assert!(ok.contains_key("samples/ok.wav"));
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].0, "samples/missing.wav");
}
