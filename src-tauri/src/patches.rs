//! User patch library; call with no application lock held.
use lpd_core::synth::{
    factory::{factory, FACTORY_IDS},
    patch::{clean_name, file_name, Category, Patch},
};
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PatchRef {
    pub id: String,
    pub name: String,
    pub category: Category,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct PatchList {
    pub factory: Vec<PatchRef>,
    pub user: Vec<PatchRef>,
}

fn path(dir: &Path, file: &str) -> Result<PathBuf, String> {
    if file.contains("..")
        || file
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || !file.to_ascii_lowercase().ends_with(".lbpatch")
    {
        return Err("Not a patch file name".into());
    }
    Ok(dir.join(file))
}

fn reference(id: String, patch: &Patch) -> PatchRef {
    PatchRef {
        id,
        name: patch.name.clone(),
        category: patch.category,
    }
}

pub fn factory_list() -> Vec<PatchRef> {
    FACTORY_IDS
        .iter()
        .map(|id| reference((*id).into(), &factory(id).expect("known factory id")))
        .collect()
}

pub fn list(dir: &Path) -> Vec<PatchRef> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut patches: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            read(dir, &name).ok().map(|p| reference(name, &p))
        })
        .collect();
    patches.sort_by(|a, b| a.name.cmp(&b.name));
    patches
}

pub fn read(dir: &Path, file: &str) -> Result<Patch, String> {
    let json = fs::read_to_string(path(dir, file)?).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "Patch file not found".into()
        } else {
            e.to_string()
        }
    })?;
    Patch::from_json(&json).map_err(|e| e.to_string())
}

pub fn write(dir: &Path, patch: &Patch, overwrite: bool) -> Result<String, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let file = file_name(&patch.name);
    let target = path(dir, &file)?;
    let tmp = dir.join(format!(
        ".{file}.{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> std::io::Result<()> {
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        out.write_all(patch.to_json().as_bytes())?;
        out.sync_all()?;
        drop(out);
        if overwrite {
            fs::rename(&tmp, &target)
        } else {
            // An exclusive link commits a new file atomically without a check/rename race.
            fs::hard_link(&tmp, &target)
        }
    })();
    let _ = fs::remove_file(&tmp);
    result.map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            "exists".into()
        } else {
            e.to_string()
        }
    })?;
    Ok(file)
}

pub fn rename(dir: &Path, file: &str, name: &str) -> Result<PatchRef, String> {
    let old = path(dir, file)?;
    let mut patch = read(dir, file)?;
    patch.name = clean_name(name);
    let same = file.eq_ignore_ascii_case(&file_name(&patch.name));
    let new = write(dir, &patch, same)?;
    if !same {
        fs::remove_file(old).map_err(|e| e.to_string())?;
    }
    Ok(reference(new, &patch))
}

pub fn delete(dir: &Path, file: &str) -> Result<(), String> {
    fs::remove_file(path(dir, file)?).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "Patch file not found".into()
        } else {
            e.to_string()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpd_core::synth::{
        params::SynthParams,
        patch::{Category, Patch},
    };

    #[test]
    fn write_then_list_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let patch = Patch::new("TEST", Category::Bass, SynthParams::default());
        let file = write(dir.path(), &patch, false).unwrap();
        assert_eq!(read(dir.path(), &file).unwrap(), patch);
        assert_eq!(
            list(dir.path()),
            vec![PatchRef {
                id: file,
                name: "TEST".into(),
                category: Category::Bass
            }]
        );
    }

    #[test]
    fn writing_an_existing_name_needs_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let patch = Patch::init();
        write(dir.path(), &patch, false).unwrap();
        assert_eq!(write(dir.path(), &patch, false), Err("exists".into()));
        assert!(write(dir.path(), &patch, true).is_ok());
    }

    #[test]
    fn rename_moves_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let old = write(dir.path(), &Patch::init(), false).unwrap();
        let renamed = rename(dir.path(), &old, "NEW NAME").unwrap();
        assert!(!dir.path().join(old).exists());
        assert_eq!(renamed.name, "NEW NAME");
        assert_eq!(list(dir.path()), vec![renamed]);
    }

    #[test]
    fn delete_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = write(dir.path(), &Patch::init(), false).unwrap();
        delete(dir.path(), &file).unwrap();
        assert!(list(dir.path()).is_empty());
    }

    #[test]
    fn unsafe_file_names_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        for file in [
            "..\\x.lbpatch",
            "../x.lbpatch",
            "A..B.lbpatch",
            "C:x.lbpatch",
        ] {
            assert_eq!(read(dir.path(), file), Err("Not a patch file name".into()));
            assert!(delete(dir.path(), file).is_err());
            assert!(rename(dir.path(), file, "X").is_err());
        }
    }

    #[test]
    fn unreadable_files_are_skipped_in_the_list() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("BAD.lbpatch"), "not json").unwrap();
        std::fs::write(dir.path().join("NOT A PATCH.txt"), Patch::init().to_json()).unwrap();
        assert!(list(dir.path()).is_empty());
        assert!(list(&dir.path().join("missing")).is_empty());
    }
}
