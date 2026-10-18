use lpd_core::audio::*;
use std::sync::Arc;

fn constant(frames: usize, value: f32) -> Arc<Sample> {
    Arc::new(Sample {
        rate: 48_000,
        data: vec![value; frames * 2],
    })
}

fn params(start: usize, end: usize) -> PlayParams {
    PlayParams {
        gain: 1.0,
        pitch_semitones: 0.0,
        start_frame: start,
        end_frame: end,
        choke_group: None,
        mix_group: 0,
        hold: false,
    }
}

fn render(v: &mut Voice, frames: usize) -> (Vec<f32>, bool) {
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    let alive = v.render_add(&mut l, &mut r);
    assert_eq!(l, r);
    (l, alive)
}

#[test]
fn plays_with_gain() {
    let mut v = Voice::idle();
    let mut p = params(0, 1000);
    p.gain = 0.5;
    v.start(1, constant(1000, 1.0), p, 1.0, 0);
    let (out, alive) = render(&mut v, 100);
    assert!(alive);
    assert!(out.iter().all(|&x| (x - 0.5).abs() < 1e-6));
}

#[test]
fn finishes_at_end_of_range() {
    let mut v = Voice::idle();
    v.start(1, constant(100, 1.0), params(0, 100), 1.0, 0);
    let (out, alive) = render(&mut v, 200);
    assert!(!alive);
    assert!(!v.is_active());
    assert!(out[99] > 0.0);
    assert!(out[100..].iter().all(|&x| x == 0.0));
}

#[test]
fn rate_ratio_two_plays_twice_as_fast() {
    let mut v = Voice::idle();
    v.start(1, constant(100, 1.0), params(0, 100), 2.0, 0);
    let (out, _) = render(&mut v, 200);
    assert!(out[49] > 0.0);
    assert!(out[50..].iter().all(|&x| x == 0.0));
}

#[test]
fn trim_start_begins_inside_the_sample() {
    let data: Vec<f32> = (0..1000).flat_map(|i| [i as f32, i as f32]).collect();
    let mut v = Voice::idle();
    v.start(
        1,
        Arc::new(Sample { rate: 48_000, data }),
        params(10, 500),
        1.0,
        0,
    );
    let (out, _) = render(&mut v, 3);
    assert_eq!(&out[..3], &[10.0, 11.0, 12.0]);
}

#[test]
fn end_fade_avoids_a_click() {
    let mut v = Voice::idle();
    v.start(1, constant(1000, 1.0), params(0, 1000), 1.0, 0);
    let (out, _) = render(&mut v, 1000);
    assert!(out[999] < 0.05, "last frame {}", out[999]);
    assert!(out[800] > 0.99);
}

#[test]
fn fade_out_reaches_silence_and_stops() {
    let mut v = Voice::idle();
    v.start(1, constant(10_000, 1.0), params(0, 10_000), 1.0, 0);
    v.fade_out(48);
    assert!(v.is_fading());
    let (out, alive) = render(&mut v, 100);
    assert!(!alive);
    assert!(out.windows(2).take(48).all(|w| w[1] <= w[0]));
    assert!(out[49..].iter().all(|&x| x == 0.0));
}

#[test]
fn a_slower_fade_never_overrides_a_faster_one() {
    let mut v = Voice::idle();
    v.start(1, constant(10_000, 1.0), params(0, 10_000), 1.0, 0);
    v.fade_out(10);
    v.fade_out(1000);
    let (_, alive) = render(&mut v, 20);
    assert!(!alive);
}
