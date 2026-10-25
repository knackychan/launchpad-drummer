//! What the UI shows live (spec 2.4): the newest voice's sources, destinations, envelopes and LFOs.

use super::params::{FilterType, DESTS, ENVS, LFOS, SOURCES};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SynthMonitor {
    /// Value of every source, `Source` order.
    pub sources: [f32; SOURCES],
    /// Live value of every destination, 0–1 (pitch destinations: 0.5 + modulation / 2).
    pub live: [f32; DESTS],
    /// Per envelope: level 0–1, stage 0 idle … 4 release.
    pub env: [[f32; 2]; ENVS],
    /// Per LFO: output, phase 0–1.
    pub lfo: [[f32; 2]; LFOS],
    pub cutoff_hz: f32,
    pub reso: f32,
    pub drive: f32,
    pub filter: FilterType,
    pub voices: u8,
    /// Synth render time / block time; the engine fills it in.
    pub cpu: f32,
    pub resets: u32,
}

impl Default for SynthMonitor {
    fn default() -> Self {
        SynthMonitor {
            sources: [0.0; SOURCES],
            live: [0.0; DESTS],
            env: [[0.0; 2]; ENVS],
            lfo: [[0.0; 2]; LFOS],
            cutoff_hz: 12_000.0,
            reso: 0.0,
            drive: 0.0,
            filter: FilterType::Lp24,
            voices: 0,
            cpu: 0.0,
            resets: 0,
        }
    }
}
