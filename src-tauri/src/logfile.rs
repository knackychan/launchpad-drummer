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
    /// Tests only: the thread whose lines are logged, so other tests running in parallel cannot
    /// write into the file a test is checking. `None` in the app: every thread logs.
    owner: Option<std::thread::ThreadId>,
}

static TARGET: Mutex<Option<Target>> = Mutex::new(None);

/// Start logging into `dir` (`app.path().app_log_dir()`).
pub fn init(dir: PathBuf) {
    init_with_limit(dir, MAX_BYTES);
}

fn init_with_limit(dir: PathBuf, limit: u64) {
    let _ = std::fs::create_dir_all(&dir);
    let owner = cfg!(test).then(|| std::thread::current().id());
    *TARGET.lock().unwrap_or_else(PoisonError::into_inner) = Some(Target { dir, limit, owner });
}

static PROCESS_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Pin the zero of the startup clock. Call first thing in `run`; later calls change nothing.
pub fn mark_process_start() {
    let _ = PROCESS_START.set(std::time::Instant::now());
}

/// Log `Startup +123.4 ms name`: milliseconds on the monotonic clock since `mark_process_start`.
/// Never call it from an audio callback (it writes to the log file).
pub fn milestone(name: &str) {
    let start = *PROCESS_START.get_or_init(std::time::Instant::now);
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    log(format!("Startup +{ms:.1} ms {name}"));
}

/// Append `[YYYY-MM-DD HH:MM:SS] line`. Does nothing before `init`; write errors are ignored.
pub fn log(line: impl AsRef<str>) {
    #[cfg(debug_assertions)]
    eprintln!("{}", line.as_ref());
    let guard = TARGET.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(target) = guard.as_ref() else {
        return;
    };
    if target
        .owner
        .is_some_and(|o| o != std::thread::current().id())
    {
        return;
    }
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
/// lpd.log is moved out of the way first; when that fails nothing is shifted, so no history is
/// lost, and the next call tries again.
fn rotate(target: &Target) {
    let name = |n: u32| target.dir.join(format!("lpd.{n}.log"));
    let current = target.dir.join("lpd.log");
    let moving = target.dir.join("lpd.log.rotating");
    if std::fs::rename(&current, &moving).is_err() {
        return;
    }
    for n in (1..KEEP).rev() {
        if name(n).exists() {
            let _ = std::fs::rename(name(n), name(n + 1));
        }
    }
    let _ = std::fs::rename(&moving, name(1));
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

        // lpd.log cannot be renamed (held open without delete sharing): older files stay put.
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let read = |n: &str| std::fs::read(tmp.path().join(n)).unwrap();
            let before = [read("lpd.1.log"), read("lpd.2.log"), read("lpd.3.log")];
            let holder = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(1 | 2) // FILE_SHARE_READ | FILE_SHARE_WRITE
                .open(tmp.path().join("lpd.log"))
                .unwrap();
            for i in 0..10 {
                log(format!("held line {i} with some padding"));
            }
            assert_eq!(
                [read("lpd.1.log"), read("lpd.2.log"), read("lpd.3.log")],
                before,
                "no history lost"
            );
            assert_eq!(logs(tmp.path()), names);
            drop(holder);
            log("released");
            assert!(String::from_utf8(read("lpd.1.log"))
                .unwrap()
                .contains("held line 9"));
        }
    }
}
