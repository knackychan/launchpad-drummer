//! Router: single owner of set state, bank selection, and high-priority MIDI-to-audio dispatch.

use lpd_core::audio::{db_to_gain, Command, Event, Sample};
use lpd_core::devices::launchpad::LpEvent;
use lpd_core::kit::{self, Set};
use rtrb::{Consumer, Producer};
use std::collections::HashMap;
use std::sync::Arc;

const TOP_MIXER: u8 = 7;

pub struct Router {
    set: Set,
    samples: HashMap<String, Arc<Sample>>,
    bank: usize,
    held: HashMap<(u8, u8), u16>,
    voice_counts: HashMap<u16, usize>,
    audio_tx: Producer<Command>,
    audio_rx: Consumer<Event>,
}

impl Router {
    pub fn new(
        set: Set,
        samples: HashMap<String, Arc<Sample>>,
        mut audio_tx: Producer<Command>,
        audio_rx: Consumer<Event>,
    ) -> Self {
        Self::send_mixer_commands(&set, &mut audio_tx);
        Self {
            set,
            samples,
            bank: 0,
            held: HashMap::new(),
            voice_counts: HashMap::new(),
            audio_tx,
            audio_rx,
        }
    }

    pub fn bank(&self) -> usize {
        self.bank
    }

    pub fn set(&self) -> &Set {
        &self.set
    }

    pub fn set_mut(&mut self) -> &mut Set {
        &mut self.set
    }

    pub fn samples(&self) -> &HashMap<String, Arc<Sample>> {
        &self.samples
    }

    pub fn samples_mut(&mut self) -> &mut HashMap<String, Arc<Sample>> {
        &mut self.samples
    }

    pub fn update_group(&mut self, group: usize, g: kit::Group) {
        if group < self.set.mixer.groups.len() {
            let params = kit::group_params(&g);
            self.set.mixer.groups[group] = g;
            let _ = self.audio_tx.push(Command::SetGroup {
                group: group as u8,
                params,
            });
        }
    }

    pub fn update_master_db(&mut self, master_db: f32) {
        self.set.mixer.master_db = master_db;
        let _ = self
            .audio_tx
            .push(Command::SetMasterGain(db_to_gain(master_db)));
    }

    pub fn update_pad(&mut self, bank: usize, pad: kit::Pad) {
        if bank < self.set.banks.len() {
            self.set.banks[bank].set_pad(pad);
        }
    }

    pub fn remove_pad(&mut self, bank: usize, x: u8, y: u8) {
        if bank < self.set.banks.len() {
            self.set.banks[bank]
                .pads
                .retain(|p| !(p.x == x && p.y == y));
        }
    }

    pub fn update_bank_lights(&mut self, bank: usize, lights: kit::LightModes) {
        if bank < self.set.banks.len() {
            self.set.banks[bank].lights = lights;
        }
    }

    pub fn update_bank_humanize(&mut self, bank: usize, humanize_db: f32) {
        if bank < self.set.banks.len() {
            self.set.banks[bank].humanize_db = humanize_db;
        }
    }

    pub fn rename_bank(&mut self, bank: usize, name: String) {
        if bank < self.set.banks.len() {
            self.set.banks[bank].name = name;
        }
    }

    pub fn pad_id(bank: usize, x: u8, y: u8) -> u16 {
        (bank * 64 + y as usize * 8 + x as usize) as u16
    }

    pub fn is_pad_sounding(&self, bank: usize, x: u8, y: u8) -> bool {
        let id = Self::pad_id(bank, x, y);
        self.voice_counts.get(&id).copied().unwrap_or(0) > 0
    }

    pub fn poll_audio_events(&mut self) {
        while let Ok(evt) = self.audio_rx.pop() {
            match evt {
                Event::VoiceStarted { pad } => {
                    *self.voice_counts.entry(pad).or_insert(0) += 1;
                }
                Event::VoiceEnded { pad } => {
                    if let Some(count) = self.voice_counts.get_mut(&pad) {
                        if *count > 1 {
                            *count -= 1;
                        } else {
                            self.voice_counts.remove(&pad);
                        }
                    }
                }
            }
        }
    }

    pub fn set_current_set(&mut self, set: Set, samples: HashMap<String, Arc<Sample>>) {
        Self::send_mixer_commands(&set, &mut self.audio_tx);
        self.set = set;
        self.samples = samples;
        self.held.clear();
        self.voice_counts.clear();
    }

    fn send_mixer_commands(set: &Set, tx: &mut Producer<Command>) {
        for (i, g) in set.mixer.groups.iter().enumerate() {
            let _ = tx.push(Command::SetGroup {
                group: i as u8,
                params: kit::group_params(g),
            });
        }
        let _ = tx.push(Command::SetMasterGain(db_to_gain(set.mixer.master_db)));
    }

    pub fn select_bank(&mut self, bank: usize) -> bool {
        if bank < self.set.banks.len() {
            self.bank = bank;
            true
        } else {
            false
        }
    }

    pub fn stop_all(&mut self) {
        let _ = self.audio_tx.push(Command::StopAll);
    }

    pub fn trigger_pad(&mut self, x: u8, y: u8) {
        let Some(pad) = self.set.banks[self.bank].pad(x, y) else {
            return;
        };
        let Some(sample) = self.samples.get(&pad.sample) else {
            return;
        };
        let id = Self::pad_id(self.bank, x, y);
        self.held.insert((x, y), id);
        let params = kit::play_params(pad, sample);
        let _ = self.audio_tx.push(Command::Trigger {
            pad: id,
            sample: sample.clone(),
            params,
        });
    }

    pub fn release_pad(&mut self, x: u8, y: u8) {
        if let Some(id) = self.held.remove(&(x, y)) {
            let _ = self.audio_tx.push(Command::Release { pad: id });
        }
    }

    pub fn on_lp_event(&mut self, ev: LpEvent) {
        match ev {
            LpEvent::PadDown { x, y } => self.trigger_pad(x, y),
            LpEvent::PadUp { x, y } => self.release_pad(x, y),
            LpEvent::SideDown(i) => {
                self.select_bank(i as usize);
            }
            LpEvent::TopDown(TOP_MIXER) => {
                self.stop_all();
            }
            _ => {}
        }
    }
}
