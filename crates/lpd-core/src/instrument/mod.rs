//! The keys instrument (plan 1C, Task 15; decisions D1 and D5): a built-in synth that sounds out of
//! the box, and an optional General MIDI SoundFont. Both implement `Instrument`; the audio engine
//! owns one and renders it into the mix.

mod basic;
mod soundfont;

pub use basic::BasicKeys;
pub use soundfont::SoundFontKeys;

use serde::{Deserialize, Serialize};

pub const KEYS_VOICES: usize = 16;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum KeysSound {
    Piano,
    #[default]
    EPiano,
    Organ,
    SynthPad,
    SynthLead,
    SynthBass,
}

/// The keys settings stored in the set file. `output` is a mixer channel (0..8) or `None` for the
/// master bus.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct KeysSettings {
    pub sound: KeysSound,
    pub volume_db: f32,
    pub output: Option<u8>,
}

impl Default for KeysSettings {
    fn default() -> Self {
        KeysSettings {
            sound: KeysSound::EPiano,
            volume_db: -6.0,
            output: None,
        }
    }
}

/// General MIDI program of a sound (SoundFont engine).
pub fn gm_program(sound: KeysSound) -> u8 {
    match sound {
        KeysSound::Piano => 0,
        KeysSound::EPiano => 4,
        KeysSound::Organ => 16,
        KeysSound::SynthPad => 89,
        KeysSound::SynthLead => 81,
        KeysSound::SynthBass => 38,
    }
}

/// Everything the engine asks of a keys sound. Called on the audio thread: no allocation, no locks.
pub trait Instrument: Send {
    fn note_on(&mut self, note: u8, velocity: u8);
    fn note_off(&mut self, note: u8);
    /// −2..2 semitones; applies to held notes live.
    fn pitch_bend(&mut self, semitones: f32);
    /// 0..1: vibrato depth up to ±0.5 semitone at 5.5 Hz.
    fn modulation(&mut self, depth: f32);
    fn set_sound(&mut self, sound: KeysSound);
    /// Every note ends with a fast (about 5 ms) release.
    fn all_notes_off(&mut self);
    /// Adds into `l` and `r` (same length, at most `MAX_BLOCK`). Never allocates.
    fn render_add(&mut self, l: &mut [f32], r: &mut [f32]);
}

impl std::fmt::Debug for dyn Instrument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Instrument")
    }
}
