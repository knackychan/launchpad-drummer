//! Application state coordinating router, lights, kit manager, and audio status.

use crate::audio::AudioStatus;
use crate::lights::LightPainter;
use crate::router::Router;
use crate::store::KitManager;
use lpd_core::audio::{Command, Event};
use lpd_core::devices::launchpad::LpEvent;
use lpd_core::kit::Set;
use rtrb::{Consumer, Producer};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub router: Arc<Mutex<Router>>,
    pub painter: Arc<Mutex<LightPainter>>,
    pub kit_manager: Arc<Mutex<KitManager>>,
    pub audio_status: Arc<Mutex<AudioStatus>>,
}

impl AppState {
    pub fn new(set: Set, audio_tx: Producer<Command>, audio_rx: Consumer<Event>) -> Self {
        let router = Router::new(set.clone(), Default::default(), audio_tx, audio_rx);
        let kit_manager = KitManager {
            set,
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
        }
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
        if let Ok(mut r) = self.router.lock() {
            r.update_pad(bank, pad.clone());
        }
        if let Ok(mut km) = self.kit_manager.lock() {
            if bank < km.set.banks.len() {
                km.set.banks[bank].set_pad(pad);
            }
        }
    }

    pub fn remove_pad(&self, bank: usize, x: u8, y: u8) {
        if let Ok(mut r) = self.router.lock() {
            r.remove_pad(bank, x, y);
        }
        if let Ok(mut km) = self.kit_manager.lock() {
            if bank < km.set.banks.len() {
                km.set.banks[bank].pads.retain(|p| !(p.x == x && p.y == y));
            }
        }
    }

    pub fn update_group(&self, group: usize, g: lpd_core::kit::Group) {
        if let Ok(mut r) = self.router.lock() {
            r.update_group(group, g.clone());
        }
        if let Ok(mut km) = self.kit_manager.lock() {
            if group < km.set.mixer.groups.len() {
                km.set.mixer.groups[group] = g;
            }
        }
    }

    pub fn update_master_db(&self, master_db: f32) {
        if let Ok(mut r) = self.router.lock() {
            r.update_master_db(master_db);
        }
        if let Ok(mut km) = self.kit_manager.lock() {
            km.set.mixer.master_db = master_db;
        }
    }

    pub fn update_bank_lights(&self, bank: usize, lights: lpd_core::kit::LightModes) {
        if let Ok(mut r) = self.router.lock() {
            r.update_bank_lights(bank, lights);
        }
        if let Ok(mut km) = self.kit_manager.lock() {
            if bank < km.set.banks.len() {
                km.set.banks[bank].lights = lights;
            }
        }
    }

    pub fn rename_bank(&self, bank: usize, name: String) {
        if let Ok(mut r) = self.router.lock() {
            r.rename_bank(bank, name.clone());
        }
        if let Ok(mut km) = self.kit_manager.lock() {
            if bank < km.set.banks.len() {
                km.set.banks[bank].name = name;
            }
        }
    }

    pub fn load_set_dir(&self, dir_str: &str) -> Result<Set, String> {
        let dir = std::path::Path::new(dir_str);
        let loaded = KitManager::load_from_dir(dir).map_err(|e| e.to_string())?;
        if let Ok(mut km) = self.kit_manager.lock() {
            km.set = loaded.set.clone();
            km.dir = Some(loaded.dir.clone());
            km.samples = loaded.samples.clone();
            km.failed_samples = loaded.failed_samples.clone();
        }
        if let Ok(mut r) = self.router.lock() {
            r.set_current_set(loaded.set.clone(), loaded.samples);
        }
        Ok(loaded.set)
    }

    pub fn new_set_dir(&self, dir_str: &str) -> Result<Set, String> {
        let dir = std::path::Path::new(dir_str);
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let set = Set::new();
        lpd_core::kit::save(dir, &set).map_err(|e| e.to_string())?;
        if let Ok(mut km) = self.kit_manager.lock() {
            km.set = set.clone();
            km.dir = Some(dir.to_path_buf());
            km.samples.clear();
            km.failed_samples.clear();
        }
        if let Ok(mut r) = self.router.lock() {
            r.set_current_set(set.clone(), Default::default());
        }
        Ok(set)
    }

    pub fn save_set(&self) -> Result<(), String> {
        if let Ok(km) = self.kit_manager.lock() {
            km.save().map_err(|e| e.to_string())
        } else {
            Err("failed to acquire kit manager lock".into())
        }
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
