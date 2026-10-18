//! Real-time audio engine. Nothing in here touches a sound card.

mod sample;
mod voice;

pub use sample::Sample;
pub use voice::{PlayParams, Voice};

pub const GROUPS: usize = 8;

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupParams {
    /// Linear gain.
    pub gain: f32,
    /// -1 = left, 0 = centre, 1 = right.
    pub pan: f32,
    pub pitch_semitones: f32,
    pub lowpass_hz: f32,
    pub mute: bool,
    pub solo: bool,
}

impl Default for GroupParams {
    fn default() -> Self {
        GroupParams {
            gain: 1.0,
            pan: 0.0,
            pitch_semitones: 0.0,
            lowpass_hz: 20_000.0,
            mute: false,
            solo: false,
        }
    }
}
