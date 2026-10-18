//! Real-time audio engine. Nothing in here touches a sound card.

mod biquad;
mod engine;
mod sample;
mod voice;

pub use biquad::LowPass;
pub use engine::{Command, Engine, Event, GroupParams, GROUPS, MAX_BLOCK, MAX_VOICES};
pub use sample::Sample;
pub use voice::{PlayParams, Voice};

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}
