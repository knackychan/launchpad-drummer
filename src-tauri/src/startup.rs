//! What the app opens at launch: the last set, or why it could not.

use crate::store::LoadedKit;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Startup {
    Loaded {
        path: String,
        name: String,
        from_backup: bool,
        backup_saved_at: Option<String>,
    },
    FirstRun,
    NotFound {
        path: String,
    },
    Unreadable {
        path: String,
        name: String,
    },
}

/// `load` is `KitManager::load_from_dir` in the app, a fake in tests.
pub fn decide_startup(
    last: Option<&Path>,
    load: impl Fn(&Path) -> anyhow::Result<LoadedKit>,
) -> (Startup, Option<LoadedKit>) {
    let Some(dir) = last else {
        return (Startup::FirstRun, None);
    };
    let path = dir.display().to_string();
    if !dir.exists() {
        return (Startup::NotFound { path }, None);
    }
    let name = set_name(dir);
    match load(dir) {
        Ok(kit) => (
            Startup::Loaded {
                path,
                name,
                from_backup: kit.from_backup,
                backup_saved_at: backup_time(dir),
            },
            Some(kit),
        ),
        Err(_) => (Startup::Unreadable { path, name }, None),
    }
}

/// The set's folder name.
pub fn set_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string())
}

/// When `set.json.bak` was written, as local "HH:MM".
pub fn backup_time(dir: &Path) -> Option<String> {
    let modified = std::fs::metadata(dir.join(lpd_core::kit::BACKUP_FILE))
        .and_then(|m| m.modified())
        .ok()?;
    Some(
        chrono::DateTime::<chrono::Local>::from(modified)
            .format("%H:%M")
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::KitManager;
    use lpd_core::kit::{self, Set};

    fn decide(last: Option<&Path>) -> (Startup, Option<LoadedKit>) {
        decide_startup(last, KitManager::load_from_dir)
    }

    #[test]
    fn no_last_set_is_first_run() {
        let (s, kit) = decide(None);
        assert_eq!(s, Startup::FirstRun);
        assert!(kit.is_none());
    }

    #[test]
    fn missing_folder_is_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("gone");
        let (s, kit) = decide(Some(&dir));
        assert_eq!(
            s,
            Startup::NotFound {
                path: dir.display().to_string()
            }
        );
        assert!(kit.is_none());
    }

    #[test]
    fn valid_set_is_loaded() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("My Kit");
        kit::save(&dir, &Set::new()).unwrap();
        let (s, kit) = decide(Some(&dir));
        assert_eq!(
            s,
            Startup::Loaded {
                path: dir.display().to_string(),
                name: "My Kit".into(),
                from_backup: false,
                backup_saved_at: None,
            }
        );
        assert_eq!(kit.unwrap().set, Set::new());
    }

    #[test]
    fn broken_set_with_good_backup_loads_the_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("kit");
        kit::save(&dir, &Set::new()).unwrap();
        kit::save(&dir, &Set::new()).unwrap(); // rotates the first copy into set.json.bak
        std::fs::write(dir.join(kit::SET_FILE), "{ broken").unwrap();
        let (s, kit) = decide(Some(&dir));
        let Startup::Loaded {
            from_backup,
            backup_saved_at,
            ..
        } = s
        else {
            panic!("{s:?}");
        };
        assert!(from_backup);
        let at = backup_saved_at.expect("backup time");
        assert_eq!(at.len(), 5, "{at}");
        assert_eq!(&at[2..3], ":", "{at}");
        assert!(kit.is_some());
    }

    #[test]
    fn both_files_broken_is_unreadable_and_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("kit");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(kit::SET_FILE), "{ broken main").unwrap();
        std::fs::write(dir.join(kit::BACKUP_FILE), "{ broken backup").unwrap();
        let (s, kit) = decide(Some(&dir));
        assert_eq!(
            s,
            Startup::Unreadable {
                path: dir.display().to_string(),
                name: "kit".into()
            }
        );
        assert!(kit.is_none());
        assert_eq!(
            std::fs::read(dir.join(kit::SET_FILE)).unwrap(),
            b"{ broken main"
        );
        assert_eq!(
            std::fs::read(dir.join(kit::BACKUP_FILE)).unwrap(),
            b"{ broken backup"
        );
        assert!(!dir.join(kit::BROKEN_FILE).exists());
    }

    #[test]
    fn serializes_with_kind_tag_and_camel_case_fields() {
        let s = Startup::Loaded {
            path: "p".into(),
            name: "n".into(),
            from_backup: true,
            backup_saved_at: Some("09:41".into()),
        };
        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            serde_json::json!({"kind":"loaded","path":"p","name":"n","fromBackup":true,"backupSavedAt":"09:41"})
        );
        assert_eq!(
            serde_json::to_value(Startup::FirstRun).unwrap(),
            serde_json::json!({"kind":"firstRun"})
        );
    }
}
