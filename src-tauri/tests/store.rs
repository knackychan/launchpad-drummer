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
    r.state.new_set(kit_dir.to_str().unwrap()).unwrap();
    let src = tmp.path().join("loose/snare.wav");
    write_wav(&src, 480);

    let rel_path = r.state.import_sample(src.to_str().unwrap()).unwrap();
    assert_eq!(rel_path, "samples/snare.wav");
    assert!(kit_dir.join("samples/snare.wav").exists());
    let peaks = r.state.sample_peaks(&rel_path, 16).expect("loaded");
    assert_eq!(peaks.length_ms, 10.0);
}

#[test]
fn missing_samples_lists_failed_and_unloaded_pads_top_row_first() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("kit");
    let mut set = Set::new();
    set.banks[0].set_pad(Pad::new(0, 0, "samples/ok.wav", [1, 1, 1]));
    set.banks[0].set_pad(Pad::new(5, 0, "samples/gone.wav", [1, 1, 1]));
    set.banks[0].set_pad(Pad::new(2, 7, "samples/gone.wav", [1, 1, 1]));
    set.banks[2].set_pad(Pad::new(1, 3, "samples/bad.wav", [1, 1, 1]));
    kit::save(&dir, &set).unwrap();
    write_wav(&dir.join("samples/ok.wav"), 480);
    std::fs::write(dir.join("samples/bad.wav"), b"not audio").unwrap();

    let r = rig(1024);
    r.state.open_set(dir.to_str().unwrap()).unwrap();
    let missing: Vec<_> = r
        .state
        .missing_samples()
        .into_iter()
        .map(|m| (m.bank, m.x, m.y, m.sample))
        .collect();
    assert_eq!(
        missing,
        [
            (0, 2, 7, "samples/gone.wav".to_string()),
            (0, 5, 0, "samples/gone.wav".to_string()),
            (2, 1, 3, "samples/bad.wav".to_string()),
        ]
    );
}

#[test]
fn open_set_failure_changes_nothing_and_names_the_set() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(1024);
    let good = tmp.path().join("good");
    r.state.new_set(good.to_str().unwrap()).unwrap();
    r.state.rename_bank(0, "Mine".into());
    let bad = tmp.path().join("Broken Kit");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join(kit::SET_FILE), "{ nope").unwrap();

    let error = r.state.open_set(bad.to_str().unwrap()).unwrap_err();
    assert_eq!(
        error,
        "Couldn't open Broken Kit — set.json and backup are unreadable. Files left untouched."
    );
    assert_eq!(r.state.get_current_set().banks[0].name, "Mine");
    assert_eq!(
        r.state.kit_manager.lock().unwrap().dir.as_deref(),
        Some(good.as_path())
    );
    assert_eq!(std::fs::read(bad.join(kit::SET_FILE)).unwrap(), b"{ nope");
}

#[test]
fn new_and_open_set_report_the_set_and_update_startup() {
    use launchpad_drummer_lib::startup::Startup;
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(1024);
    assert_eq!(r.state.get_startup(), Startup::FirstRun);
    let dir = tmp.path().join("Kit A");
    let created = r.state.new_set(dir.to_str().unwrap()).unwrap();
    assert_eq!(created.name, "Kit A");
    assert_eq!(created.path, dir.display().to_string());
    assert!(!created.from_backup);
    assert_eq!(created.set, Set::new());
    assert_eq!(
        r.state.get_startup(),
        Startup::Loaded {
            path: dir.display().to_string(),
            name: "Kit A".into(),
            from_backup: false,
            backup_saved_at: None,
        }
    );
    assert_eq!(
        r.state.get_config().last_set.as_deref(),
        dir.to_str(),
        "remembered for the next launch"
    );
}

#[test]
fn restore_opens_the_last_set_from_the_config() {
    use launchpad_drummer_lib::config::{self, AppConfig};
    use launchpad_drummer_lib::startup::Startup;
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Live");
    let mut set = Set::new();
    set.banks[0].name = "Saved".into();
    kit::save(&dir, &set).unwrap();
    let config_path = tmp.path().join("cfg").join("config.json");
    let c = AppConfig {
        last_set: Some(dir.display().to_string()),
        library_dir: Some("D:\\Lib".into()),
        ..Default::default()
    };
    config::save(&config_path, &c).unwrap();

    let r = rig(1024);
    let startup = r.state.restore(config_path.clone());
    assert!(matches!(startup, Startup::Loaded { ref name, .. } if name == "Live"));
    assert_eq!(r.state.get_startup(), startup);
    assert_eq!(r.state.get_current_set().banks[0].name, "Saved");
    assert_eq!(r.state.get_config(), c);

    // Config changes are written back to the same file.
    r.state.set_library_dir(None).unwrap();
    assert_eq!(config::load(&config_path).library_dir, None);

    // A missing last set: nothing is opened and saving stays off.
    let r = rig(1024);
    std::fs::remove_dir_all(&dir).unwrap();
    let startup = r.state.restore(config_path);
    assert_eq!(
        startup,
        Startup::NotFound {
            path: dir.display().to_string()
        }
    );
    assert_eq!(r.state.kit_manager.lock().unwrap().dir, None);
    assert!(r.state.save_set().is_err());
}

#[test]
fn the_default_set_is_made_once_and_then_reopened_as_edited() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(1024);
    let dir = tmp.path().join("Starter");
    let created = r.state.open_default_set(&dir).unwrap();
    assert_eq!(created.set, kit::starter_set());
    assert_eq!(created.set.pattern.lanes.len(), 5);
    assert!(dir.join("samples/kick_808.wav").exists());
    assert!(r.state.missing_samples().is_empty());
    let loaded = KitManager::load_from_dir(&dir).unwrap();
    assert_eq!(loaded.set, created.set);
    assert!(loaded.failed_samples.is_empty());

    let mut edited = created.set.clone();
    edited.banks[0].name = "MINE".into();
    kit::save(&dir, &edited).unwrap();
    let again = r.state.open_default_set(&dir).unwrap();
    assert_eq!(again.set.banks[0].name, "MINE", "an existing default set is opened, not rebuilt");
}
