//! Undo/redo stack of `Set` snapshots.

use super::Set;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub const UNDO_STEPS: usize = 50;
const COALESCE_WINDOW: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct History {
    undo: VecDeque<Set>,
    redo: Vec<Set>,
    last_key: Option<String>,
    last_at: Option<Instant>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Call BEFORE applying an edit with the set as it was. Same `key` within 1 s of the
    /// previous record = coalesced (no new step; e.g. one knob drag). `None` never coalesces.
    pub fn record(&mut self, before: &Set, key: Option<&str>, now: Instant) {
        let coalesce = match (key, self.last_key.as_deref(), self.last_at) {
            (Some(k), Some(last), Some(at)) => {
                k == last && now.saturating_duration_since(at) < COALESCE_WINDOW
            }
            _ => false,
        };
        self.redo.clear();
        self.last_key = key.map(str::to_owned);
        self.last_at = Some(now);
        if coalesce && !self.undo.is_empty() {
            return;
        }
        self.push_undo(before.clone());
    }

    pub fn undo(&mut self, current: &Set) -> Option<Set> {
        let snapshot = self.undo.pop_back()?;
        self.redo.push(current.clone());
        self.break_coalescing();
        Some(restore_keeping_mixer(snapshot, current))
    }

    pub fn redo(&mut self, current: &Set) -> Option<Set> {
        let snapshot = self.redo.pop()?;
        self.push_undo(current.clone());
        self.break_coalescing();
        Some(restore_keeping_mixer(snapshot, current))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.break_coalescing();
    }

    fn push_undo(&mut self, set: Set) {
        if self.undo.len() == UNDO_STEPS {
            self.undo.pop_front();
        }
        self.undo.push_back(set);
    }

    fn break_coalescing(&mut self) {
        self.last_key = None;
        self.last_at = None;
    }
}

/// Mixer values are not covered by undo (UI spec 7.3): the restored set keeps `current`'s
/// group volume/pan/pitch/lowpass/mute/solo and master, but takes group *names* from `snapshot`.
pub fn restore_keeping_mixer(snapshot: Set, current: &Set) -> Set {
    let mut restored = snapshot;
    let names: Vec<String> = restored
        .mixer
        .groups
        .iter()
        .map(|g| g.name.clone())
        .collect();
    restored.mixer = current.mixer.clone();
    for (g, name) in restored.mixer.groups.iter_mut().zip(names) {
        g.name = name;
    }
    restored
}
