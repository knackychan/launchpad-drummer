//! Save bookkeeping: what is dirty, when the next autosave is due, and what the UI is told.
//! Pure state; the disk work lives in `AppState::run_save`.

use serde::Serialize;
use std::time::{Duration, Instant};

pub const SAVE_DELAY: Duration = Duration::from_secs(1);
pub const RETRY_DELAY: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SaveState {
    Idle,
    Pending,
    Saved,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveStatus {
    pub state: SaveState,
    pub reason: Option<String>,
}

impl SaveStatus {
    fn of(state: SaveState) -> Self {
        Self {
            state,
            reason: None,
        }
    }
}

pub struct Saver {
    enabled: bool,
    /// Identity of the open set; bumped by `reset`. Results for another epoch are ignored.
    epoch: u64,
    /// Bumped by every change; `saved_rev` is the newest change known to be on disk.
    rev: u64,
    saved_rev: u64,
    deadline: Option<Instant>,
    status: SaveStatus,
    reported: SaveStatus,
}

impl Default for Saver {
    fn default() -> Self {
        Self::new()
    }
}

impl Saver {
    pub fn new() -> Self {
        Self {
            enabled: false,
            epoch: 0,
            rev: 0,
            saved_rev: 0,
            deadline: None,
            status: SaveStatus::of(SaveState::Idle),
            reported: SaveStatus::of(SaveState::Idle),
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn rev(&self) -> u64 {
        self.rev
    }

    pub fn status(&self) -> &SaveStatus {
        &self.status
    }

    /// Another set became active: start clean. `enabled` = it has a folder to save into.
    pub fn reset(&mut self, enabled: bool) {
        self.epoch += 1;
        self.enabled = enabled;
        self.rev = 0;
        self.saved_rev = 0;
        self.deadline = None;
        self.status = SaveStatus::of(SaveState::Idle);
    }

    /// The set changed.
    pub fn touch(&mut self, now: Instant) {
        self.rev += 1;
        if !self.enabled {
            return;
        }
        self.deadline = Some(now + SAVE_DELAY);
        if self.status.state != SaveState::Failed {
            self.status = SaveStatus::of(SaveState::Pending);
        }
    }

    pub fn due(&self, now: Instant) -> bool {
        self.enabled && self.deadline.is_some_and(|d| now >= d)
    }

    /// Outcome of a save of revision `rev` of set `epoch`. Only that revision counts as saved;
    /// anything newer stays pending.
    pub fn finish(&mut self, epoch: u64, rev: u64, result: Result<(), String>, now: Instant) {
        if epoch != self.epoch {
            return;
        }
        match result {
            Ok(()) => {
                self.saved_rev = self.saved_rev.max(rev);
                if self.saved_rev >= self.rev {
                    self.deadline = None;
                    self.status = SaveStatus::of(SaveState::Saved);
                } else {
                    self.deadline.get_or_insert(now + SAVE_DELAY);
                    self.status = SaveStatus::of(SaveState::Pending);
                }
            }
            Err(reason) => {
                self.deadline = Some(now + RETRY_DELAY);
                self.status = SaveStatus {
                    state: SaveState::Failed,
                    reason: Some(reason),
                };
            }
        }
    }

    /// The status if it changed since the last call (what to emit to the UI).
    pub fn take_change(&mut self) -> Option<SaveStatus> {
        if self.status == self.reported {
            return None;
        }
        self.reported = self.status.clone();
        Some(self.status.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(t0: Instant, ms: u64) -> Instant {
        t0 + Duration::from_millis(ms)
    }

    fn enabled() -> Saver {
        let mut s = Saver::new();
        s.reset(true);
        s
    }

    #[test]
    fn debounces_and_saves_only_the_captured_revision() {
        let t0 = Instant::now();
        let mut s = enabled();
        s.touch(t0);
        s.touch(at(t0, 500));
        assert_eq!(s.status().state, SaveState::Pending);
        assert!(!s.due(at(t0, 1_400)));
        assert!(s.due(at(t0, 1_500)));

        let (epoch, rev) = (s.epoch(), s.rev());
        s.touch(at(t0, 1_600)); // edit while the write is in flight
        s.finish(epoch, rev, Ok(()), at(t0, 1_700));
        assert_eq!(
            s.status().state,
            SaveState::Pending,
            "newer edit stays dirty"
        );
        assert!(s.due(at(t0, 2_600)));

        let (epoch, rev) = (s.epoch(), s.rev());
        s.finish(epoch, rev, Ok(()), at(t0, 2_700));
        assert_eq!(s.status().state, SaveState::Saved);
        assert!(!s.due(at(t0, 60_000)));
    }

    #[test]
    fn failure_keeps_dirty_state_and_retries() {
        let t0 = Instant::now();
        let mut s = enabled();
        s.touch(t0);
        let (epoch, rev) = (s.epoch(), s.rev());
        s.finish(epoch, rev, Err("disk full".into()), at(t0, 1_000));
        assert_eq!(s.status().state, SaveState::Failed);
        assert_eq!(s.status().reason.as_deref(), Some("disk full"));
        assert!(!s.due(at(t0, 2_900)));
        assert!(s.due(at(t0, 3_000)), "retry 2 s later");

        s.touch(at(t0, 3_100)); // still failed, deadline moves
        assert_eq!(s.status().state, SaveState::Failed);
        assert!(!s.due(at(t0, 3_200)));

        let (epoch, rev) = (s.epoch(), s.rev());
        s.finish(epoch, rev, Ok(()), at(t0, 4_200));
        assert_eq!(s.status().state, SaveState::Saved);
        assert_eq!(s.status().reason, None);
    }

    #[test]
    fn results_for_a_replaced_set_are_ignored() {
        let t0 = Instant::now();
        let mut s = enabled();
        s.touch(t0);
        let (epoch, rev) = (s.epoch(), s.rev());
        s.reset(true);
        s.touch(at(t0, 10));
        s.finish(epoch, rev, Ok(()), at(t0, 20));
        assert_eq!(s.status().state, SaveState::Pending);
        s.finish(epoch, rev, Err("old dir failed".into()), at(t0, 30));
        assert_eq!(s.status().state, SaveState::Pending);
    }

    #[test]
    fn disabled_saver_never_goes_dirty_or_due() {
        let t0 = Instant::now();
        let mut s = Saver::new();
        s.touch(t0);
        assert_eq!(s.status().state, SaveState::Idle);
        assert!(!s.due(at(t0, 60_000)));
    }

    #[test]
    fn take_change_reports_each_change_once() {
        let t0 = Instant::now();
        let mut s = enabled();
        assert_eq!(s.take_change(), None);
        s.touch(t0);
        assert_eq!(s.take_change().unwrap().state, SaveState::Pending);
        assert_eq!(s.take_change(), None);
    }
}
