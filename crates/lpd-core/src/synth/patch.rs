//! Patch files and their validation (keys synth spec §8).
use super::params::{ParamId, SynthParams};
use serde::{Deserialize, Serialize};

pub const PATCH_VERSION: u32 = 1;
pub const NAME_MAX: usize = 24;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Piano,
    #[default]
    Keys,
    Organ,
    Pad,
    Lead,
    Bass,
    Pluck,
    Fx,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Self::Piano,
        Self::Keys,
        Self::Organ,
        Self::Pad,
        Self::Lead,
        Self::Bass,
        Self::Pluck,
        Self::Fx,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Piano => "piano",
            Self::Keys => "keys",
            Self::Organ => "organ",
            Self::Pad => "pad",
            Self::Lead => "lead",
            Self::Bass => "bass",
            Self::Pluck => "pluck",
            Self::Fx => "fx",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Piano => "PIANO",
            Self::Keys => "KEYS",
            Self::Organ => "ORGAN",
            Self::Pad => "PAD",
            Self::Lead => "LEAD",
            Self::Bass => "BASS",
            Self::Pluck => "PLUCK",
            Self::Fx => "FX",
        }
    }
    pub fn color(self) -> [u8; 3] {
        match self {
            Self::Piano => [63, 63, 63],
            Self::Keys => [63, 40, 0],
            Self::Organ => [63, 20, 0],
            Self::Pad => [25, 0, 63],
            Self::Lead => [0, 63, 25],
            Self::Bass => [0, 30, 63],
            Self::Pluck => [63, 63, 0],
            Self::Fx => [63, 0, 40],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PatchSource {
    Factory(String),
    User(String),
    Init,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Patch {
    pub patch_version: u32,
    pub name: String,
    pub category: Category,
    pub params: SynthParams,
}

impl Patch {
    pub fn new(name: &str, category: Category, params: SynthParams) -> Patch {
        Patch {
            patch_version: PATCH_VERSION,
            name: clean_name(name),
            category,
            params: clamped(params),
        }
    }
    pub fn init() -> Patch {
        Patch::new("INIT", Category::Keys, SynthParams::default())
    }
    pub fn from_json(s: &str) -> anyhow::Result<Patch> {
        let mut p: Patch = serde_json::from_str(s)?;
        anyhow::ensure!(
            p.patch_version <= PATCH_VERSION,
            "Patch made by a newer version"
        );
        p.name = clean_name(&p.name);
        p.params = clamped(p.params);
        Ok(p)
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("Patch always serializes")
    }
}
impl Default for Patch {
    fn default() -> Patch {
        Patch::init()
    }
}

pub fn clamped(mut p: SynthParams) -> SynthParams {
    for &id in ParamId::ALL {
        let v = id.get(&p);
        id.set(&mut p, v);
    }
    p
}

pub fn clean_name(name: &str) -> String {
    let name: String = name.trim().to_uppercase().chars().take(NAME_MAX).collect();
    if name.is_empty() {
        "UNTITLED".into()
    } else {
        name
    }
}

pub fn file_name(name: &str) -> String {
    let name: String = clean_name(name)
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut name = name.trim_end_matches(['.', ' ']).replace("..", "__");
    if name.is_empty() {
        name = "UNTITLED".into();
    }
    let stem = name.split('.').next().unwrap();
    let reserved = matches!(stem, "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if reserved {
        name.insert(stem.len(), '_');
    }
    format!("{name}.lbpatch")
}
