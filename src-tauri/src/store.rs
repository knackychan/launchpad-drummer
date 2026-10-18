//! Kit store and sample loading management.

use anyhow::{bail, Result};
use lpd_core::audio::Sample;
use lpd_core::kit::{self, Loaded, Set};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

pub fn list_dir(path: &Path) -> Result<Vec<DirEntry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let metadata = entry.metadata()?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & (0x2 | 0x4) != 0 {
                continue;
            }
        }
        let is_dir = metadata.is_dir();
        if is_dir || metadata.is_file() && kit::ops::is_audio_path(&entry.path()) {
            entries.push(DirEntry {
                name,
                path: entry.path().to_string_lossy().into_owned(),
                is_dir,
            });
        }
    }
    entries.sort_by_key(|e| (!e.is_dir, e.name.to_lowercase()));
    Ok(entries)
}

pub struct LoadedKit {
    pub set: Set,
    pub dir: PathBuf,
    pub samples: HashMap<String, Arc<Sample>>,
    pub failed_samples: Vec<(String, String)>,
    pub from_backup: bool,
}

pub struct KitManager {
    pub dir: Option<PathBuf>,
    pub samples: HashMap<String, Arc<Sample>>,
    pub failed_samples: Vec<(String, String)>,
}

impl Default for KitManager {
    fn default() -> Self {
        Self::new_default()
    }
}

impl KitManager {
    pub fn new_default() -> Self {
        Self {
            dir: None,
            samples: HashMap::new(),
            failed_samples: Vec::new(),
        }
    }

    pub fn load_from_dir(dir: &Path) -> Result<LoadedKit> {
        let Loaded { set, from_backup } = kit::load(dir)?;
        let (samples, failed) = kit::load_samples(dir, &set);
        let failed_samples = failed
            .into_iter()
            .map(|(path, err)| (path, err.to_string()))
            .collect();
        Ok(LoadedKit {
            set,
            dir: dir.to_path_buf(),
            samples,
            failed_samples,
            from_backup,
        })
    }
}

/// Resolve a sample reference against the set folder and decode it. Slow: run it on a worker,
/// holding no application lock.
///
/// - set-relative path: used as is (must stay inside the folder);
/// - absolute path inside the folder: made relative;
/// - any other absolute path: copied into the folder's `samples/` first.
///
/// Returns the set-relative path (forward slashes) and the decoded sample.
pub fn prepare_sample(dir: &Path, input: &str) -> Result<(String, Arc<Sample>)> {
    let input = input.trim();
    if input.is_empty() {
        bail!("no sample path given");
    }
    let path = Path::new(input);
    let rel = if path.is_absolute() {
        match path.strip_prefix(dir) {
            Ok(inside) => relative_string(inside)?,
            Err(_) => kit::import_sample(dir, path)?,
        }
    } else {
        relative_string(path)?
    };
    let sample = kit::decode_file(&dir.join(&rel))?;
    Ok((rel, Arc::new(sample)))
}

fn relative_string(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => bail!("{} is not a path inside the set folder", path.display()),
        }
    }
    if parts.is_empty() {
        bail!("no sample path given");
    }
    Ok(parts.join("/"))
}
