use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use lpd_core::audio::*;
use std::sync::Arc;

#[global_allocator]
static ALLOC: AllocDisabler = AllocDisabler;

#[test]
fn audition_paths_never_allocate_or_free() {
    let (mut engine, _) = Engine::new(48_000);
    let samples: Vec<_> = (0..4)
        .map(|_| {
            Arc::new(Sample {
                rate: 48_000,
                data: vec![0.25; 4096],
            })
        })
        .collect();
    let mut out = [0.0; 1024];
    assert_no_alloc(|| {
        for sample in &samples {
            engine.handle(Command::Audition {
                sample: sample.clone(),
            });
            engine.render(&mut out);
            engine.render(&mut out);
        }
        engine.handle(Command::StopAudition);
        engine.render(&mut out);
        engine.handle(Command::Audition {
            sample: samples[0].clone(),
        });
        engine.handle(Command::StopAll);
        engine.render(&mut out);
    });
}

#[test]
fn tapped_engine_never_allocates_even_with_full_queues() {
    let (mut engine, _events, mut taps) = Engine::with_taps(48_000);
    let sample = Arc::new(Sample {
        rate: 48_000,
        data: vec![0.25; 48_000 * 2],
    });
    let mut out = vec![0.0f32; 512 * 2];
    let trigger = |engine: &mut Engine, pad: u16, group: u8| {
        engine.handle(Command::Trigger {
            pad,
            sample: sample.clone(),
            params: PlayParams {
                gain: 1.0,
                pitch_semitones: 0.0,
                start_frame: 0,
                end_frame: sample.frames(),
                choke_group: None,
                mix_group: group,
                hold: false,
            },
        });
    };
    // Queues have room: trigger plus 4 blocks.
    assert_no_alloc(|| {
        trigger(&mut engine, 1, 0);
        trigger(&mut engine, 2, 5);
        for _ in 0..4 {
            engine.render(&mut out);
        }
    });
    // Nothing drained: both queues fill up and stay full while rendering goes on.
    assert_no_alloc(|| {
        for _ in 0..40 {
            engine.render(&mut out);
        }
    });
    assert_eq!(taps.scope.slots(), SCOPE_CAPACITY);
    // Drained queues work again.
    while taps.scope.pop().is_ok() {}
    while taps.levels.pop().is_ok() {}
    assert_no_alloc(|| {
        for _ in 0..4 {
            engine.render(&mut out);
        }
    });
}

#[test]
fn audio_callback_never_allocates() {
    let (mut engine, mut events) = Engine::new(48_000);
    let (mut tx, mut rx) = rtrb::RingBuffer::new(256);
    let mut scratch = host::scratch();
    let mut out = vec![0.0f32; 512 * 2];
    // Kept alive here so the audio thread only ever decrements the refcount.
    let sample = Arc::new(Sample {
        rate: 44_100,
        data: vec![0.25; 44_100 * 2],
    });
    for pad in 0..40u16 {
        let params = PlayParams {
            gain: 1.0,
            pitch_semitones: 0.0,
            start_frame: 0,
            end_frame: sample.frames(),
            choke_group: if pad % 2 == 0 { Some(1) } else { None },
            mix_group: (pad % 8) as u8,
            hold: pad % 3 == 0,
        };
        tx.push(Command::Trigger {
            pad,
            sample: sample.clone(),
            params,
        })
        .unwrap();
    }
    tx.push(Command::SetGroup {
        group: 2,
        params: GroupParams {
            lowpass_hz: 1_000.0,
            ..GroupParams::default()
        },
    })
    .unwrap();
    tx.push(Command::Release { pad: 3 }).unwrap();
    tx.push(Command::SetMasterGain(0.8)).unwrap();
    assert_no_alloc(|| {
        for _ in 0..200 {
            host::process(&mut engine, &mut rx, &mut scratch, &mut out, 2);
        }
    });
    tx.push(Command::StopAll).unwrap();
    assert_no_alloc(|| {
        for _ in 0..20 {
            host::process(&mut engine, &mut rx, &mut scratch, &mut out, 2);
        }
    });
    while events.pop().is_ok() {}
}

#[test]
fn keys_paths_never_allocate_or_free() {
    use lpd_core::instrument::{BasicKeys, KeysSound};
    let (mut engine, _) = Engine::new(48_000);
    // Created before the guard, as the host does.
    let replacement: Box<dyn lpd_core::instrument::Instrument> = Box::new(BasicKeys::new(48_000));
    let mut out = [0.0; 2048];
    assert_no_alloc(|| {
        engine.handle(Command::KeysSound(KeysSound::SynthPad));
        engine.handle(Command::KeysRoute {
            gain: 0.5,
            group: Some(3),
        });
        engine.handle(Command::KeysNoteOn {
            note: 60,
            velocity: 100,
        });
        engine.handle(Command::KeysPitch(1.5));
        engine.handle(Command::KeysMod(0.7));
        for _ in 0..4 {
            engine.render(&mut out);
        }
        engine.handle(Command::KeysNoteOff { note: 60 });
        engine.handle(Command::KeysRoute {
            gain: 0.5,
            group: None,
        });
        engine.handle(Command::KeysNoteOn {
            note: 64,
            velocity: 90,
        });
        engine.render(&mut out);
        engine.handle(Command::KeysAllOff);
        engine.handle(Command::SetInstrument(replacement));
        engine.render(&mut out);
    });
}

#[test]
fn a_full_trash_queue_leaks_instead_of_freeing() {
    use lpd_core::instrument::{BasicKeys, Instrument};
    let (mut engine, _) = Engine::new(48_000);
    // Nobody collects: the queue (capacity 4) fills up; the fifth and later boxes are leaked.
    let mut boxes: Vec<Option<Box<dyn Instrument>>> = (0..8)
        .map(|_| Some(Box::new(BasicKeys::new(48_000)) as Box<dyn Instrument>))
        .collect();
    assert_no_alloc(|| {
        for slot in boxes.iter_mut() {
            engine.handle(Command::SetInstrument(slot.take().unwrap()));
        }
    });
}
