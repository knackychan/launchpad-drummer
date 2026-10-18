//! Per-machine app settings: `<app_config_dir>/config.json`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub last_set: Option<String>,
    pub library_dir: Option<String>,
    pub soundfont: Option<String>,
    pub audio: AudioPrefs,
    /// Port name → "keys" | "other".
    pub midi_roles: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AudioPrefs {
    /// "wasapi" | "asio".
    pub backend: String,
    pub device: Option<String>,
    pub buffer_frames: Option<u32>,
}

impl Default for AudioPrefs {
    fn default() -> Self {
        Self {
            backend: "wasapi".into(),
            device: None,
            buffer_frames: None,
        }
    }
}

/// A missing or broken file gives the defaults.
pub fn load(path: &Path) -> AppConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Atomic write: `config.json.tmp`, then rename over `config.json`.
pub fn save(path: &Path, c: &AppConfig) -> anyhow::Result<()> {
    use anyhow::Context;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_file_name("config.json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(c)?)
        .with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let c = load(&tmp.path().join("config.json"));
        assert_eq!(c, AppConfig::default());
        assert_eq!(c.audio.backend, "wasapi");
    }

    #[test]
    fn save_then_load_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub").join("config.json");
        let mut c = AppConfig {
            last_set: Some(r"D:\Sets\Kit".into()),
            library_dir: Some(r"D:\Samples".into()),
            ..Default::default()
        };
        c.audio.buffer_frames = Some(128);
        c.midi_roles
            .insert("Keystation 49 MK3".into(), "keys".into());
        save(&path, &c).unwrap();
        assert_eq!(load(&path), c);
        assert!(!path.with_file_name("config.json.tmp").exists());
    }

    #[test]
    fn broken_json_gives_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), AppConfig::default());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"lastSet":"D:\\Kit","crtLevel":3,"audio":{"backend":"asio","extra":1}}"#,
        )
        .unwrap();
        let c = load(&path);
        assert_eq!(c.last_set.as_deref(), Some(r"D:\Kit"));
        assert_eq!(c.audio.backend, "asio");
    }
}
