use lpd_core::audio::*;
use std::sync::Arc;

const SR: u32 = 48_000;

fn dc(frames: usize, value: f32) -> Arc<Sample> {
    Arc::new(Sample {
        rate: SR,
        data: vec![value; frames * 2],
    })
}

fn trigger(e: &mut Engine, pad: u16, sample: &Arc<Sample>, group: u8) {
    e.handle(Command::Trigger {
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
}

fn render(e: &mut Engine, frames: usize) {
    let mut out = vec![0.0; frames * 2];
    e.render(&mut out);
}

fn last_levels(taps: &mut Taps) -> Option<Levels> {
    let mut last = None;
    while let Ok(l) = taps.levels.pop() {
        last = Some(l);
    }
    last
}

#[test]
fn a_sample_on_group_three_shows_only_in_group_three_and_master() {
    let (mut e, _events, mut taps) = Engine::with_taps(SR);
    trigger(&mut e, 1, &dc(48_000, 0.5), 3);
    render(&mut e, 256);
    let levels = last_levels(&mut taps).expect("one block was rendered");
    assert!(levels.groups[3] > 0.0);
    for (i, g) in levels.groups.iter().enumerate() {
        if i != 3 {
            assert_eq!(*g, 0.0, "group {i}");
        }
    }
    assert!(levels.master[0] > 0.0 && levels.master[1] > 0.0);
}

#[test]
fn a_muted_group_reads_zero_everywhere() {
    let (mut e, _events, mut taps) = Engine::with_taps(SR);
    e.handle(Command::SetGroup {
        group: 3,
        params: GroupParams {
            mute: true,
            ..Default::default()
        },
    });
    trigger(&mut e, 1, &dc(48_000, 0.5), 3);
    render(&mut e, 256);
    let levels = last_levels(&mut taps).unwrap();
    assert_eq!(levels.groups, [0.0; 8]);
    assert_eq!(levels.master, [0.0; 2]);
}

#[test]
fn solo_silences_the_other_groups_in_the_levels() {
    let (mut e, _events, mut taps) = Engine::with_taps(SR);
    e.handle(Command::SetGroup {
        group: 1,
        params: GroupParams {
            solo: true,
            ..Default::default()
        },
    });
    trigger(&mut e, 1, &dc(48_000, 0.5), 3);
    trigger(&mut e, 2, &dc(48_000, 0.5), 1);
    render(&mut e, 256);
    let levels = last_levels(&mut taps).unwrap();
    assert_eq!(levels.groups[3], 0.0);
    assert!(levels.groups[1] > 0.0);
}

#[test]
fn the_scope_receives_one_value_per_frame() {
    let (mut e, _events, mut taps) = Engine::with_taps(SR);
    trigger(&mut e, 1, &dc(48_000, 0.5), 0);
    render(&mut e, 256);
    assert_eq!(taps.scope.slots(), 256);
    let first = taps.scope.pop().unwrap();
    assert!(
        (first - 0.5).abs() < 1e-3,
        "mono sum of two 0.5 channels: {first}"
    );
}

#[test]
fn a_full_scope_queue_drops_the_overflow_and_keeps_rendering() {
    let (mut e, _events, taps) = Engine::with_taps(SR);
    trigger(&mut e, 1, &dc(480_000, 0.5), 0);
    // 8192 capacity: render more than that without draining.
    for _ in 0..40 {
        render(&mut e, 512);
    }
    assert_eq!(taps.scope.slots(), SCOPE_CAPACITY);
    // The levels queue is bounded too: 40 blocks fit, 600 blocks do not overflow it.
    for _ in 0..600 {
        render(&mut e, 64);
    }
    assert_eq!(taps.levels.slots(), LEVELS_CAPACITY);
}

#[test]
fn an_engine_without_taps_is_unchanged() {
    let (mut e, _events) = Engine::new(SR);
    trigger(&mut e, 1, &dc(48_000, 0.5), 0);
    render(&mut e, 256);
}
