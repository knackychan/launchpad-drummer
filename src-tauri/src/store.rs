//! Kit store and sample loading management.

use anyhow::{bail, Result};
use lpd_core::audio::Sample;
use lpd_core::kit::{self, Loaded, Set};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

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
