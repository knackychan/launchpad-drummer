use super::Set;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub const SET_FILE: &str = "set.json";
pub const BACKUP_FILE: &str = "set.json.bak";
pub const BROKEN_FILE: &str = "set.json.broken";
pub const SAMPLES_DIR: &str = "samples";
const TMP_FILE: &str = "set.json.tmp";

#[derive(Debug)]
pub struct Loaded {
    pub set: Set,
    /// true when set.json was unreadable and set.json.bak was used.
    pub from_backup: bool,
}

pub fn load(dir: &Path) -> Result<Loaded> {
    let main = dir.join(SET_FILE);
    let main_err = match read_set(&main) {
        Ok(set) => {
            return Ok(Loaded {
                set,
                from_backup: false,
            })
        }
        Err(e) => e,
    };
    let set = read_set(&dir.join(BACKUP_FILE))
        .with_context(|| format!("set unreadable ({main_err:#}) and backup unreadable"))?;
    if main.exists() {
        let broken = dir.join(BROKEN_FILE);
        if broken.exists() {
            let _ = fs::remove_file(&broken);
        }
        fs::rename(&main, &broken).context("move broken set.json aside")?;
    }
    Ok(Loaded {
        set,
        from_backup: true,
    })
}

fn read_set(path: &Path) -> Result<Set> {
    let s = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Set::from_json(&s).with_context(|| format!("parse {}", path.display()))
}

/// Refuse to create a set where a kit, its backup or recovery files already are.
/// Reads only; call before touching the folder or any active state.
pub fn ensure_new_set_dir(dir: &Path) -> Result<()> {
    for name in [SET_FILE, BACKUP_FILE, BROKEN_FILE, TMP_FILE] {
        let path = dir.join(name);
        // Anything but a clean "not found" (including unreadable) counts as occupied.
        let free = matches!(
            fs::symlink_metadata(&path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound
        );
        if !free {
            anyhow::bail!(
                "{} already contains a set ({name}). Use Open to load it, or pick a different folder.",
                dir.display()
            );
        }
    }
    Ok(())
}

/// Atomic save: tmp file, rotate the previous set.json to set.json.bak, rename tmp.
pub fn save(dir: &Path, set: &Set) -> Result<()> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join(TMP_FILE);
    let main = dir.join(SET_FILE);
    let bak = dir.join(BACKUP_FILE);
    fs::write(&tmp, set.to_json()).with_context(|| format!("write {}", tmp.display()))?;
    if main.exists() {
        if bak.exists() {
            let _ = fs::remove_file(&bak);
        }
        fs::rename(&main, &bak).context("rotate backup")?;
    }
    if main.exists() {
        let _ = fs::remove_file(&main);
    }
    fs::rename(&tmp, &main).context("move new set.json into place")?;
    Ok(())
}

/// Copy `src` into the set's samples folder; returns the set-relative path.
pub fn import_sample(dir: &Path, src: &Path) -> Result<String> {
    let samples = dir.join(SAMPLES_DIR);
    fs::create_dir_all(&samples)?;
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .context("sample name is not valid UTF-8")?;
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("");
    let name_for = |n: u32| {
        let stem = if n == 1 {
            stem.to_string()
        } else {
            format!("{stem}-{n}")
        };
        if ext.is_empty() {
            stem
        } else {
            format!("{stem}.{ext}")
        }
    };
    let mut n = 1;
    while samples.join(name_for(n)).exists() {
        n += 1;
    }
    let name = name_for(n);
    fs::copy(src, samples.join(&name)).with_context(|| format!("copy {}", src.display()))?;
    Ok(format!("{SAMPLES_DIR}/{name}"))
}
