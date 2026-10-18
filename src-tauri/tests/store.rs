mod common;

use common::{rig, write_wav};
use launchpad_drummer_lib::store::{prepare_sample, KitManager};
use lpd_core::kit::{self, Pad, Set};

#[test]
fn a_saved_set_loads_back_through_the_kit_manager() {
    let dir = tempfile::tempdir().unwrap();
    let mut pad = Pad::new(0, 0, "samples/kick.wav", [63, 0, 0]);
    pad.volume_db = -3.0;
    let mut set = Set::new();
    set.banks[0].set_pad(pad);
    kit::save(dir.path(), &set).expect("save ok");

    let loaded = KitManager::load_from_dir(dir.path()).expect("load ok");
    assert_eq!(loaded.set.banks[0].pads.len(), 1);
    assert_eq!(loaded.set.banks[0].pads[0].volume_db, -3.0);
}

#[test]
fn prepare_sample_copies_and_decodes_an_outside_file() {
    let set_dir = tempfile::tempdir().unwrap();
    let src_dir = tempfile::tempdir().unwrap();
    let src = src_dir.path().join("snare.wav");
    write_wav(&src, 480);

    let (rel_path, sample) = prepare_sample(set_dir.path(), src.to_str().unwrap()).unwrap();
    assert_eq!(rel_path, "samples/snare.wav");
    assert_eq!(sample.frames(), 480);
    assert!(set_dir.path().join("samples/snare.wav").exists());
}

#[test]
fn import_sample_copies_into_the_open_set() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(1024);
    let kit_dir = tmp.path().join("kit");
    r.state.new_set_dir(kit_dir.to_str().unwrap()).unwrap();
    let src = tmp.path().join("loose/snare.wav");
    write_wav(&src, 480);

    let rel_path = r.state.import_sample(src.to_str().unwrap()).unwrap();
    assert_eq!(rel_path, "samples/snare.wav");
    assert!(kit_dir.join("samples/snare.wav").exists());
    let peaks = r.state.sample_peaks(&rel_path, 16).expect("loaded");
    assert_eq!(peaks.length_ms, 10.0);
}
