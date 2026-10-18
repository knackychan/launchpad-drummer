use lpd_core::audio::Sample;
use lpd_core::kit::*;

fn one_second() -> Sample {
    Sample {
        rate: 48_000,
        data: vec![0.0; 48_000 * 2],
    }
}

#[test]
fn pad_settings_convert_to_play_params() {
    let mut pad = Pad::new(2, 3, "s.wav", [0, 0, 0]);
    pad.volume_db = -6.0;
    pad.pitch_semitones = 2.5;
    pad.trim_start_ms = 10.0;
    pad.trim_end_ms = Some(20.0);
    pad.mode = PadMode::Hold;
    pad.choke_group = Some(4);
    let p = play_params(&pad, &one_second());
    assert!((p.gain - 0.501).abs() < 0.001);
    assert_eq!(p.pitch_semitones, 2.5);
    assert_eq!((p.start_frame, p.end_frame), (480, 960));
    assert!(p.hold);
    assert_eq!(p.choke_group, Some(4));
    assert_eq!(p.mix_group, 3);
}

#[test]
fn trim_is_clamped_to_the_sample() {
    let mut pad = Pad::new(0, 0, "s.wav", [0, 0, 0]);
    pad.trim_end_ms = Some(5_000.0);
    let p = play_params(&pad, &one_second());
    assert_eq!((p.start_frame, p.end_frame), (0, 48_000));
    pad.trim_start_ms = 500.0;
    pad.trim_end_ms = Some(100.0);
    let p = play_params(&pad, &one_second());
    assert_eq!((p.start_frame, p.end_frame), (24_000, 24_000));
}

#[test]
fn group_converts_to_group_params() {
    let mut g = Set::new().mixer.groups.remove(0);
    g.volume_db = -6.0;
    g.pan = 0.5;
    g.pitch_semitones = -3.0;
    g.lowpass_hz = 2_000.0;
    g.mute = true;
    let p = group_params(&g);
    assert!((p.gain - 0.501).abs() < 0.001);
    assert_eq!(
        (p.pan, p.pitch_semitones, p.lowpass_hz, p.mute, p.solo),
        (0.5, -3.0, 2_000.0, true, false)
    );
}
