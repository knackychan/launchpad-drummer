//! A General MIDI SoundFont as the keys sound (rustysynth). Loaded off the audio thread, then
//! handed to the engine as a boxed `Instrument`.

use super::{gm_program, Instrument, KeysSound};
use crate::audio::MAX_BLOCK;
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

const CHANNEL: i32 = 0;
const MAX_POLYPHONY: usize = 32;

pub struct SoundFontKeys {
    synth: Synthesizer,
    left: Vec<f32>,
    right: Vec<f32>,
}

impl SoundFontKeys {
    pub fn load(path: &Path, sample_rate: u32) -> anyhow::Result<Self> {
        let mut file =
            File::open(path).map_err(|e| anyhow::anyhow!("can't open {}: {e}", path.display()))?;
        let font = SoundFont::new(&mut file)
            .map_err(|e| anyhow::anyhow!("not a usable SoundFont: {e}"))?;
        let mut settings = SynthesizerSettings::new(sample_rate as i32);
        settings.maximum_polyphony = MAX_POLYPHONY;
        let synth = Synthesizer::new(&Arc::new(font), &settings)
            .map_err(|e| anyhow::anyhow!("SoundFont rejected: {e}"))?;
        let mut keys = SoundFontKeys {
            synth,
            left: vec![0.0; MAX_BLOCK],
            right: vec![0.0; MAX_BLOCK],
        };
        keys.set_pitch_bend_range();
        keys.set_sound(KeysSound::default());
        Ok(keys)
    }

    /// RPN 0: pitch bend range ±2 semitones.
    fn set_pitch_bend_range(&mut self) {
        for (cc, value) in [(101, 0), (100, 0), (6, 2), (38, 0)] {
            self.synth.process_midi_message(CHANNEL, 0xB0, cc, value);
        }
    }
}

impl Instrument for SoundFontKeys {
    fn note_on(&mut self, note: u8, velocity: u8) {
        self.synth
            .note_on(CHANNEL, i32::from(note), i32::from(velocity));
    }

    fn note_off(&mut self, note: u8) {
        self.synth.note_off(CHANNEL, i32::from(note));
    }

    fn pitch_bend(&mut self, wheel: f32) {
        let value = (8192.0 + wheel.clamp(-1.0, 1.0) * 8191.0).round() as i32;
        self.synth
            .process_midi_message(CHANNEL, 0xE0, value & 0x7F, value >> 7);
    }

    fn modulation(&mut self, depth: f32) {
        let value = (depth.clamp(0.0, 1.0) * 127.0).round() as i32;
        self.synth.process_midi_message(CHANNEL, 0xB0, 1, value);
    }

    fn sustain(&mut self, down: bool) {
        self.synth
            .process_midi_message(CHANNEL, 0xB0, 64, if down { 127 } else { 0 });
    }

    fn set_sound(&mut self, sound: KeysSound) {
        self.synth
            .process_midi_message(CHANNEL, 0xC0, i32::from(gm_program(sound)), 0);
    }

    fn all_notes_off(&mut self) {
        self.synth.note_off_all(false);
    }

    fn render_add(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let mut at = 0;
        while at < n {
            let len = (n - at).min(MAX_BLOCK);
            self.synth
                .render(&mut self.left[..len], &mut self.right[..len]);
            for i in 0..len {
                l[at + i] += self.left[i];
                r[at + i] += self.right[i];
            }
            at += len;
        }
    }
}
