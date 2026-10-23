//! 60 Hz Light Painter Engine.
//! Calculates live 81-LED frames based on:
//! Base pad colours, Play-state (35% idle vs 100% sounding), Flash (150 ms decay to white),
//! Ripple (expanding concentric rings), Missing sample red blink (500 ms cycle), and active bank side button.

use lpd_core::devices::launchpad as lp;
use lpd_core::kit::{Bank, Set};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

const FLASH_DURATION: Duration = Duration::from_millis(150);
const RIPPLE_STEP: Duration = Duration::from_millis(40);
const BLINK_PERIOD_MS: u128 = 500;
const BLINK_HALF_PERIOD_MS: u128 = 250;
const WHITE: [u8; 3] = [63, 63, 63];
const OFF: [u8; 3] = [0, 0, 0];
const MISSING_RED: [u8; 3] = [63, 0, 0];
const FULL_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug)]
struct PadHit {
    x: u8,
    y: u8,
    time: Instant,
}

/// The active bank, which of its samples are loaded and which pads sound, copied under one
/// router lock so painting never touches the router.
#[derive(Clone, Debug)]
pub struct PaintSnapshot {
    pub bank_index: usize,
    pub bank: Bank,
    pub loaded: HashSet<String>,
    /// Index `y * 8 + x` of the active bank.
    pub sounding: [bool; 64],
}

/// LEDs that differ from what the device was last told, tagged with the cache generation
/// they were computed against.
#[derive(Debug)]
pub struct Diff {
    pub leds: Vec<(u8, [u8; 3])>,
    generation: u64,
}

/// Lock order: the painter is a leaf. Never hold it while taking another lock, and never take
/// the router from inside a painter call.
pub struct LightPainter {
    hits: Vec<PadHit>,
    start_time: Instant,
    last_frame: HashMap<u8, [u8; 3]>,
    /// Bumped by `invalidate`; a `Diff` from an older generation must not update the cache.
    generation: u64,
}

impl Default for LightPainter {
    fn default() -> Self {
        Self::new()
    }
}

impl LightPainter {
    pub fn new() -> Self {
        Self {
            hits: Vec::new(),
            start_time: Instant::now(),
            last_frame: HashMap::new(),
            generation: 0,
        }
    }

    pub fn on_pad_hit(&mut self, x: u8, y: u8, time: Instant) {
        self.hits
            .retain(|h| time.saturating_duration_since(h.time) < Duration::from_millis(600));
        self.hits.push(PadHit { x, y, time });
    }

    /// Compute the full frame for `snap` (the active bank).
    pub fn paint_snapshot(&mut self, snap: &PaintSnapshot, now: Instant) -> Vec<(u8, [u8; 3])> {
        self.paint_bank(
            &snap.bank,
            snap.bank_index,
            &snap.loaded,
            &|x, y| snap.sounding[y as usize * 8 + x as usize],
            now,
        )
    }

    /// Compute full frame of (led_id, [r, g, b]) for 64 grid pads and 8 side buttons.
    pub fn paint<F>(
        &mut self,
        set: &Set,
        active_bank: usize,
        loaded_samples: &HashSet<String>,
        is_sounding: &F,
        now: Instant,
    ) -> Vec<(u8, [u8; 3])>
    where
        F: Fn(usize, u8, u8) -> bool,
    {
        let Some(bank) = set.banks.get(active_bank) else {
            return vec![];
        };
        self.paint_bank(
            bank,
            active_bank,
            loaded_samples,
            &|x, y| is_sounding(active_bank, x, y),
            now,
        )
    }

