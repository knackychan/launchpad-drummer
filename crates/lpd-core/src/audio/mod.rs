//! Real-time audio engine. Nothing in here touches a sound card.

mod biquad;
mod engine;
pub mod host;
mod sample;
mod voice;

pub use biquad::LowPass;
pub use engine::{
    Command, Engine, Event, GroupParams, AUDITION_PAD, GROUPS, MAX_BLOCK, MAX_VOICES,
};
pub use sample::Sample;
pub use voice::{PlayParams, Voice};

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Fader bottom: the UI shows −∞ here (`src/lib/mix.ts` DB_FLOOR).
pub const FADER_FLOOR_DB: f32 = -60.0;

/// dB to linear gain for a fader or volume control: at or below the floor is exact silence.
/// Use `db_to_gain` for ordinary maths such as humanize.
pub fn fader_gain(db: f32) -> f32 {
    if db <= FADER_FLOOR_DB {
        0.0
    } else {
        db_to_gain(db)
    }
}
