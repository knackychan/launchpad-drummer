//! MIDI learn: bindings from a note or CC of some input device to a pad or an app parameter
//! (plan 1C, Task 12). Pure data and mappings; the router applies them.

use super::{Mixer, Set};
use crate::devices::lcxl;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    /// The MIDI input port name the control was learned on.
    pub device: String,
    pub kind: LearnKind,
    pub number: u8,
    pub target: Target,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LearnKind {
    Note,
    Cc,
}

/// JSON: `{"pad":{"bank":0,"x":0,"y":0}}` or `{"param":"group.2.volume"}`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    Pad { bank: u8, x: u8, y: u8 },
    Param(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamValue {
    Continuous(f32),
    /// Fires when the CC crosses from below 64 to 64 or above (the router tracks the edge).
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupParam {
    Volume,
    Pan,
    Pitch,
    Lowpass,
    Mute,
    Solo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BankParam {
    Humanize,
    Flash,
    PlayState,
    Ripple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamId {
    Group(usize, GroupParam),
    Master,
    Bank(usize, BankParam),
    KeysVolume,
    KeysMod,
    SeqPlay,
    SeqBpm,
    SeqSwing,
    StopAll,
}

fn index(s: &str) -> Option<usize> {
    let mut chars = s.chars();
    let c = chars.next()?;
    (chars.next().is_none() && ('0'..='7').contains(&c)).then(|| c as usize - '0' as usize)
}

fn parse_id(id: &str) -> Option<ParamId> {
    let parts: Vec<&str> = id.split('.').collect();
    match parts.as_slice() {
        ["master"] => Some(ParamId::Master),
        ["stopAll"] => Some(ParamId::StopAll),
        ["keys", "volume"] => Some(ParamId::KeysVolume),
        ["keys", "mod"] => Some(ParamId::KeysMod),
        ["seq", "play"] => Some(ParamId::SeqPlay),
        ["seq", "bpm"] => Some(ParamId::SeqBpm),
        ["seq", "swing"] => Some(ParamId::SeqSwing),
        ["group", n, what] => {
            let what = match *what {
                "volume" => GroupParam::Volume,
                "pan" => GroupParam::Pan,
                "pitch" => GroupParam::Pitch,
                "lowpass" => GroupParam::Lowpass,
                "mute" => GroupParam::Mute,
                "solo" => GroupParam::Solo,
                _ => return None,
            };
            Some(ParamId::Group(index(n)?, what))
        }
        ["bank", n, what] => {
            let what = match *what {
                "humanize" => BankParam::Humanize,
                "flash" => BankParam::Flash,
                "playState" => BankParam::PlayState,
                "ripple" => BankParam::Ripple,
                _ => return None,
            };
            Some(ParamId::Bank(index(n)?, what))
        }
        _ => None,
    }
}

pub fn is_param(id: &str) -> bool {
    parse_id(id).is_some()
}

/// The mixer channel of a `group.{n}.*` id.
pub fn group_index(id: &str) -> Option<usize> {
    match parse_id(id)? {
        ParamId::Group(i, _) => Some(i),
        _ => None,
    }
}

/// CC value → parameter value (plan 1C Task 12 table).
pub fn param_value(id: &str, cc: u8) -> Option<ParamValue> {
    let cc = cc.min(127);
    let unit = f32::from(cc) / 127.0;
    let continuous = ParamValue::Continuous;
    Some(match parse_id(id)? {
        ParamId::Group(_, GroupParam::Volume) | ParamId::Master => continuous(lcxl::fader_db(cc)),
        ParamId::Group(_, GroupParam::Pan) => continuous(lcxl::knob_pan(cc)),
        ParamId::Group(_, GroupParam::Pitch) => continuous(lcxl::knob_pitch(cc)),
        ParamId::Group(_, GroupParam::Lowpass) => continuous(lcxl::knob_lowpass(cc)),
        ParamId::Group(_, GroupParam::Mute | GroupParam::Solo)
        | ParamId::Bank(_, BankParam::Flash | BankParam::PlayState | BankParam::Ripple)
        | ParamId::SeqPlay
        | ParamId::StopAll => ParamValue::Toggle,
        ParamId::Bank(_, BankParam::Humanize) => continuous((unit * 6.0 / 0.5).round() * 0.5),
        ParamId::KeysVolume => continuous(-40.0 + unit * 46.0),
        ParamId::KeysMod => continuous(unit),
        ParamId::SeqBpm => continuous((60.0 + unit * 140.0).round()),
        ParamId::SeqSwing => continuous((50.0 + unit * 25.0).round()),
    })
}

/// Removes any binding with the same source `(device, kind, number)` and any binding with the
/// same target, then adds `b`.
pub fn bind(learn: &mut Vec<Binding>, b: Binding) {
    learn.retain(|old| {
        !(old.device == b.device && old.kind == b.kind && old.number == b.number)
            && old.target != b.target
    });
    learn.push(b);
}

pub fn unbind(learn: &mut Vec<Binding>, index: usize) -> bool {
    if index < learn.len() {
        learn.remove(index);
        true
    } else {
        false
    }
}

/// `from`/`to` are `(bank, x, y)`; `None` removes the pad's bindings.
pub fn retarget_pad(learn: &mut Vec<Binding>, from: (u8, u8, u8), to: Option<(u8, u8, u8)>) {
    let is_from = |t: &Target| {
        *t == Target::Pad {
            bank: from.0,
            x: from.1,
            y: from.2,
        }
    };
    match to {
        Some((bank, x, y)) => {
            for b in learn.iter_mut().filter(|b| is_from(&b.target)) {
                b.target = Target::Pad { bank, x, y };
            }
        }
        None => learn.retain(|b| !is_from(&b.target)),
    }
}

/// Exchange the bindings of two pads.
pub fn swap_pad_bindings(learn: &mut [Binding], a: (u8, u8, u8), b: (u8, u8, u8)) {
    let pad = |p: (u8, u8, u8)| Target::Pad {
        bank: p.0,
        x: p.1,
        y: p.2,
    };
    let (ta, tb) = (pad(a), pad(b));
    for binding in learn.iter_mut() {
        if binding.target == ta {
            binding.target = tb.clone();
        } else if binding.target == tb {
            binding.target = ta.clone();
        }
    }
}

/// Apply a mixer parameter (`group.*`, `master`). `Toggle` flips mute/solo. True when the
/// value changed.
pub fn apply_mixer_param(mixer: &mut Mixer, id: &str, value: ParamValue) -> bool {
    match (parse_id(id), value) {
        (Some(ParamId::Master), ParamValue::Continuous(v)) => replace(&mut mixer.master_db, v),
        (Some(ParamId::Group(i, what)), value) => {
            let Some(g) = mixer.groups.get_mut(i) else {
                return false;
            };
            match (what, value) {
                (GroupParam::Volume, ParamValue::Continuous(v)) => replace(&mut g.volume_db, v),
                (GroupParam::Pan, ParamValue::Continuous(v)) => replace(&mut g.pan, v),
                (GroupParam::Pitch, ParamValue::Continuous(v)) => {
                    replace(&mut g.pitch_semitones, v)
                }
                (GroupParam::Lowpass, ParamValue::Continuous(v)) => replace(&mut g.lowpass_hz, v),
                (GroupParam::Mute, ParamValue::Toggle) => {
                    g.mute = !g.mute;
                    true
                }
                (GroupParam::Solo, ParamValue::Toggle) => {
                    g.solo = !g.solo;
                    true
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// Apply a set-level parameter (`bank.*`, `seq.bpm`, `seq.swing`). The keys parameters are
/// applied by their own task. True when the value changed.
pub fn apply_set_param(set: &mut Set, id: &str, value: ParamValue) -> bool {
    match (parse_id(id), value) {
        (Some(ParamId::SeqBpm), ParamValue::Continuous(v)) => {
            replace(&mut set.pattern.bpm, v.clamp(60.0, 200.0))
        }
        (Some(ParamId::SeqSwing), ParamValue::Continuous(v)) => {
            replace(&mut set.pattern.swing, v.clamp(50.0, 75.0))
        }
        (Some(ParamId::Bank(i, what)), value) => {
            let Some(bank) = set.banks.get_mut(i) else {
                return false;
            };
            match (what, value) {
                (BankParam::Humanize, ParamValue::Continuous(v)) => {
                    replace(&mut bank.humanize_db, v.clamp(0.0, 6.0))
                }
                (BankParam::Flash, ParamValue::Toggle) => flip(&mut bank.lights.flash),
                (BankParam::PlayState, ParamValue::Toggle) => flip(&mut bank.lights.play_state),
                (BankParam::Ripple, ParamValue::Toggle) => flip(&mut bank.lights.ripple),
                _ => false,
            }
        }
        _ => false,
    }
}

fn replace(slot: &mut f32, v: f32) -> bool {
    let changed = *slot != v;
    *slot = v;
    changed
}

fn flip(slot: &mut bool) -> bool {
    *slot = !*slot;
    true
}