    fn paint_bank(
        &mut self,
        bank: &Bank,
        active_bank: usize,
        loaded_samples: &HashSet<String>,
        is_sounding: &dyn Fn(u8, u8) -> bool,
        now: Instant,
    ) -> Vec<(u8, [u8; 3])> {
        let modes = bank.lights;
        let elapsed_ms = now.saturating_duration_since(self.start_time).as_millis();
        let blink_on = (elapsed_ms % BLINK_PERIOD_MS) < BLINK_HALF_PERIOD_MS;

        let mut frame = Vec::with_capacity(72);

        for y in 0..8u8 {
            for x in 0..8u8 {
                let led_id = lp::grid_led(x, y);
                let color = match bank.pad(x, y) {
                    None => OFF,
                    Some(pad) => {
                        if !loaded_samples.contains(&pad.sample) {
                            if blink_on {
                                MISSING_RED
                            } else {
                                OFF
                            }
                        } else {
                            let sounding = is_sounding(x, y);
                            let mut base = pad.color;
                            if modes.play_state && !sounding {
                                base = [
                                    (base[0] as f32 * 0.35).round() as u8,
                                    (base[1] as f32 * 0.35).round() as u8,
                                    (base[2] as f32 * 0.35).round() as u8,
                                ];
                            }
                            if modes.ripple {
                                for hit in &self.hits {
                                    let dt = now.saturating_duration_since(hit.time);
                                    let dist = ((x as i16 - hit.x as i16).abs())
                                        .max((y as i16 - hit.y as i16).abs())
                                        as u32;
                                    let ring_time = dist * RIPPLE_STEP.as_millis() as u32;
                                    let dt_ms = dt.as_millis() as u32;
                                    if dt_ms >= ring_time && dt_ms < ring_time + 40 && dist <= 4 {
                                        let boost = 20u8;
                                        base = [
                                            base[0].saturating_add(boost).min(63),
                                            base[1].saturating_add(boost).min(63),
                                            base[2].saturating_add(boost).min(63),
                                        ];
                                    }
                                }
                            }
                            if modes.flash {
                                if let Some(last_hit) =
                                    self.hits.iter().filter(|h| h.x == x && h.y == y).last()
                                {
                                    let dt = now.saturating_duration_since(last_hit.time);
                                    if dt < FLASH_DURATION {
                                        let frac = dt.as_secs_f32() / FLASH_DURATION.as_secs_f32();
                                        base = [
                                            (WHITE[0] as f32
                                                + (base[0] as f32 - WHITE[0] as f32) * frac)
                                                .round()
                                                .clamp(0.0, 63.0)
                                                as u8,
                                            (WHITE[1] as f32
                                                + (base[1] as f32 - WHITE[1] as f32) * frac)
                                                .round()
                                                .clamp(0.0, 63.0)
                                                as u8,
                                            (WHITE[2] as f32
                                                + (base[2] as f32 - WHITE[2] as f32) * frac)
                                                .round()
                                                .clamp(0.0, 63.0)
                                                as u8,
                                        ];
                                    }
                                }
                            }
                            base
                        }
                    }
                };
                frame.push((led_id, color));
            }
        }

        for i in 0..8u8 {
            let led_id = lp::side_led(i);
            let color = if (i as usize) == active_bank {
                WHITE
            } else {
                OFF
            };
            frame.push((led_id, color));
        }

        frame
    }

    /// LEDs that changed since the last successful MIDI send. Does not update the cache;
    /// call `mark_sent` once the output was sent.
    pub fn diff(&self, full_frame: &[(u8, [u8; 3])]) -> Diff {
        let leds = full_frame
            .iter()
            .copied()
            .filter(|(id, rgb)| self.last_frame.get(id) != Some(rgb))
            .collect();
        Diff {
            leds,
            generation: self.generation,
        }
    }

    /// Record a delivered diff. Ignored when the cache was invalidated after the diff was taken.
    pub fn mark_sent(&mut self, diff: &Diff) {
        if diff.generation == self.generation {
            self.last_frame.extend(diff.leds.iter().copied());
        }
    }

    /// Forget what the device shows, so the next frame is sent in full. Call on connection loss,
    /// reconnect, failed sends and whenever another owner has drawn on the device.
    pub fn invalidate(&mut self) {
        self.last_frame.clear();
        self.generation += 1;
    }
}

/// Where Launchpad output goes. Implemented for the MIDI port; tests use a fake.
pub trait LedSink {
    fn send(&mut self, message: &[u8]) -> Result<(), String>;
}

