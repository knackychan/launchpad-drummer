//! Real-time audio engine. Nothing in here touches a sound card.

mod sample;

pub use sample::Sample;

pub const GROUPS: usize = 8;

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayParams {
    /// Linear gain.
    pub gain: f32,
    pub pitch_semitones: f32,
    /// Playback range in source frames: start_frame <= end_frame <= sample.frames().
    pub start_frame: usize,
    pub end_frame: usize,
    pub choke_group: Option<u8>,
    /// 0..8
    pub mix_group: u8,
    /// Hold mode: releasing the pad fades the voice out.
    pub hold: bool,
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
