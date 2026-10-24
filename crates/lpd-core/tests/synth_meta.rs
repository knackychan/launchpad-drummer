use std::path::Path;

#[test]
fn synth_meta_ts_is_up_to_date() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/lib/synth-meta.ts");
    let want = lpd_core::synth::export::ui_meta_ts();
    let have = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    if have != want {
        std::fs::write(&path, &want).unwrap();
        panic!("src/lib/synth-meta.ts was out of date and has been rewritten — run the tests again and commit it");
    }
}

#[test]
fn the_meta_describes_every_param_and_destination() {
    let meta = lpd_core::synth::export::ui_meta();
    assert_eq!(meta["params"].as_array().unwrap().len(), 109);
    assert_eq!(meta["dests"].as_array().unwrap().len(), 27);
    assert_eq!(meta["sources"][1]["color"], "#ffb21a");
    assert_eq!(meta["lcxl"][1][0], "filter.cutoff");
    assert_eq!(meta["factory"].as_array().unwrap().len(), 16);
    let cutoff = meta["params"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "filter.cutoff")
        .unwrap();
    assert_eq!(
        (
            cutoff["kind"].as_str(),
            cutoff["taper"].as_str(),
            cutoff["unit"].as_str()
        ),
        (Some("float"), Some("log"), Some("hz"))
    );
    assert_eq!(cutoff["dest"], "filter.cutoff");
    assert_eq!(cutoff["default"], 12_000.0);
    let table = meta["params"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "oscA.table")
        .unwrap();
    assert_eq!(table["labels"][4], "FM SWEEP");
}
