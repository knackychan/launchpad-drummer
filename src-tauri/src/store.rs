//! Kit store and sample loading management.

use anyhow::{bail, Result};
use lpd_core::audio::Sample;
use lpd_core::kit::{self, Loaded, Set};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

    pub fn set_dir(&mut self, dir: PathBuf) {
        self.dir = Some(dir);
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

    /// Save `set` (the router owns it) into the configured set directory.
    pub fn save(&self, set: &Set) -> Result<()> {
        let Some(dir) = self.dir.as_ref() else {
            bail!("no set directory configured");
        };
        kit::save(dir, set)?;
        Ok(())
    }

    pub fn import_sample_file(&mut self, src: &Path) -> Result<(String, Arc<Sample>)> {
        let Some(dir) = self.dir.as_ref() else {
            bail!("no set directory configured");
        };
        let rel_path = kit::import_sample(dir, src)?;
        let decoded = kit::decode_file(&dir.join(&rel_path))?;
        let sample_arc = Arc::new(decoded);
        self.samples.insert(rel_path.clone(), sample_arc.clone());
        Ok((rel_path, sample_arc))
    }
}
