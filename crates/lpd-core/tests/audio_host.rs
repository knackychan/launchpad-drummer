use lpd_core::audio::*;
use std::sync::Arc;

fn setup() -> (
    Engine,
    rtrb::Producer<Command>,
    rtrb::Consumer<Command>,
    Arc<Sample>,
) {
    let (engine, _events) = Engine::new(48_000);
    let (tx, rx) = rtrb::RingBuffer::new(64);
    let sample = Arc::new(Sample {
        rate: 48_000,
        data: [0.2, 0.6].repeat(48_000),
    });
    (engine, tx, rx, sample)
}

fn trigger(tx: &mut rtrb::Producer<Command>, s: &Arc<Sample>) {
    let params = PlayParams {
        gain: 1.0,
        pitch_semitones: 0.0,
        start_frame: 0,
        end_frame: s.frames(),
        choke_group: None,
        mix_group: 0,
        hold: false,
    };
    tx.push(Command::Trigger {
        pad: 0,
        sample: s.clone(),
        params,
    })
    .unwrap();
}

#[test]
fn stereo_device_gets_left_and_right() {
    let (mut e, mut tx, mut rx, s) = setup();
    trigger(&mut tx, &s);
    let mut scratch = host::scratch();
    let mut out = vec![0.0; 16];
    host::process(&mut e, &mut rx, &mut scratch, &mut out, 2);
    assert!((out[0] - 0.2).abs() < 1e-4 && (out[1] - 0.6).abs() < 1e-4);
}

#[test]
fn mono_device_gets_mixdown() {
    let (mut e, mut tx, mut rx, s) = setup();
    trigger(&mut tx, &s);
    let mut scratch = host::scratch();
    let mut out = vec![0.0; 8];
    host::process(&mut e, &mut rx, &mut scratch, &mut out, 1);
    assert!((out[0] - 0.4).abs() < 1e-4);
}

#[test]
fn extra_channels_are_silent() {
    let (mut e, mut tx, mut rx, s) = setup();
    trigger(&mut tx, &s);
    let mut scratch = host::scratch();
    let mut out = vec![9.0; 4 * 8];
    host::process(&mut e, &mut rx, &mut scratch, &mut out, 4);
    assert!((out[0] - 0.2).abs() < 1e-4 && (out[1] - 0.6).abs() < 1e-4);
    assert_eq!((out[2], out[3]), (0.0, 0.0));
}

#[test]
fn buffers_larger_than_scratch_are_filled() {
    let (mut e, mut tx, mut rx, s) = setup();
    trigger(&mut tx, &s);
    let mut scratch = host::scratch();
    let mut out = vec![0.0; 2 * (MAX_BLOCK * 3 + 7)];
    host::process(&mut e, &mut rx, &mut scratch, &mut out, 2);
    assert!((out[out.len() - 2] - 0.2).abs() < 1e-4);
}
