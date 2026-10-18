use launchpad_drummer_lib::store::list_dir;

#[test]
fn directory_listing_is_shallow_filtered_and_sorted() {
    let dir = tempfile::tempdir().unwrap();
    for folder in ["Zed", "alpha", ".hidden"] {
        std::fs::create_dir(dir.path().join(folder)).unwrap();
    }
    for file in ["z.WAV", "A.flac", ".secret.wav", "notes.txt"] {
        std::fs::write(dir.path().join(file), []).unwrap();
    }
    let rows = list_dir(dir.path()).unwrap();
    assert_eq!(
        rows.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        ["alpha", "Zed", "A.flac", "z.WAV"]
    );
    assert!(rows[0].is_dir && !rows[2].is_dir);
    assert!(list_dir(&dir.path().join("missing")).is_err());
}
