//! Findings 6 and 7 (LED side): the painter's cache must describe what the device really shows.

mod common;

use common::rig;
use launchpad_drummer_lib::lights::{LedOutput, LedSink, LightPainter};
use launchpad_drummer_lib::state::InputMode;
use lpd_core::devices::launchpad as lp;
use lpd_core::kit::Pad;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Default)]
struct Wire {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    broken: Arc<Mutex<bool>>,
}

impl LedSink for Wire {
    fn send(&mut self, message: &[u8]) -> Result<(), String> {
        if *self.broken.lock().unwrap() {
            return Err("port gone".into());
        }
        self.sent.lock().unwrap().push(message.to_vec());
        Ok(())
    }
}

impl Wire {
    fn take(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut *self.sent.lock().unwrap())
    }
    /// SysEx RGB messages only (not the layout/clear initialisation).
    fn led_count(&self) -> usize {
        self.take()
            .iter()
            .filter(|m| m.get(6) == Some(&0x0B))
            .map(|m| (m.len() - 8) / 4)
            .sum()
    }
}

fn frame(shade: u8) -> Vec<(u8, [u8; 3])> {
    let mut f = Vec::new();
    for y in 0..8 {
        for x in 0..8 {
            f.push((lp::grid_led(x, y), [shade, 0, 0]));
        }
    }
    for i in 0..8 {
        f.push((lp::side_led(i), [0, 0, 0]));
    }
    f
}

fn connected() -> (LedOutput<Wire>, Mutex<LightPainter>, Wire) {
    let wire = Wire::default();
    let painter = Mutex::new(LightPainter::new());
    let mut out = LedOutput::default();
    out.connect(wire.clone(), &painter).unwrap();
    wire.take();
    (out, painter, wire)
}

#[test]
fn an_unchanged_frame_sends_nothing() {
    let (mut out, painter, wire) = connected();
    out.send_sampler(&painter, &frame(40)).unwrap();
    assert_eq!(wire.led_count(), 72);
    out.send_sampler(&painter, &frame(40)).unwrap();
    assert_eq!(wire.led_count(), 0);
    out.send_sampler(&painter, &frame(41)).unwrap();
    assert_eq!(wire.led_count(), 64, "only the changed pads");
}

#[test]
fn reconnecting_repaints_everything_without_any_pad_hit() {
    let (mut out, painter, wire) = connected();
    out.send_sampler(&painter, &frame(40)).unwrap();
    wire.take();

    out.disconnect(&painter);
    assert!(!out.is_connected());
    // While unplugged nothing is sent or remembered.
    out.send_sampler(&painter, &frame(40)).unwrap();

    let fresh = Wire::default();
    out.connect(fresh.clone(), &painter).unwrap();
    assert!(fresh.take().len() >= 2, "layout + clear were sent");
    out.send_sampler(&painter, &frame(40)).unwrap();
    assert_eq!(
        fresh.led_count(),
        72,
        "the full kit frame, unchanged since before"
    );
}

#[test]
fn a_failed_send_forgets_the_cache_and_the_retry_is_complete() {
    let (mut out, painter, wire) = connected();
    out.send_sampler(&painter, &frame(40)).unwrap();
    wire.take();

    *wire.broken.lock().unwrap() = true;
    assert!(out.send_sampler(&painter, &frame(41)).is_err());
    assert!(!out.is_connected(), "a lost port drops the connection");

    let fresh = Wire::default();
    out.connect(fresh.clone(), &painter).unwrap();
    out.send_sampler(&painter, &frame(41)).unwrap();
    assert_eq!(
        fresh.led_count(),
        72,
        "nothing from the failed send was assumed delivered"
    );
}

#[test]
fn a_game_frame_forces_a_complete_sampler_repaint_afterwards() {
    let (mut out, painter, wire) = connected();
    out.send_sampler(&painter, &frame(40)).unwrap();
    wire.take();

    out.send_game(&painter, vec![0xF0, 0, 0x20, 0x29, 2, 0x18, 0x0B, 0xF7])
        .unwrap();
    assert_eq!(wire.take().len(), 1);
    out.send_sampler(&painter, &frame(40)).unwrap();
    assert_eq!(
        wire.led_count(),
        72,
        "the unchanged sampler frame is drawn in full"
    );
}

#[test]
fn a_stale_diff_cannot_mark_the_cache_after_an_invalidation() {
    let mut painter = LightPainter::new();
    let diff = painter.diff(&frame(40));
    painter.invalidate(); // another owner drew on the device meanwhile
    painter.mark_sent(&diff);
    assert_eq!(painter.diff(&frame(40)).leds.len(), 72);
}

#[test]
fn a_game_resumes_with_its_last_frame_after_a_reconnect() {
    let (mut out, painter, wire) = connected();
    let message = vec![0xF0, 0, 0x20, 0x29, 2, 0x18, 0x0B, 1, 2, 3, 4, 0xF7];
    out.send_game(&painter, message.clone()).unwrap();
    wire.take();
    out.disconnect(&painter);

    let fresh = Wire::default();
    out.connect(fresh.clone(), &painter).unwrap();
    out.resend_game(&painter).unwrap();
    assert!(fresh.take().contains(&message));
    out.clear_game();
    out.resend_game(&painter).unwrap();
    assert!(fresh.take().is_empty());
}

#[test]
fn switching_input_owner_invalidates_the_painter() {
    let r = rig(256);
    r.state.edit(None, |set| {
        set.banks[0].set_pad(Pad::new(0, 0, "samples/a.wav", [60, 0, 0]));
    });
    let full = r.state.paint_frame(Instant::now());
    let first = r.state.painter.lock().unwrap().diff(&full);
    r.state.painter.lock().unwrap().mark_sent(&first);
    assert!(r.state.painter.lock().unwrap().diff(&full).leds.is_empty());

    r.state.set_input_mode(InputMode::Game);
    assert_eq!(
        r.state.painter.lock().unwrap().diff(&full).leds.len(),
        full.len()
    );
    let again = r.state.painter.lock().unwrap().diff(&full);
    r.state.painter.lock().unwrap().mark_sent(&again);

    r.state.set_input_mode(InputMode::Sampler);
    assert_eq!(
        r.state.painter.lock().unwrap().diff(&full).leds.len(),
        full.len()
    );
}
