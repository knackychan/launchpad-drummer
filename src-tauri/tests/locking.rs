//! Finding 1: pad input and LED painting must never wait on each other in opposite orders.

mod common;

use common::{rig, within};
use launchpad_drummer_lib::lights::PaintSnapshot;
use lpd_core::audio::{Event, Sample};
use lpd_core::devices::launchpad::LpEvent;
use lpd_core::kit::Pad;
use std::sync::Arc;
use std::time::Instant;

fn kit_rig() -> common::Rig {
    let r = rig(4096);
    r.state.edit(None, |set| {
        set.banks[0].set_pad(Pad::new(0, 0, "samples/a.wav", [60, 0, 0]));
        set.banks[0].set_pad(Pad::new(1, 0, "samples/missing.wav", [0, 60, 0]));
        set.banks[1].set_pad(Pad::new(0, 0, "samples/a.wav", [0, 0, 60]));
    });
    r.state.router.lock().unwrap().install_sample(
        "samples/a.wav".into(),
        Arc::new(Sample {
            rate: 48_000,
            data: vec![0.5; 480],
        }),
    );
    r
}

#[test]
fn router_stays_free_while_the_painter_is_busy() {
    let r = kit_rig();
    let state = r.state.clone();
    // Hold the painter: everything that needs it parks, but nothing may park holding the router.
    let painter = state.painter.clone();
    let guard = painter.lock().unwrap();

    let paint = {
        let s = state.clone();
        std::thread::spawn(move || s.paint_frame(Instant::now()))
    };
    let press = {
        let s = state.clone();
        std::thread::spawn(move || s.on_lp_event(LpEvent::PadDown { x: 0, y: 0 }))
    };
    let trigger = {
        let s = state.clone();
        std::thread::spawn(move || s.trigger_pad(0, 0))
    };
    std::thread::sleep(std::time::Duration::from_millis(200));

    let s = state.clone();
    let bank = within(5, move || {
        s.release_pad(0, 0);
        s.select_bank(2);
        s.stop_all();
        s.current_bank()
    });
    assert_eq!(bank, 2, "router operations ran while the painter was held");

    drop(guard);
    within(5, move || {
        paint.join().unwrap();
        press.join().unwrap();
        trigger.join().unwrap();
    });
}

#[test]
fn painting_consumes_its_snapshot_without_the_router() {
    let r = kit_rig();
    let state = r.state.clone();
    let snapshot = state.router.lock().unwrap().paint_snapshot().unwrap();
    // Keep the router locked for the whole paint.
    let router = state.router.clone();
    let _held = router.lock().unwrap();
    let painter = state.painter.clone();
    let frame = within(5, move || {
        painter
            .lock()
            .unwrap()
            .paint_snapshot(&snapshot, Instant::now())
    });
    assert_eq!(frame.len(), 72);
}

#[test]
fn snapshot_holds_only_the_active_bank_with_loaded_and_sounding_flags() {
    let mut r = kit_rig();
    r.state.select_bank(1);
    r.events.push(Event::VoiceStarted { pad: 64 }).unwrap();
    r.state.maintain();
    let snap: PaintSnapshot = r.state.router.lock().unwrap().paint_snapshot().unwrap();
    assert_eq!(snap.bank_index, 1);
    assert_eq!(snap.bank.pads.len(), 1);
    assert!(snap.loaded.contains("samples/a.wav"));
    assert!(!snap.loaded.contains("samples/missing.wav"));
    assert!(snap.sounding[0]);
    assert_eq!(snap.sounding.iter().filter(|s| **s).count(), 1);
}

#[test]
fn concurrent_input_and_painting_make_progress() {
    let r = kit_rig();
    let state = r.state.clone();
    within(30, move || {
        let mut workers = Vec::new();
        for t in 0..4 {
            let s = state.clone();
            workers.push(std::thread::spawn(move || {
                for i in 0..4_000u32 {
                    if t % 2 == 0 {
                        s.on_lp_event(LpEvent::PadDown { x: 0, y: 0 });
                        s.on_lp_event(LpEvent::PadUp { x: 0, y: 0 });
                        s.trigger_pad(0, 0);
                    } else {
                        let _ = s.paint_frame(Instant::now());
                        if i % 64 == 0 {
                            s.update_bank_humanize(0, (i % 6) as f32);
                        }
                    }
                    // keep the queue from filling up
                    if i % 256 == 0 {
                        s.maintain();
                    }
                }
            }));
        }
        for w in workers {
            w.join().unwrap();
        }
    });
}
