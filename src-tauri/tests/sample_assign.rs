//! Finding 4: assigning a sample decodes it, installs it and makes the pad playable at once.

mod common;

use common::{drain, rig, write_wav, Rig};
use lpd_core::audio::Command;

fn open_kit(tmp: &std::path::Path) -> Rig {
    let r = rig(1024);
    r.state
        .new_set_dir(tmp.join("kit").to_str().unwrap())
        .unwrap();
    r
}

fn triggers(r: &mut Rig) -> usize {
    drain(&mut r.commands)
        .iter()
        .filter(|c| matches!(c, Command::Trigger { .. }))
        .count()
}

#[test]
fn a_set_relative_sample_is_playable_immediately() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = open_kit(tmp.path());
    write_wav(&tmp.path().join("kit/samples/snare.wav"), 4_800);
    drain(&mut r.commands);

    let set = r
        .state
        .assign_sample(0, 2, 3, "samples/snare.wav", Some([1, 2, 3]))
        .unwrap();
    let pad = set.banks[0].pad(2, 3).expect("pad created");
    assert_eq!(pad.sample, "samples/snare.wav");
    assert_eq!(pad.color, [1, 2, 3]);

    r.state.trigger_pad(2, 3); // active bank is 0; no reload, no save
    assert_eq!(triggers(&mut r), 1, "playable without reloading");
    let peaks = r
        .state
        .sample_peaks("samples/snare.wav", 32)
        .expect("waveform");
    assert_eq!(peaks.length_ms, 100.0);
}

#[test]
fn an_external_file_is_copied_into_the_set_and_assigned() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = open_kit(tmp.path());
    let outside = tmp.path().join("elsewhere/kick.wav");
    write_wav(&outside, 4_800);
    drain(&mut r.commands);

    let set = r
        .state
        .assign_sample(0, 0, 0, outside.to_str().unwrap(), None)
        .unwrap();
    assert_eq!(set.banks[0].pad(0, 0).unwrap().sample, "samples/kick.wav");
    assert!(tmp.path().join("kit/samples/kick.wav").exists());
    assert!(outside.exists(), "the original is left alone");
    r.state.trigger_pad(0, 0);
    assert_eq!(triggers(&mut r), 1);
}

#[test]
fn reassigning_keeps_the_pad_settings_and_is_one_undo_step() {
    let tmp = tempfile::tempdir().unwrap();
    let r = open_kit(tmp.path());
    write_wav(&tmp.path().join("kit/samples/a.wav"), 480);
    write_wav(&tmp.path().join("kit/samples/b.wav"), 480);
    r.state
        .assign_sample(0, 1, 1, "samples/a.wav", None)
        .unwrap();
    let mut pad = r.state.get_current_set().banks[0]
        .pad(1, 1)
        .unwrap()
        .clone();
    pad.volume_db = -9.0;
    r.state.update_pad(0, pad, None);

    let set = r
        .state
        .assign_sample(0, 1, 1, "samples/b.wav", None)
        .unwrap();
    let pad = set.banks[0].pad(1, 1).unwrap();
    assert_eq!(
        (pad.sample.as_str(), pad.volume_db),
        ("samples/b.wav", -9.0)
    );

    let undone = r.state.undo().unwrap();
    assert_eq!(undone.banks[0].pad(1, 1).unwrap().sample, "samples/a.wav");
}

#[test]
fn a_bad_file_leaves_the_pad_playable_and_says_why() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = open_kit(tmp.path());
    write_wav(&tmp.path().join("kit/samples/good.wav"), 480);
    r.state
        .assign_sample(0, 0, 0, "samples/good.wav", None)
        .unwrap();
    std::fs::write(tmp.path().join("kit/samples/bad.wav"), b"not audio").unwrap();
    drain(&mut r.commands);

    for bad in ["samples/bad.wav", "samples/none.wav", "../escape.wav", ""] {
        let error = r.state.assign_sample(0, 0, 0, bad, None).unwrap_err();
        assert!(!error.is_empty(), "{bad}");
    }
    let set = r.state.get_current_set();
    assert_eq!(set.banks[0].pad(0, 0).unwrap().sample, "samples/good.wav");
    r.state.trigger_pad(0, 0);
    assert_eq!(triggers(&mut r), 1, "the old sample still plays");
}

