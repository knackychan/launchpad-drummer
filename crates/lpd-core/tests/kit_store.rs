use lpd_core::kit::*;
use std::fs;

fn set_named(name: &str) -> Set {
    let mut set = Set::new();
    set.banks[0].name = name.to_string();
    set
}

#[test]
fn save_then_load_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), &set_named("A")).unwrap();
    let loaded = load(dir.path()).unwrap();
    assert_eq!(loaded.set.banks[0].name, "A");
    assert!(!loaded.from_backup);
    assert!(!dir.path().join("set.json.tmp").exists());
}

#[test]
fn second_save_keeps_previous_version_as_backup() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), &set_named("A")).unwrap();
    save(dir.path(), &set_named("B")).unwrap();
    let bak = fs::read_to_string(dir.path().join(BACKUP_FILE)).unwrap();
    assert_eq!(Set::from_json(&bak).unwrap().banks[0].name, "A");
    assert_eq!(load(dir.path()).unwrap().set.banks[0].name, "B");
}

#[test]
fn corrupt_set_loads_backup_and_keeps_broken_file() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), &set_named("A")).unwrap();
    save(dir.path(), &set_named("B")).unwrap();
    fs::write(dir.path().join(SET_FILE), "{ broken").unwrap();
    let loaded = load(dir.path()).unwrap();
    assert!(loaded.from_backup);
    assert_eq!(loaded.set.banks[0].name, "A");
    assert_eq!(
        fs::read_to_string(dir.path().join(BROKEN_FILE)).unwrap(),
        "{ broken"
    );
    assert!(!dir.path().join(SET_FILE).exists());
}

#[test]
fn load_fails_when_nothing_is_readable() {
    let dir = tempfile::tempdir().unwrap();
    assert!(load(dir.path()).is_err());
    fs::write(dir.path().join(SET_FILE), "x").unwrap();
    fs::write(dir.path().join(BACKUP_FILE), "y").unwrap();
    assert!(load(dir.path()).is_err());
    assert_eq!(fs::read_to_string(dir.path().join(SET_FILE)).unwrap(), "x");
}

#[test]
fn import_copies_into_samples_with_collision_suffix() {
    let set_dir = tempfile::tempdir().unwrap();
    let src_dir = tempfile::tempdir().unwrap();
    let src = src_dir.path().join("kick.wav");
    fs::write(&src, b"RIFF").unwrap();
    assert_eq!(
        import_sample(set_dir.path(), &src).unwrap(),
        "samples/kick.wav"
    );
    assert_eq!(
        import_sample(set_dir.path(), &src).unwrap(),
        "samples/kick-2.wav"
    );
    assert_eq!(
        import_sample(set_dir.path(), &src).unwrap(),
        "samples/kick-3.wav"
    );
    assert_eq!(
        fs::read(set_dir.path().join("samples/kick-2.wav")).unwrap(),
        b"RIFF"
    );
}
