#![allow(dead_code)]

use launchpad_drummer_lib::state::AppState;
use lpd_core::audio::{Command, Event};
use lpd_core::kit::Set;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

pub fn write_wav(path: &Path, frames: usize) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        w.write_sample(8_000i16 + (i % 7) as i16).unwrap();
    }
    w.finalize().unwrap();
}

pub struct Rig {
    pub state: AppState,
    pub commands: rtrb::Consumer<Command>,
    pub events: rtrb::Producer<Event>,
}

/// An AppState with hand-driven queues (nothing consumes them unless the test does).
pub fn rig(queue: usize) -> Rig {
    let (tx, commands) = rtrb::RingBuffer::new(queue);
    let (events, rx) = rtrb::RingBuffer::new(256);
    Rig {
        state: AppState::new(Set::new(), tx, rx),
        commands,
        events,
    }
}

pub fn drain(commands: &mut rtrb::Consumer<Command>) -> Vec<Command> {
    let mut all = Vec::new();
    while let Ok(c) = commands.pop() {
        all.push(c);
    }
    all
}

/// Run `f` on its own thread and fail instead of hanging when it does not finish in `secs`.
/// A stuck worker is left behind; the test process still exits.
pub fn within<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .unwrap_or_else(|_| panic!("did not finish within {secs} s (deadlock or stall)"))
}
