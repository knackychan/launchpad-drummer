use launchpad_drummer_lib::lights::LightPainter;
use lpd_core::devices::launchpad as lp;
use lpd_core::kit::{Pad, Set};
use std::collections::HashSet;
use std::time::{Duration, Instant};

fn test_set() -> Set {
    let mut set = Set::new();
    set.banks[0].lights.play_state = true;
    set.banks[0].lights.flash = true;
    set.banks[0].lights.ripple = false;
    set.banks[0].set_pad(Pad::new(0, 0, "samples/a.wav", [60, 0, 0]));
    set.banks[0].set_pad(Pad::new(1, 0, "samples/missing.wav", [0, 60, 0]));
    set
}

#[test]
fn idle_pads_dimmed_when_play_state_enabled() {
    let set = test_set();
    let mut loaded_samples = HashSet::new();
    loaded_samples.insert("samples/a.wav".to_string());
    let mut painter = LightPainter::new();
    let now = Instant::now();
    let frame = painter.paint(&set, 0, &loaded_samples, &|_b, _x, _y| false, now);
    let led_0_0 = frame
        .iter()
        .find(|(id, _)| *id == lp::grid_led(0, 0))
        .unwrap()
        .1;
    // 35% of 60 is 21
    assert!(led_0_0[0] >= 18 && led_0_0[0] <= 24);
    assert_eq!(led_0_0[1], 0);

    // When sounding:
    let frame_sounding = painter.paint(&set, 0, &loaded_samples, &|_b, x, y| x == 0 && y == 0, now);
    let led_sounding = frame_sounding
        .iter()
        .find(|(id, _)| *id == lp::grid_led(0, 0))
        .unwrap()
        .1;
    assert_eq!(led_sounding, [60, 0, 0]);
}

#[test]
fn flash_starts_white_and_decays() {
    let set = test_set();
    let mut loaded_samples = HashSet::new();
    loaded_samples.insert("samples/a.wav".to_string());
    let mut painter = LightPainter::new();
    let t0 = Instant::now();
    painter.on_pad_hit(0, 0, t0);
    let frame_hit = painter.paint(&set, 0, &loaded_samples, &|_b, _x, _y| true, t0);
    let led_hit = frame_hit
        .iter()
        .find(|(id, _)| *id == lp::grid_led(0, 0))
        .unwrap()
        .1;
    assert_eq!(led_hit, [63, 63, 63]);

    // After 200 ms (> 150 ms), flash has fully decayed
    let t_later = t0 + Duration::from_millis(200);
    let frame_later = painter.paint(&set, 0, &loaded_samples, &|_b, _x, _y| true, t_later);
    let led_later = frame_later
        .iter()
        .find(|(id, _)| *id == lp::grid_led(0, 0))
        .unwrap()
        .1;
    assert_eq!(led_later, [60, 0, 0]);
}

#[test]
fn missing_sample_blinks_red() {
    let set = test_set();
    let mut loaded_samples = HashSet::new();
    loaded_samples.insert("samples/a.wav".to_string());
    let mut painter = LightPainter::new();
    let t0 = Instant::now();
    // At t0: missing pad (1,0) is in ON phase -> red
    let frame_on = painter.paint(&set, 0, &loaded_samples, &|_b, _x, _y| false, t0);
    let led_missing_on = frame_on
        .iter()
        .find(|(id, _)| *id == lp::grid_led(1, 0))
        .unwrap()
        .1;
    assert_eq!(led_missing_on, [63, 0, 0]);

    // At t0 + 300 ms: in OFF phase of 500 ms cycle (250 ms on, 250 ms off) -> off
    let frame_off = painter.paint(
        &set,
        0,
        &loaded_samples,
        &|_b, _x, _y| false,
        t0 + Duration::from_millis(300),
    );
    let led_missing_off = frame_off
        .iter()
        .find(|(id, _)| *id == lp::grid_led(1, 0))
        .unwrap()
        .1;
    assert_eq!(led_missing_off, [0, 0, 0]);
}

#[test]
fn side_buttons_highlight_active_bank() {
    let set = test_set();
    let loaded_samples = HashSet::new();
    let mut painter = LightPainter::new();
    let now = Instant::now();
    let frame = painter.paint(&set, 2, &loaded_samples, &|_b, _x, _y| false, now);
    for i in 0..8u8 {
        let led = frame
            .iter()
            .find(|(id, _)| *id == lp::side_led(i))
            .unwrap()
            .1;
        if i == 2 {
            assert_eq!(led, [63, 63, 63]);
        } else {
            assert_eq!(led, [0, 0, 0]);
        }
    }
}
