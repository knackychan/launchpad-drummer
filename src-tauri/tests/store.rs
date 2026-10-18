use launchpad_drummer_lib::store::KitManager;
use lpd_core::kit::{Pad, Set};
use std::path::Path;

fn write_test_wav(path: &Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..480 {
        w.write_sample(1000i16).unwrap();
    }
    w.finalize().unwrap();
}

#[test]
fn kit_manager_saves_and_loads_set() {
    let dir = tempfile::tempdir().unwrap();
    let mut km = KitManager::new_default();
    km.set_dir(dir.path().to_path_buf());
    let mut pad = Pad::new(0, 0, "samples/kick.wav", [63, 0, 0]);
    pad.volume_db = -3.0;
    let mut set = Set::new();
    set.banks[0].set_pad(pad);
    km.save(&set).expect("save ok");

    let loaded = KitManager::load_from_dir(dir.path()).expect("load ok");
    assert_eq!(loaded.set.banks[0].pads.len(), 1);
    assert_eq!(loaded.set.banks[0].pads[0].volume_db, -3.0);
}

#[test]
fn import_sample_copies_and_decodes() {
    let set_dir = tempfile::tempdir().unwrap();
    let src_dir = tempfile::tempdir().unwrap();
    let src = src_dir.path().join("snare.wav");
    write_test_wav(&src);

    let mut km = KitManager::new_default();
    km.set_dir(set_dir.path().to_path_buf());
    let (rel_path, sample) = km.import_sample_file(&src).expect("import ok");
    assert_eq!(rel_path, "samples/snare.wav");
    assert_eq!(sample.frames(), 480);
    assert!(set_dir.path().join("samples/snare.wav").exists());
}
