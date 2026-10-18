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
