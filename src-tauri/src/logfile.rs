//! Append-only app log `lpd.log` in the app log folder, rotated at `MAX_BYTES`.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

pub const MAX_BYTES: u64 = 1_000_000;

/// Older files kept beside `lpd.log`: `lpd.1.log` (newest) … `lpd.3.log`.
const KEEP: u32 = 3;

struct Target {
    dir: PathBuf,
    limit: u64,
}

static TARGET: Mutex<Option<Target>> = Mutex::new(None);

/// Start logging into `dir` (`app.path().app_log_dir()`).
pub fn init(dir: PathBuf) {
    init_with_limit(dir, MAX_BYTES);
}

fn init_with_limit(dir: PathBuf, limit: u64) {
    let _ = std::fs::create_dir_all(&dir);
    *TARGET.lock().unwrap_or_else(PoisonError::into_inner) = Some(Target { dir, limit });
}

/// Append `[YYYY-MM-DD HH:MM:SS] line`. Does nothing before `init`; write errors are ignored.
pub fn log(line: impl AsRef<str>) {
    #[cfg(debug_assertions)]
    eprintln!("{}", line.as_ref());
    let guard = TARGET.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(target) = guard.as_ref() else {
        return;
    };
    let path = target.dir.join("lpd.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() >= target.limit) {
        rotate(target);
    }
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "[{stamp}] {}", line.as_ref());
    }
}

/// lpd.2.log → lpd.3.log, lpd.1.log → lpd.2.log, lpd.log → lpd.1.log (the oldest is replaced).
fn rotate(target: &Target) {
    let name = |n: u32| match n {
        0 => target.dir.join("lpd.log"),
        n => target.dir.join(format!("lpd.{n}.log")),
    };
    for n in (0..KEEP).rev() {
        if name(n).exists() {
            let _ = std::fs::rename(name(n), name(n + 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logs(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    // One test: the log is process-wide state.
    #[test]
    fn rotates_past_the_limit_and_keeps_three_old_files() {
        let tmp = tempfile::tempdir().unwrap();
        init_with_limit(tmp.path().to_path_buf(), 64);
        log("first line");
        let text = std::fs::read_to_string(tmp.path().join("lpd.log")).unwrap();
        assert!(text.starts_with('['), "{text}");
        assert!(text.ends_with("] first line\n"), "{text}");
        assert_eq!(text.len(), "[2026-10-18 12:00:00] first line\n".len());

        for i in 0..3 {
            log(format!("line {i} with some padding"));
        }
        assert!(
            tmp.path().join("lpd.1.log").exists(),
            "{:?}",
            logs(tmp.path())
        );

        for i in 0..40 {
            log(format!("line {i} with some padding"));
        }
        let names = logs(tmp.path());
        assert_eq!(names, ["lpd.1.log", "lpd.2.log", "lpd.3.log", "lpd.log"]);
    }
}
