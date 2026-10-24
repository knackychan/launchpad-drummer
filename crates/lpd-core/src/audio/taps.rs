//! Lock-free taps on the engine output for the telemetry display. The audio thread only pushes
//! (a full queue drops the value); another thread drains.

use super::GROUPS;
use crate::synth::monitor::SynthMonitor;
use rtrb::{Consumer, Producer, RingBuffer};

pub const SCOPE_CAPACITY: usize = 8192;
pub const LEVELS_CAPACITY: usize = 512;
pub const KEYS_SCOPE_CAPACITY: usize = 8192;
pub const SYNTH_MONITOR_CAPACITY: usize = 64;

/// Peak |x| over one rendered block.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Levels {
    pub master: [f32; 2],
    pub groups: [f32; GROUPS],
}

/// The reading side, owned by the telemetry thread.
pub struct Taps {
    /// Mono sum of every output frame, in order.
    pub scope: Consumer<f32>,
    pub levels: Consumer<Levels>,
    pub keys: Consumer<f32>,
    pub synth: Consumer<SynthMonitor>,
}

/// The writing side, owned by the engine.
pub(crate) struct TapSender {
    pub scope: Producer<f32>,
    pub levels: Producer<Levels>,
    pub keys: Producer<f32>,
    pub synth: Producer<SynthMonitor>,
}

pub(crate) fn channel() -> (TapSender, Taps) {
    let (scope_tx, scope) = RingBuffer::new(SCOPE_CAPACITY);
    let (levels_tx, levels) = RingBuffer::new(LEVELS_CAPACITY);
    let (keys_tx, keys) = RingBuffer::new(KEYS_SCOPE_CAPACITY);
    let (synth_tx, synth) = RingBuffer::new(SYNTH_MONITOR_CAPACITY);
    (
        TapSender {
            scope: scope_tx,
            levels: levels_tx,
            keys: keys_tx,
            synth: synth_tx,
        },
        Taps {
            scope,
            levels,
            keys,
            synth,
        },
    )
}
