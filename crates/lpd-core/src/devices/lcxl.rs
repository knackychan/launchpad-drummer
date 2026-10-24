//! Novation Launch Control XL protocol (User template 0). Values are the engine spec's
//! "expected" ones (§2.3), unverified until `lp-probe` has run on the unit (plan decision D6).

pub const PORT_NAME: &str = "Launch Control XL";
/// The template the LEDs are addressed in and the controls are expected to send on.
pub const USER_TEMPLATE: u8 = 0;

use crate::synth::params::ParamId;
pub const SYNTH_KNOBS: [[ParamId; 8]; 3] = [
    [
        ParamId::OscAPos,
        ParamId::OscAWarpAmt,
        ParamId::OscADetune,
        ParamId::OscALevel,
        ParamId::OscBPos,
        ParamId::OscBWarpAmt,
        ParamId::OscBDetune,
        ParamId::OscBLevel,
    ],
    [
        ParamId::FilterCutoff,
        ParamId::FilterReso,
        ParamId::FilterDrive,
        ParamId::FilterEnvAmt,
        ParamId::Lfo1Rate,
        ParamId::Lfo2Rate,
        ParamId::FxDelayMix,
        ParamId::FxReverbMix,
    ],
    [
        ParamId::Env1Attack,
        ParamId::Env1Decay,
        ParamId::Env1Sustain,
        ParamId::Env1Release,
        ParamId::Env2Attack,
        ParamId::Env2Decay,
        ParamId::Env2Sustain,
        ParamId::Env2Release,
    ],
];

pub fn knob_led_index(row: u8, ch: u8) -> u8 {
    row * 8 + ch
}

/// True for the Launch Control XL's port, whatever Windows adds around the name.
pub fn is_lcxl_port(name: &str) -> bool {
    name.trim().contains(PORT_NAME)
}

/// `ch` and `row` are 0-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcxlEvent {
    Fader { ch: u8, value: u8 },
    Knob { row: u8, ch: u8, value: u8 },
    Button { row: u8, ch: u8, on: bool },
}

/// Any MIDI channel. CC 77–84 are the faders; CC 13–20, 29–36 and 49–56 the three knob rows.
/// Notes 41–44 and 57–60 are the first button row (channels 0–3 and 4–7), 73–76 and 89–92 the
/// second. Note On with velocity > 0 is "on"; Note On with velocity 0 and Note Off are "off".
pub fn decode(msg: &[u8]) -> Option<LcxlEvent> {
    let &[status, data, value] = msg else {
        return None;
    };
    match status & 0xF0 {
        0xB0 => match data {
            77..=84 => Some(LcxlEvent::Fader {
                ch: data - 77,
                value,
            }),
            13..=20 => Some(LcxlEvent::Knob {
                row: 0,
                ch: data - 13,
                value,
            }),
            29..=36 => Some(LcxlEvent::Knob {
                row: 1,
                ch: data - 29,
                value,
            }),
            49..=56 => Some(LcxlEvent::Knob {
                row: 2,
                ch: data - 49,
                value,
            }),
            _ => None,
        },
        kind @ (0x90 | 0x80) => {
            let (row, ch) = match data {
                41..=44 => (0, data - 41),
                57..=60 => (0, data - 57 + 4),
                73..=76 => (1, data - 73),
                89..=92 => (1, data - 89 + 4),
                _ => return None,
            };
            Some(LcxlEvent::Button {
                row,
                ch,
                on: kind == 0x90 && value > 0,
            })
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedColor {
    Off,
    Red,
    Green,
    Amber,
}

pub fn led_value(c: LedColor) -> u8 {
    match c {
        LedColor::Off => 0x0C,
        LedColor::Red => 0x0F,
        LedColor::Green => 0x3C,
        LedColor::Amber => 0x3F,
    }
}

/// LED index of a button: row 0 (track focus) 24 + ch, row 1 (track control) 32 + ch.
pub fn led_index(row: u8, ch: u8) -> u8 {
    if row == 0 {
        24 + ch
    } else {
        32 + ch
    }
}

pub fn led_message(template: u8, index: u8, value: u8) -> Vec<u8> {
    vec![
        0xF0, 0x00, 0x20, 0x29, 0x02, 0x11, 0x78, template, index, value, 0xF7,
    ]
}

/// Fader law: three quarters of the travel is 0 dB, the top is +6 dB, below that
/// `40·log10(p / 0.75)` down to the −60 dB floor.
pub fn fader_db(value: u8) -> f32 {
    let p = f32::from(value.min(127)) / 127.0;
    if p >= 0.75 {
        (p - 0.75) / 0.25 * 6.0
    } else if p > 0.0 {
        (40.0 * (p / 0.75).log10()).max(-60.0)
    } else {
        -60.0
    }
}

/// 0 → −1 (left), 64 → 0, 127 → +1.
pub fn knob_pan(value: u8) -> f32 {
    ((f32::from(value) - 64.0) / 63.0).clamp(-1.0, 1.0)
}

/// Whole semitones, ±12.
pub fn knob_pitch(value: u8) -> f32 {
    ((f32::from(value) - 64.0) / 63.0 * 12.0)
        .round()
        .clamp(-12.0, 12.0)
}

/// 200 Hz … 20 kHz, logarithmic.
pub fn knob_lowpass(value: u8) -> f32 {
    200.0 * 100.0_f32.powf(f32::from(value.min(127)) / 127.0)
}
