//! Router: single owner of set state, bank selection, and high-priority MIDI-to-audio dispatch.

use crate::lights::PaintSnapshot;
use lpd_core::audio::{db_to_gain, fader_gain, Command, Event, Sample};
use lpd_core::devices::launchpad::LpEvent;
use lpd_core::kit::{self, Set};
use rtrb::{Consumer, Producer};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const TOP_MIXER: u8 = 7;

/// A refused set replacement: nothing changed and the inputs are handed back.
#[derive(Debug)]
pub struct Rejected {
    pub reason: String,
    pub set: Set,
    pub samples: HashMap<String, Arc<Sample>>,
}

pub struct Router {
    set: Set,
    samples: HashMap<String, Arc<Sample>>,
    /// Samples no longer reachable from the caches. The audio thread may still hold the last
    /// strong reference, so they are freed here, off the audio thread, once nothing else does.
    retired: Vec<Arc<Sample>>,
    bank: usize,
    /// Held pads by pad id (bank * 64 + y * 8 + x).
    held: HashSet<u16>,
    voice_counts: HashMap<u16, usize>,
    audio_tx: Producer<Command>,
    audio_rx: Consumer<Event>,
    /// xorshift32 state for humanize; never 0.
    rng: u32,
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
            retired: Vec::new(),
            bank: 0,
            held: HashSet::new(),
            voice_counts: HashMap::new(),
            audio_tx,
            audio_rx,
            rng: time_seed(),
        }
    }

    /// Fix the humanize random sequence (tests).
    pub fn set_seed(&mut self, seed: u32) {
        self.rng = seed.max(1);
    }

    /// Uniform in [-1, 1).
    fn next_unit(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 23) as f32 - 1.0
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

    /// Put a decoded sample into the active cache; the entry it replaces is retired.
    pub fn install_sample(&mut self, path: String, sample: Arc<Sample>) {
        if let Some(old) = self.samples.insert(path, sample) {
            self.retire([old]);
        }
    }

    /// Keep `samples` alive until the audio side lets go of them (see `drain_reclaimable`).
    pub fn retire(&mut self, samples: impl IntoIterator<Item = Arc<Sample>>) {
        for s in samples {
            if !self.retired.iter().any(|r| Arc::ptr_eq(r, &s)) {
                self.retired.push(s);
            }
        }
    }

    /// Retired samples nothing else references. The caller drops them after releasing the router
    /// lock; the strong count can only fall (the audio thread never clones), so 1 is final.
    pub fn drain_reclaimable(&mut self) -> Vec<Arc<Sample>> {
        let (free, keep) = std::mem::take(&mut self.retired)
            .into_iter()
            .partition(|s| Arc::strong_count(s) == 1);
        self.retired = keep;
        free
    }

    pub fn retired_count(&self) -> usize {
        self.retired.len()
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
            .push(Command::SetMasterGain(fader_gain(master_db)));
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

    /// Swap in another set. Nothing changes unless the audio queue has room for the whole
    /// transition, so the stop below cannot be lost. Voices of the old set fade out (StopAll is
    /// queued ahead of any trigger for the new set); their samples are retired, never freed here.
    /// Voice counters stay as they are so in-flight start/end events remain balanced.
    pub fn replace_set(
        &mut self,
        set: Set,
        samples: HashMap<String, Arc<Sample>>,
    ) -> Result<(), Box<Rejected>> {
        let needed = 1 + set.mixer.groups.len() + 1;
        if self.audio_tx.slots() < needed {
            return Err(Box::new(Rejected {
                reason: "The audio engine is busy. Try again in a moment.".into(),
                set,
                samples,
            }));
        }
        let _ = self.audio_tx.push(Command::StopAll);
        Self::send_mixer_commands(&set, &mut self.audio_tx);
        self.held.clear();
        self.set = set;
        let old = std::mem::replace(&mut self.samples, samples);
        self.retire(old.into_values());
        Ok(())
    }

    /// Release every held pad (input ownership is changing).
    pub fn release_all_held(&mut self) {
        let ids: Vec<u16> = self.held.iter().copied().collect();
        for id in ids {
            self.release_id(id);
        }
    }

    /// Everything the light painter needs for the active bank, copied under one lock.
    pub fn paint_snapshot(&self) -> Option<PaintSnapshot> {
        let bank = self.set.banks.get(self.bank)?.clone();
        let loaded = bank
            .pads
            .iter()
            .filter(|p| self.samples.contains_key(&p.sample))
            .map(|p| p.sample.clone())
            .collect();
        let mut sounding = [false; 64];
        for y in 0..8u8 {
            for x in 0..8u8 {
                sounding[y as usize * 8 + x as usize] = self.is_pad_sounding(self.bank, x, y);
            }
        }
        Some(PaintSnapshot {
            bank_index: self.bank,
            bank,
            loaded,
            sounding,
        })
    }

    /// Replace the set but keep samples, holds and voice tracking (undo/redo). Undo/redo keep the
    /// current mixer values (`restore_keeping_mixer`), so nothing is sent to the engine.
    pub fn install_set(&mut self, set: Set) {
        self.set = set;
    }

    /// Push every group and the master gain to the engine.
    pub fn send_mixer(&mut self) {
        Self::send_mixer_commands(&self.set, &mut self.audio_tx);
    }

    fn send_mixer_commands(set: &Set, tx: &mut Producer<Command>) {
        for (i, g) in set.mixer.groups.iter().enumerate() {
            let _ = tx.push(Command::SetGroup {
                group: i as u8,
                params: kit::group_params(g),
            });
        }
        let _ = tx.push(Command::SetMasterGain(fader_gain(set.mixer.master_db)));
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

    /// Current bank, full velocity.
    pub fn trigger_pad(&mut self, x: u8, y: u8) {
        self.trigger_pad_in(self.bank, x, y, 1.0);
    }

    /// The one trigger path: pad gain × `velocity_gain` × bank humanize (±humanize_db/2).
    pub fn trigger_pad_in(&mut self, bank: usize, x: u8, y: u8, velocity_gain: f32) {
        let Some(b) = self.set.banks.get(bank) else {
            return;
        };
        let humanize_db = b.humanize_db;
        let Some(pad) = b.pad(x, y) else {
            return;
        };
        let Some(sample) = self.samples.get(&pad.sample) else {
            return;
        };
        let mut params = kit::play_params(pad, sample);
        let sample = sample.clone();
        let mut gain = velocity_gain;
        if humanize_db > 0.0 {
            gain *= db_to_gain(self.next_unit() * humanize_db / 2.0);
        }
        params.gain *= gain;
        let id = Self::pad_id(bank, x, y);
        let sent = self.audio_tx.push(Command::Trigger {
            pad: id,
            sample,
            params,
        });
        if sent.is_ok() {
            self.held.insert(id);
        }
    }

    /// Release (x, y) of the current bank; if that is not held (bank switched while
    /// holding), release the pad held at (x, y) in another bank.
    pub fn release_pad(&mut self, x: u8, y: u8) {
        let current = Self::pad_id(self.bank, x, y);
        let id = if self.held.contains(&current) {
            Some(current)
        } else {
            let pos = Self::pad_id(0, x, y);
            self.held.iter().copied().find(|id| id % 64 == pos)
        };
        if let Some(id) = id {
            self.release_id(id);
        }
    }

    pub fn release_pad_in(&mut self, bank: usize, x: u8, y: u8) {
        self.release_id(Self::pad_id(bank, x, y));
    }

    fn release_id(&mut self, id: u16) {
        if self.held.remove(&id) {
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

/// True when the sets differ in what the engine hears from the mixer (group names ignored).
pub fn mixer_audio_differs(a: &Set, b: &Set) -> bool {
    a.mixer.master_db != b.mixer.master_db
        || a.mixer.groups.len() != b.mixer.groups.len()
        || a.mixer
            .groups
            .iter()
            .zip(&b.mixer.groups)
            .any(|(x, y)| kit::group_params(x) != kit::group_params(y))
}

fn time_seed() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
        .unwrap_or(0x9E37_79B9);
    nanos.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpd_core::kit::Pad;
    use rtrb::RingBuffer;

    fn router_with_pad(humanize_db: f32, volume_db: f32) -> (Router, Consumer<Command>) {
        let mut set = Set::new();
        let mut pad = Pad::new(0, 0, "samples/a.wav", [63, 0, 0]);
        pad.volume_db = volume_db;
        set.banks[0].set_pad(pad.clone());
        set.banks[0].humanize_db = humanize_db;
        set.banks[1].set_pad(pad);
        let mut samples = HashMap::new();
        samples.insert(
            "samples/a.wav".to_string(),
            Arc::new(Sample {
                rate: 48_000,
                data: vec![0.5; 200],
            }),
        );
        let (tx, mut cmd_rx) = RingBuffer::new(1024);
        let (_evt_tx, evt_rx) = RingBuffer::new(16);
        let router = Router::new(set, samples, tx, evt_rx);
        while cmd_rx.pop().is_ok() {}
        (router, cmd_rx)
    }

    fn trigger_gains(rx: &mut Consumer<Command>) -> Vec<f32> {
        let mut gains = Vec::new();
        while let Ok(cmd) = rx.pop() {
            if let Command::Trigger { params, .. } = cmd {
                gains.push(params.gain);
            }
        }
        gains
    }

    #[test]
    fn humanize_varies_gain_within_half_range() {
        let (mut router, mut rx) = router_with_pad(6.0, 0.0);
        router.set_seed(12345);
        for _ in 0..10 {
            router.trigger_pad(0, 0);
        }
        let gains = trigger_gains(&mut rx);
        assert_eq!(gains.len(), 10);
        let (lo, hi) = (db_to_gain(-3.0), db_to_gain(3.0));
        assert!(gains.iter().all(|g| *g >= lo && *g <= hi), "{gains:?}");
        assert!(gains.iter().any(|g| *g != gains[0]), "{gains:?}");
    }

    #[test]
    fn no_humanize_gives_exact_pad_gain() {
        let (mut router, mut rx) = router_with_pad(0.0, -6.0);
        router.set_seed(7);
        for _ in 0..5 {
            router.trigger_pad(0, 0);
        }
        let gains = trigger_gains(&mut rx);
        assert_eq!(gains.len(), 5);
        assert!(gains.iter().all(|g| *g == db_to_gain(-6.0)), "{gains:?}");
    }

    #[test]
    fn velocity_gain_scales_trigger() {
        let (mut router, mut rx) = router_with_pad(0.0, 0.0);
        router.trigger_pad_in(1, 0, 0, 0.5);
        assert_eq!(trigger_gains(&mut rx), vec![0.5]);
    }

    #[test]
    fn release_from_another_bank_uses_pad_id() {
        let (mut router, mut rx) = router_with_pad(0.0, 0.0);
        router.trigger_pad_in(1, 0, 0, 1.0);
        router.release_pad_in(1, 0, 0);
        let mut released = Vec::new();
        while let Ok(cmd) = rx.pop() {
            if let Command::Release { pad } = cmd {
                released.push(pad);
            }
        }
        assert_eq!(released, vec![Router::pad_id(1, 0, 0)]);
    }

    #[test]
    fn a_trigger_lost_to_a_full_queue_is_not_held() {
        let mut set = Set::new();
        set.banks[0].set_pad(Pad::new(0, 0, "samples/a.wav", [63, 0, 0]));
        let mut samples = HashMap::new();
        samples.insert(
            "samples/a.wav".to_string(),
            Arc::new(Sample {
                rate: 48_000,
                data: vec![0.5; 200],
            }),
        );
        // Smaller than the mixer burst `new` sends: the queue is full afterwards.
        let (tx, mut cmd_rx) = RingBuffer::new(2);
        let (_evt_tx, evt_rx) = RingBuffer::new(16);
        let mut router = Router::new(set, samples, tx, evt_rx);
        router.trigger_pad(0, 0);
        assert!(!router.held.contains(&Router::pad_id(0, 0, 0)));

        while cmd_rx.pop().is_ok() {}
        router.release_pad(0, 0);
        assert!(
            cmd_rx.pop().is_err(),
            "no release for a pad that never sounded"
        );
    }

    #[test]
    fn release_after_bank_switch_still_releases_pressed_pad() {
        let (mut router, mut rx) = router_with_pad(0.0, 0.0);
        router.trigger_pad(0, 0);
        router.select_bank(1);
        router.release_pad(0, 0);
        let mut released = Vec::new();
        while let Ok(cmd) = rx.pop() {
            if let Command::Release { pad } = cmd {
                released.push(pad);
            }
        }
        assert_eq!(released, vec![Router::pad_id(0, 0, 0)]);
    }
}
