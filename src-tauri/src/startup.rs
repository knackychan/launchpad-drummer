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

/// Which step of a set operation (launch restore, New, Open, Default) is running.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LoadPhase {
    Settings,
    ReadingSet,
    PreparingSamples,
    LoadingSamples,
    Installing,
    Ready,
    NeedsSet,
    Failed,
}

impl LoadPhase {
    /// The request is over: nothing more will be reported for it.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Ready | Self::NeedsSet | Self::Failed)
    }
}

/// Progress of the newest set operation, as the UI sees it (`get_load_status`, `load-status`).
/// `Startup` stays the outcome; this is the operation. `request_id` is the open ticket.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoadStatus {
    pub request_id: u64,
    /// Rises with every accepted update in the process; the UI keeps only strictly newer ones.
    pub revision: u64,
    pub phase: LoadPhase,
    pub completed: u64,
    /// `None`: unknown.
    pub total: Option<u64>,
    /// Samples that failed to decode so far (not a fatal error).
    pub failed: u64,
    pub error: Option<String>,
}

impl LoadStatus {
    pub fn initial() -> Self {
        Self {
            request_id: 0,
            revision: 0,
            phase: LoadPhase::Settings,
            completed: 0,
            total: None,
            failed: 0,
            error: None,
        }
    }
}

/// One report from a running operation.
#[derive(Clone, Debug)]
pub struct LoadStep {
    pub phase: LoadPhase,
    pub completed: u64,
    pub total: Option<u64>,
    pub failed: u64,
    pub error: Option<String>,
}

impl LoadStep {
    /// A phase with no count (indeterminate).
    pub fn phase(phase: LoadPhase) -> Self {
        Self {
            phase,
            completed: 0,
            total: None,
            failed: 0,
            error: None,
        }
    }

    /// A phase counting files: `completed` of `total`, `failed` of them failed.
    pub fn counted(phase: LoadPhase, completed: usize, total: usize, failed: usize) -> Self {
        Self {
            phase,
            completed: completed as u64,
            total: Some(total as u64),
            failed: failed as u64,
            error: None,
        }
    }

    /// The set is installed; `failed` samples did not decode (a warning, not an error).
    pub fn ready(failed: usize) -> Self {
        Self {
            failed: failed as u64,
            ..Self::phase(LoadPhase::Ready)
        }
    }

    /// The operation ended with a fatal error.
    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            ..Self::phase(LoadPhase::Failed)
        }
    }
}

/// Progress events are sent at most this often within one phase.
pub const PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

/// The newest request's status plus the emission throttle. Pure: the caller passes the clock and
/// sends what [`LoadTracker::update`] returns after releasing its lock.
pub struct LoadTracker {
    status: LoadStatus,
    last_emit: Option<std::time::Instant>,
}

impl Default for LoadTracker {
    fn default() -> Self {
        Self {
            status: LoadStatus::initial(),
            last_emit: None,
        }
    }
}

impl LoadTracker {
    pub fn status(&self) -> &LoadStatus {
        &self.status
    }

    /// `request_id` is now the newest request, in `phase`. Always worth sending.
    pub fn begin(&mut self, request_id: u64, phase: LoadPhase) -> LoadStatus {
        self.status = LoadStatus {
            request_id,
            revision: self.status.revision + 1,
            phase,
            completed: 0,
            total: None,
            failed: 0,
            error: None,
        };
        self.last_emit = None;
        self.status.clone()
    }

    /// Record progress for `request_id`. `None` when it is no longer the newest request (the
    /// caller was superseded and must not publish). Otherwise `Some(send)`: the status is always
    /// stored, and `send` is the copy to emit, or `None` while the throttle holds it back.
    /// A phase change, a finished count and a terminal phase are always sent.
    pub fn update(
        &mut self,
        request_id: u64,
        step: LoadStep,
        now: std::time::Instant,
    ) -> Option<Option<LoadStatus>> {
        if self.status.request_id != request_id {
            return None;
        }
        let LoadStep {
            phase,
            completed,
            total,
            failed,
            error,
        } = step;
        let phase_changed = self.status.phase != phase;
        let finished = total.is_some_and(|t| completed >= t);
        self.status = LoadStatus {
            request_id,
            revision: self.status.revision + 1,
            phase,
            completed,
            total,
            failed,
            error,
        };
        let due = self
            .last_emit
            .is_none_or(|at| now.saturating_duration_since(at) >= PROGRESS_INTERVAL);
        if phase_changed || phase.is_terminal() || finished || due {
            self.last_emit = Some(now);
            Some(Some(self.status.clone()))
        } else {
            Some(None)
        }
    }
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
