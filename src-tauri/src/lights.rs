//! 60 Hz Light Painter Engine.
//! Calculates live 81-LED frames based on:
//! Base pad colours, Play-state (35% idle vs 100% sounding), Flash (150 ms decay to white),
//! Ripple (expanding concentric rings), Missing sample red blink (500 ms cycle), and active bank side button.

use lpd_core::devices::launchpad as lp;
use lpd_core::kit::Set;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

const FLASH_DURATION: Duration = Duration::from_millis(150);
const RIPPLE_STEP: Duration = Duration::from_millis(40);
const BLINK_PERIOD_MS: u128 = 500;
const BLINK_HALF_PERIOD_MS: u128 = 250;
const WHITE: [u8; 3] = [63, 63, 63];
const OFF: [u8; 3] = [0, 0, 0];
const MISSING_RED: [u8; 3] = [63, 0, 0];

#[derive(Clone, Debug)]
struct PadHit {
    x: u8,
    y: u8,
    time: Instant,
}

pub struct LightPainter {
    hits: Vec<PadHit>,
    start_time: Instant,
    last_frame: HashMap<u8, [u8; 3]>,
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
        }
    }

    pub fn on_pad_hit(&mut self, x: u8, y: u8, time: Instant) {
        self.hits
            .retain(|h| time.saturating_duration_since(h.time) < Duration::from_millis(600));
        self.hits.push(PadHit { x, y, time });
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
        let bank = match set.banks.get(active_bank) {
            Some(b) => b,
            None => return vec![],
        };
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
                            let sounding = is_sounding(active_bank, x, y);
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

    /// Compute diff containing only LEDs that changed since previous call.
    pub fn diff(&mut self, full_frame: &[(u8, [u8; 3])]) -> Vec<(u8, [u8; 3])> {
        let mut changes = Vec::new();
        for &(id, rgb) in full_frame {
            let prev = self.last_frame.get(&id).copied();
            if prev != Some(rgb) {
                self.last_frame.insert(id, rgb);
                changes.push((id, rgb));
            }
        }
        changes
    }
}
