use serde::{Deserialize, Serialize};

pub const BANKS: usize = 8;
pub const GROUPS: usize = 8;
pub const SET_VERSION: u32 = 2;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Set {
    pub version: u32,
    pub banks: Vec<Bank>,
    pub mixer: Mixer,
    /// MIDI learn bindings (Task 12); old files have none.
    #[serde(default)]
    pub learn: Vec<super::learn::Binding>,
    /// The step sequencer's pattern (Task 13); old files get the default.
    #[serde(default)]
    pub pattern: crate::seq::Pattern,
    /// The keys instrument's sound, level and output (Task 15).
    #[serde(default)]
    pub keys: crate::instrument::KeysSettings,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bank {
    pub name: String,
    pub lights: LightModes,
    pub humanize_db: f32,
    /// Only pads with a sample; empty pads are omitted.
    pub pads: Vec<Pad>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LightModes {
    pub flash: bool,
    pub play_state: bool,
    pub ripple: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pad {
    /// 0..8, (0, 0) = bottom-left.
    pub x: u8,
    pub y: u8,
    /// Path relative to the set folder, forward slashes.
    pub sample: String,
    /// RGB, each 0..=63 (Launchpad LED range).
    pub color: [u8; 3],
    pub volume_db: f32,
    pub pitch_semitones: f32,
    pub trim_start_ms: f32,
    pub trim_end_ms: Option<f32>,
    pub mode: PadMode,
    /// None or 1..=8.
    pub choke_group: Option<u8>,
    /// 0..8.
    pub mix_group: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PadMode {
    OneShot,
    Hold,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Mixer {
    pub groups: Vec<Group>,
    pub master_db: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    pub volume_db: f32,
    /// -1 = left, 0 = centre, 1 = right.
    pub pan: f32,
    pub pitch_semitones: f32,
    /// 20 000 = filter bypassed.
    pub lowpass_hz: f32,
    pub mute: bool,
    pub solo: bool,
}

impl Pad {
    /// A pad with default settings; mix group = row (spec §4.2).
    pub fn new(x: u8, y: u8, sample: impl Into<String>, color: [u8; 3]) -> Pad {
        Pad {
            x,
            y,
            sample: sample.into(),
            color,
            volume_db: 0.0,
            pitch_semitones: 0.0,
            trim_start_ms: 0.0,
            trim_end_ms: None,
            mode: PadMode::OneShot,
            choke_group: None,
            mix_group: y,
        }
    }
}

impl Bank {
    pub fn pad(&self, x: u8, y: u8) -> Option<&Pad> {
        self.pads.iter().find(|p| p.x == x && p.y == y)
    }

    /// Insert, or replace the pad at the same position.
    pub fn set_pad(&mut self, pad: Pad) {
        self.pads.retain(|p| !(p.x == pad.x && p.y == pad.y));
        self.pads.push(pad);
    }
}

impl Set {
    pub fn new() -> Set {
        let banks = (1..=BANKS)
            .map(|i| Bank {
                name: format!("Bank {i}"),
                lights: LightModes {
                    flash: true,
                    play_state: true,
                    ripple: false,
                },
                humanize_db: 0.0,
                pads: Vec::new(),
            })
            .collect();
        let groups = (1..=GROUPS)
            .map(|i| Group {
                name: format!("Group {i}"),
                volume_db: 0.0,
                pan: 0.0,
                pitch_semitones: 0.0,
                lowpass_hz: 20_000.0,
                mute: false,
                solo: false,
            })
            .collect();
        Set {
            version: SET_VERSION,
            banks,
            mixer: Mixer {
                groups,
                master_db: 0.0,
            },
            learn: Vec::new(),
            pattern: crate::seq::Pattern::default(),
            keys: crate::instrument::KeysSettings::default(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("Set always serializes")
    }

    pub fn from_json(s: &str) -> anyhow::Result<Set> {
        let mut value: serde_json::Value = serde_json::from_str(s)?;
        if value["version"] == 1 {
            use crate::instrument::{factory_id, KeysSound};
            use crate::synth::{factory::factory, patch::PatchSource};
            let keys = &value["keys"];
            let sound: KeysSound = serde_json::from_value(
                keys.get("sound")
                    .cloned()
                    .unwrap_or(serde_json::json!("ePiano")),
            )?;
            let id = factory_id(sound);
            value["keys"] = serde_json::json!({
                "patch": factory(id).expect("known GM factory id"),
                "patchSource": PatchSource::Factory(id.into()), "modified": false, "gmSound": sound,
                "volumeDb": keys.get("volumeDb").cloned().unwrap_or(serde_json::json!(-6.0)),
                "output": keys.get("output").cloned().unwrap_or(serde_json::Value::Null)
            });
            value["version"] = serde_json::json!(SET_VERSION);
        }
        let set: Set = serde_json::from_value(value)?;
        anyhow::ensure!(
            set.version == SET_VERSION,
            "unsupported set version {}",
            set.version
        );
        anyhow::ensure!(
            set.banks.len() == BANKS && set.mixer.groups.len() == GROUPS,
            "a set needs {BANKS} banks and {GROUPS} mix groups"
        );
        Ok(set)
    }
}

impl Default for Set {
    fn default() -> Self {
        Set::new()
    }
}
