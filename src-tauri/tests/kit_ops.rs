mod common;
use common::{rig, write_wav};
use lpd_core::kit::Pad;

#[test]
fn a_batch_import_is_one_undo_step_and_reports_bank_capacity() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(1024);
    r.state
        .new_set(tmp.path().join("kit").to_str().unwrap())
        .unwrap();
    r.state.edit(None, |s| {
        for y in 0..8 {
            for x in 0..8 {
                if (x, y) != (2, 0) {
                    s.banks[0].set_pad(Pad::new(x, y, "old", [1, 2, 3]));
                }
            }
        }
    });
    let before = r.state.get_current_set();
    let files: Vec<_> = (0..3)
        .map(|n| {
            let p = tmp.path().join(format!("{n}.wav"));
            write_wav(&p, 480);
            p.to_string_lossy().into_owned()
        })
        .collect();
    let report = r.state.drop_files(0, 1, 0, files).unwrap();
    assert_eq!(
        (report.imported, report.leftover, report.rejected),
        (2, 1, 0)
    );
    assert_ne!(report.set.banks[0].pad(1, 0).unwrap().sample, "old");
    assert_eq!(r.state.undo().unwrap(), before);
}

#[test]
fn locate_keeps_trim_and_empty_or_failed_imports_do_not_add_history() {
    let tmp = tempfile::tempdir().unwrap();
    let r = rig(1024);
    r.state
        .new_set(tmp.path().join("kit").to_str().unwrap())
        .unwrap();
    let report = r
        .state
        .drop_files(0, 0, 0, vec!["no.txt".into(), "missing.wav".into()])
        .unwrap();
    assert_eq!((report.imported, report.rejected), (0, 2));
    assert!(!r.state.history_state().can_undo);
    let p = tmp.path().join("found.wav");
    write_wav(&p, 480);
    assert_eq!(
        r.state
            .locate_sample(0, 0, 0, p.to_str().unwrap())
            .unwrap_err(),
        "Pad is empty"
    );
    r.state.edit(None, |s| {
        let mut p = Pad::new(0, 0, "missing.wav", [1, 2, 3]);
        p.trim_start_ms = 2.0;
        p.trim_end_ms = Some(8.0);
        s.banks[0].set_pad(p);
    });
    let set = r.state.locate_sample(0, 0, 0, p.to_str().unwrap()).unwrap();
    let pad = set.banks[0].pad(0, 0).unwrap();
    assert_eq!((pad.trim_start_ms, pad.trim_end_ms), (2.0, Some(8.0)));
    assert!(r.state.missing_samples().is_empty());
}