/// Output half of the Launchpad thread: owns the connected sink and keeps the painter's cache
/// in step with what the device really shows. The painter lock is only taken briefly, never
/// across a send (a stalled USB port must not block pad input).
pub struct LedOutput<S: LedSink> {
    sink: Option<S>,
    last_full_refresh: Option<Instant>,
    /// Latest game frame (full SysEx); resent after a reconnect while a game owns the pads.
    game_frame: Option<Vec<u8>>,
}

impl<S: LedSink> Default for LedOutput<S> {
    fn default() -> Self {
        Self {
            sink: None,
            last_full_refresh: None,
            game_frame: None,
        }
    }
}

fn leaf(painter: &Mutex<LightPainter>) -> MutexGuard<'_, LightPainter> {
    painter.lock().unwrap_or_else(PoisonError::into_inner)
}

impl<S: LedSink> LedOutput<S> {
    pub fn is_connected(&self) -> bool {
        self.sink.is_some()
    }

    /// Initialise a freshly connected device. Its LEDs are all off afterwards, so the painter is
    /// invalidated and the current owner repaints in full on its next frame.
    pub fn connect(&mut self, mut sink: S, painter: &Mutex<LightPainter>) -> Result<(), String> {
        leaf(painter).invalidate();
        sink.send(&lp::SELECT_SESSION_LAYOUT)?;
        sink.send(&lp::set_all(0))?;
        self.sink = Some(sink);
        self.last_full_refresh = None;
        Ok(())
    }

    pub fn disconnect(&mut self, painter: &Mutex<LightPainter>) {
        self.sink = None;
        self.last_full_refresh = None;
        leaf(painter).invalidate();
    }

    /// Send sampler changes, with a full repaint each second to repair silently lost updates.
    /// A send error disconnects and invalidates. Without a device nothing is sent or remembered.
    pub fn send_sampler(
        &mut self,
        painter: &Mutex<LightPainter>,
        frame: &[(u8, [u8; 3])],
        now: Instant,
    ) -> Result<(), String> {
        if self.sink.is_none() {
            return Ok(());
        }
        let full_refresh = self
            .last_full_refresh
            .is_none_or(|last| now.saturating_duration_since(last) >= FULL_REFRESH_INTERVAL);
        // MIDI success is not a device acknowledgement; a lost update must not stay cached forever.
        if full_refresh {
            leaf(painter).invalidate();
        }
        let diff = leaf(painter).diff(frame);
        let mut failure = None;
        if let Some(sink) = self.sink.as_mut() {
            for message in lp::rgb_messages(&diff.leds) {
                if let Err(error) = sink.send(&message) {
                    failure = Some(error);
                    break;
                }
            }
        }
        match failure {
            Some(error) => {
                self.disconnect(painter);
                Err(error)
            }
            None => {
                leaf(painter).mark_sent(&diff);
                if full_refresh {
                    self.last_full_refresh = Some(now);
                }
                Ok(())
            }
        }
    }

    /// Show a game frame (a complete RGB SysEx message). The painter cache no longer describes
    /// the device afterwards, so it is invalidated.
    pub fn send_game(
        &mut self,
        painter: &Mutex<LightPainter>,
        message: Vec<u8>,
    ) -> Result<(), String> {
        leaf(painter).invalidate();
        let result = match self.sink.as_mut() {
            Some(sink) => sink.send(&message),
            None => Ok(()),
        };
        self.game_frame = Some(message);
        if result.is_err() {
            self.disconnect(painter);
        }
        result
    }

    /// Repeat the last game frame (after a reconnect while a game owns the pads).
    pub fn resend_game(&mut self, painter: &Mutex<LightPainter>) -> Result<(), String> {
        match self.game_frame.clone() {
            Some(message) => self.send_game(painter, message),
            None => Ok(()),
        }
    }

    /// A game released the pads: forget its frame.
    pub fn clear_game(&mut self) {
        self.game_frame = None;
    }
}
