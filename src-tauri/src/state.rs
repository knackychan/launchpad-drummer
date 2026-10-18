//! Application state coordinating router, lights, kit manager, and audio status.

use crate::audio::AudioStatus;
use crate::lights::LightPainter;
use crate::router::{mixer_audio_differs, Router};
use crate::store::KitManager;
use lpd_core::audio::{Command, Event};
use lpd_core::devices::launchpad::LpEvent;
use lpd_core::kit::{History, Set};
use rtrb::{Consumer, Producer};
use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplePeaks {
    pub peaks: Vec<f32>,
    pub length_ms: f32,
    pub rate: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryState {
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub router: Arc<Mutex<Router>>,
    pub painter: Arc<Mutex<LightPainter>>,
    pub kit_manager: Arc<Mutex<KitManager>>,
    pub audio_status: Arc<Mutex<AudioStatus>>,
    /// Undo/redo of kit edits; lock after `router`.
    pub history: Arc<Mutex<History>>,
}

impl AppState {
    pub fn new(set: Set, audio_tx: Producer<Command>, audio_rx: Consumer<Event>) -> Self {
        let router = Router::new(set, Default::default(), audio_tx, audio_rx);
        let kit_manager = KitManager {
            dir: None,
            samples: Default::default(),
            failed_samples: Default::default(),
        };
        Self {
            router: Arc::new(Mutex::new(router)),
            painter: Arc::new(Mutex::new(LightPainter::new())),
            kit_manager: Arc::new(Mutex::new(kit_manager)),
            audio_status: Arc::new(Mutex::new(AudioStatus {
                state: "stopped".into(),
                device_name: "none".into(),
                sample_rate: 48_000,
                buffer_size: None,
                channels: 2,
            })),
            history: Arc::new(Mutex::new(History::new())),
        }
    }

    /// The one kit-edit path: records undo (coalescing by `key`), applies `f` to the
    /// router's set, re-sends mixer commands when mixer values changed and marks the set dirty.
    /// An edit that leaves the set unchanged adds no undo step.
    pub fn edit<R>(&self, key: Option<&str>, f: impl FnOnce(&mut Set) -> R) -> R {
        let mut r = self.router.lock().unwrap_or_else(PoisonError::into_inner);
        let before = r.set().clone();
        let out = f(r.set_mut());
        if *r.set() != before {
            self.history
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .record(&before, key, Instant::now());
            if mixer_audio_differs(&before, r.set()) {
                r.send_mixer();
            }
        }
        drop(r);
        self.touch();
        out
    }

    /// Mark the set dirty for autosave (Task 2 hooks the saver here).
    fn touch(&self) {}

    pub fn undo(&self) -> Option<Set> {
        self.step_history(|h, current| h.undo(current))
    }

    pub fn redo(&self) -> Option<Set> {
        self.step_history(|h, current| h.redo(current))
    }

    fn step_history(&self, step: impl FnOnce(&mut History, &Set) -> Option<Set>) -> Option<Set> {
        let mut r = self.router.lock().unwrap_or_else(PoisonError::into_inner);
        let set = {
            let mut h = self.history.lock().unwrap_or_else(PoisonError::into_inner);
            step(&mut h, r.set())?
        };
        r.install_set(set.clone());
        drop(r);
        self.touch();
        Some(set)
    }

    pub fn history_state(&self) -> HistoryState {
        let h = self.history.lock().unwrap_or_else(PoisonError::into_inner);
        HistoryState {
            can_undo: h.can_undo(),
            can_redo: h.can_redo(),
        }
    }

    fn clear_history(&self) {
        self.history
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    pub fn trigger_pad(&self, x: u8, y: u8) {
        if let Ok(mut r) = self.router.lock() {
            r.trigger_pad(x, y);
            if let Ok(mut p) = self.painter.lock() {
                p.on_pad_hit(x, y, Instant::now());
            }
        }
    }

    pub fn release_pad(&self, x: u8, y: u8) {
        if let Ok(mut r) = self.router.lock() {
            r.release_pad(x, y);
        }
    }

    pub fn select_bank(&self, bank: usize) -> bool {
        if let Ok(mut r) = self.router.lock() {
            r.select_bank(bank)
        } else {
            false
        }
    }

    pub fn current_bank(&self) -> usize {
        if let Ok(r) = self.router.lock() {
            r.bank()
        } else {
            0
        }
    }

    pub fn stop_all(&self) {
        if let Ok(mut r) = self.router.lock() {
            r.stop_all();
        }
    }

    pub fn on_lp_event(&self, ev: LpEvent) {
        if let Ok(mut r) = self.router.lock() {
            if let LpEvent::PadDown { x, y } = ev {
                if let Ok(mut p) = self.painter.lock() {
                    p.on_pad_hit(x, y, Instant::now());
                }
            }
            r.on_lp_event(ev);
        }
    }

    pub fn paint_frame(&self, now: Instant) -> Vec<(u8, [u8; 3])> {
        let (set, bank, loaded_samples) = if let Ok(r) = self.router.lock() {
            let loaded: HashSet<String> = r.samples().keys().cloned().collect();
            (r.set().clone(), r.bank(), loaded)
        } else {
            return vec![];
        };
        let is_sounding = |b: usize, x: u8, y: u8| -> bool {
            if let Ok(r) = self.router.lock() {
                r.is_pad_sounding(b, x, y)
            } else {
                false
            }
        };
        if let Ok(mut p) = self.painter.lock() {
            p.paint(&set, bank, &loaded_samples, &is_sounding, now)
        } else {
            vec![]
        }
    }

    pub fn set_audio_status(&self, status: AudioStatus) {
        if let Ok(mut stat) = self.audio_status.lock() {
            *stat = status;
        }
    }

    pub fn get_audio_status(&self) -> AudioStatus {
        if let Ok(s) = self.audio_status.lock() {
            s.clone()
        } else {
            AudioStatus {
                state: "unknown".into(),
                device_name: "none".into(),
                sample_rate: 48_000,
                buffer_size: None,
                channels: 2,
            }
        }
    }

    pub fn get_current_set(&self) -> Set {
        if let Ok(r) = self.router.lock() {
            r.set().clone()
        } else {
            Set::new()
        }
    }

    pub fn update_pad(&self, bank: usize, pad: lpd_core::kit::Pad) {
        self.edit(None, |set| {
            if let Some(b) = set.banks.get_mut(bank) {
                b.set_pad(pad);
            }
        });
    }

    pub fn remove_pad(&self, bank: usize, x: u8, y: u8) {
        self.edit(None, |set| {
            if let Some(b) = set.banks.get_mut(bank) {
                b.pads.retain(|p| !(p.x == x && p.y == y));
            }
        });
    }

    /// A rename is an undoable kit edit; value changes (volume, pan, ...) are not (UI spec 7.3).
    pub fn update_group(&self, group: usize, g: lpd_core::kit::Group) {
        let renamed = self
            .router
            .lock()
            .ok()
            .and_then(|r| {
                r.set()
                    .mixer
                    .groups
                    .get(group)
                    .map(|old| old.name != g.name)
            })
            .unwrap_or(false);
        if renamed {
            let name = g.name.clone();
            self.edit(None, |set| set.mixer.groups[group].name = name);
        }
        if let Ok(mut r) = self.router.lock() {
            r.update_group(group, g);
        }
        self.touch();
    }

    pub fn update_master_db(&self, master_db: f32) {
        if let Ok(mut r) = self.router.lock() {
            r.update_master_db(master_db);
        }
        self.touch();
    }

    pub fn update_bank_lights(&self, bank: usize, lights: lpd_core::kit::LightModes) {
        self.edit(None, |set| {
            if let Some(b) = set.banks.get_mut(bank) {
                b.lights = lights;
            }
        });
    }

    pub fn update_bank_humanize(&self, bank: usize, humanize_db: f32) {
        let humanize_db = humanize_db.clamp(0.0, 6.0);
        self.edit(None, |set| {
            if let Some(b) = set.banks.get_mut(bank) {
                b.humanize_db = humanize_db;
            }
        });
    }

    /// Peak envelope of a loaded sample for the editor waveform: `buckets` max-abs values
    /// over both channels. None when the sample is not loaded.
    pub fn sample_peaks(&self, path: &str, buckets: usize) -> Option<SamplePeaks> {
        let sample = self.kit_manager.lock().ok()?.samples.get(path)?.clone();
        let frames = sample.frames();
        let buckets = buckets.clamp(16, 2048);
        let mut peaks = vec![0.0f32; buckets];
        if frames > 0 {
            for (i, peak) in peaks.iter_mut().enumerate() {
                let start = i * frames / buckets;
                let end = ((i + 1) * frames / buckets).clamp(start + 1, frames);
                for f in start..end {
                    let (l, r) = sample.frame(f as isize);
                    *peak = peak.max(l.abs()).max(r.abs());
                }
            }
        }
        Some(SamplePeaks {
            peaks,
            length_ms: frames as f32 * 1000.0 / sample.rate.max(1) as f32,
            rate: sample.rate,
        })
    }

    pub fn rename_bank(&self, bank: usize, name: String) {
        self.edit(None, |set| {
            if let Some(b) = set.banks.get_mut(bank) {
                b.name = name;
            }
        });
    }

    pub fn load_set_dir(&self, dir_str: &str) -> Result<Set, String> {
        let dir = std::path::Path::new(dir_str);
        let loaded = KitManager::load_from_dir(dir).map_err(|e| e.to_string())?;
        if let Ok(mut km) = self.kit_manager.lock() {
            km.dir = Some(loaded.dir.clone());
            km.samples = loaded.samples.clone();
            km.failed_samples = loaded.failed_samples.clone();
        }
        if let Ok(mut r) = self.router.lock() {
            r.set_current_set(loaded.set.clone(), loaded.samples);
        }
        self.clear_history();
        Ok(loaded.set)
    }

    pub fn new_set_dir(&self, dir_str: &str) -> Result<Set, String> {
        let dir = std::path::Path::new(dir_str);
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let set = Set::new();
        lpd_core::kit::save(dir, &set).map_err(|e| e.to_string())?;
        if let Ok(mut km) = self.kit_manager.lock() {
            km.dir = Some(dir.to_path_buf());
            km.samples.clear();
            km.failed_samples.clear();
        }
        if let Ok(mut r) = self.router.lock() {
            r.set_current_set(set.clone(), Default::default());
        }
        self.clear_history();
        Ok(set)
    }

    pub fn save_set(&self) -> Result<(), String> {
        let r = self
            .router
            .lock()
            .map_err(|_| "failed to acquire router lock".to_string())?;
        let km = self
            .kit_manager
            .lock()
            .map_err(|_| "failed to acquire kit manager lock".to_string())?;
        km.save(r.set()).map_err(|e| e.to_string())
    }

    pub fn import_sample(&self, src: &str) -> Result<String, String> {
        let src_path = std::path::Path::new(src);
        let (rel_path, sample) = if let Ok(mut km) = self.kit_manager.lock() {
            km.import_sample_file(src_path).map_err(|e| e.to_string())?
        } else {
            return Err("failed to acquire kit manager lock".into());
        };
        if let Ok(mut r) = self.router.lock() {
            r.samples_mut().insert(rel_path.clone(), sample);
        }
        Ok(rel_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpd_core::kit::Pad;
    use rtrb::RingBuffer;

    fn app() -> (AppState, Consumer<Command>) {
        let (tx, cmd_rx) = RingBuffer::new(1024);
        let (_evt_tx, evt_rx) = RingBuffer::new(16);
        (AppState::new(Set::new(), tx, evt_rx), cmd_rx)
    }

    #[test]
    fn edit_then_undo_restores_pad() {
        let (state, _rx) = app();
        state.edit(None, |set| {
            set.banks[0].set_pad(Pad::new(1, 2, "samples/a.wav", [1, 2, 3]))
        });
        assert!(state.get_current_set().banks[0].pad(1, 2).is_some());
        assert!(state.history_state().can_undo);

        let undone = state.undo().expect("one step");
        assert!(undone.banks[0].pad(1, 2).is_none());
        assert!(state.get_current_set().banks[0].pad(1, 2).is_none());
        assert!(state.history_state().can_redo);

        let redone = state.redo().expect("redo step");
        assert!(redone.banks[0].pad(1, 2).is_some());
        assert!(state.get_current_set().banks[0].pad(1, 2).is_some());
    }

    #[test]
    fn update_pad_command_is_undoable() {
        let (state, _rx) = app();
        state.update_pad(3, Pad::new(0, 0, "samples/a.wav", [1, 2, 3]));
        assert!(state.get_current_set().banks[3].pad(0, 0).is_some());
        state.undo().unwrap();
        assert!(state.get_current_set().banks[3].pad(0, 0).is_none());
    }

    #[test]
    fn group_volume_change_is_not_undoable() {
        let (state, _rx) = app();
        let mut g = state.get_current_set().mixer.groups[0].clone();
        g.volume_db = -6.0;
        state.update_group(0, g);
        state.update_master_db(-3.0);
        assert_eq!(state.get_current_set().mixer.groups[0].volume_db, -6.0);
        assert_eq!(state.get_current_set().mixer.master_db, -3.0);
        assert!(!state.history_state().can_undo);
        assert_eq!(state.undo(), None);
    }

    #[test]
    fn group_rename_is_undoable_but_keeps_volume() {
        let (state, _rx) = app();
        let mut g = state.get_current_set().mixer.groups[1].clone();
        g.name = "Snares".into();
        state.update_group(1, g.clone());
        assert!(state.history_state().can_undo);
        g.volume_db = -9.0;
        state.update_group(1, g);
        let undone = state.undo().unwrap();
        assert_eq!(undone.mixer.groups[1].name, "Group 2");
        assert_eq!(undone.mixer.groups[1].volume_db, -9.0);
    }
}
