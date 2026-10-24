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

#[test]
fn progress_counts_each_unique_path_once_in_first_reference_order() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("samples")).unwrap();
    common::write_wav_stereo(&dir.path().join("samples/a.wav"), 48_000, 10, 0.1, 0.1);
    common::write_wav_stereo(&dir.path().join("samples/b.wav"), 48_000, 10, 0.1, 0.1);
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "samples/b.wav", [0, 0, 0]));
    set.banks[0].set_pad(Pad::new(1, 0, "samples/a.wav", [0, 0, 0]));
    set.banks[1].set_pad(Pad::new(0, 0, "samples/b.wav", [0, 0, 0]));
    assert_eq!(
        unique_sample_paths(&set),
        ["samples/b.wav", "samples/a.wav"]
    );
    let seen = std::cell::RefCell::new(Vec::new());
    let (ok, failed) = load_samples_with_progress(dir.path(), &set, |c, t, f| {
        seen.borrow_mut().push((c, t, f))
    });
    assert_eq!(ok.len(), 2);
    assert!(failed.is_empty());
    assert_eq!(*seen.borrow(), [(0, 2, 0), (1, 2, 0), (2, 2, 0)]);
}

#[test]
fn progress_for_an_empty_set_reports_zero_of_zero_once() {
    let dir = tempfile::tempdir().unwrap();
    let seen = std::cell::RefCell::new(Vec::new());
    let (ok, failed) = load_samples_with_progress(dir.path(), &Set::new(), |c, t, f| {
        seen.borrow_mut().push((c, t, f))
    });
    assert!(ok.is_empty() && failed.is_empty());
    assert_eq!(*seen.borrow(), [(0, 0, 0)]);
}

#[test]
fn a_missing_file_completes_and_counts_as_failed_while_the_rest_loads() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("samples")).unwrap();
    common::write_wav_stereo(&dir.path().join("samples/ok.wav"), 48_000, 10, 0.1, 0.1);
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "samples/missing.wav", [0, 0, 0]));
    set.banks[0].set_pad(Pad::new(1, 0, "samples/ok.wav", [0, 0, 0]));
    let last = std::cell::Cell::new((0, 0, 0));
    let (ok, failed) = load_samples_with_progress(dir.path(), &set, |c, t, f| last.set((c, t, f)));
    assert_eq!(last.get(), (2, 2, 1));
    assert_eq!(ok.len(), 1);
    assert_eq!(failed.len(), 1);
}