#[test]
fn assignment_needs_a_set_folder() {
    let r = rig(1024);
    let error = r
        .state
        .assign_sample(0, 0, 0, "samples/a.wav", None)
        .unwrap_err();
    assert!(error.contains("set"), "{error}");
    assert!(r.state.begin_assign(9, 0, 0).is_err());
}

#[test]
fn a_result_for_a_replaced_set_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = open_kit(tmp.path());
    write_wav(&tmp.path().join("kit/samples/a.wav"), 480);
    let ticket = r.state.begin_assign(0, 0, 0).unwrap();
    let (rel, sample) =
        launchpad_drummer_lib::store::prepare_sample(&ticket.dir, "samples/a.wav").unwrap();

    // The user opens another set while the worker decodes.
    r.state
        .new_set_dir(tmp.path().join("second").to_str().unwrap())
        .unwrap();
    let error = r
        .state
        .finish_assign(&ticket, &rel, sample, None)
        .unwrap_err();
    assert!(error.contains("changed"), "{error}");
    assert!(r.state.get_current_set().banks[0].pad(0, 0).is_none());
    assert!(r.state.sample_peaks("samples/a.wav", 16).is_none());
    drain(&mut r.commands);
    r.state.trigger_pad(0, 0);
    assert_eq!(triggers(&mut r), 0);
}

#[test]
fn a_late_result_never_overwrites_a_newer_assignment() {
    let tmp = tempfile::tempdir().unwrap();
    let r = open_kit(tmp.path());
    write_wav(&tmp.path().join("kit/samples/old.wav"), 480);
    write_wav(&tmp.path().join("kit/samples/new.wav"), 480);

    // Two requests for the same pad; the newer worker finishes first.
    let first = r.state.begin_assign(0, 4, 4).unwrap();
    let second = r.state.begin_assign(0, 4, 4).unwrap();
    let (rel_old, old) =
        launchpad_drummer_lib::store::prepare_sample(&first.dir, "samples/old.wav").unwrap();
    let (rel_new, new) =
        launchpad_drummer_lib::store::prepare_sample(&second.dir, "samples/new.wav").unwrap();

    r.state.finish_assign(&second, &rel_new, new, None).unwrap();
    let error = r
        .state
        .finish_assign(&first, &rel_old, old, None)
        .unwrap_err();
    assert!(error.contains("newer"), "{error}");
    let set = r.state.get_current_set();
    assert_eq!(set.banks[0].pad(4, 4).unwrap().sample, "samples/new.wav");
    assert!(r.state.sample_peaks("samples/old.wav", 16).is_none());

    // A different pad is not affected by pad (4, 4)'s requests.
    let other = r.state.begin_assign(0, 5, 5).unwrap();
    let again = r.state.begin_assign(0, 4, 4).unwrap();
    let (rel, sample) =
        launchpad_drummer_lib::store::prepare_sample(&other.dir, "samples/old.wav").unwrap();
    r.state.finish_assign(&other, &rel, sample, None).unwrap();
    drop(again);
}

#[test]
fn import_installs_the_sample_without_assigning_it() {
    let tmp = tempfile::tempdir().unwrap();
    let r = open_kit(tmp.path());
    let src = tmp.path().join("loose/hat.wav");
    write_wav(&src, 480);
    let rel = r.state.import_sample(src.to_str().unwrap()).unwrap();
    assert_eq!(rel, "samples/hat.wav");
    assert!(r.state.sample_peaks(&rel, 16).is_some());
    assert!(r
        .state
        .get_current_set()
        .banks
        .iter()
        .all(|b| b.pads.is_empty()));
}
