//! Launchpad events → engine commands and LED frames. No I/O here.

use lpd_core::audio::{db_to_gain, Command, Sample};
use lpd_core::devices::launchpad::{self as lp, LpEvent};
use lpd_core::kit::{self, Set};
use std::collections::HashMap;
use std::sync::Arc;

pub enum Action {
    Audio(Command),
    Leds(Vec<(u8, [u8; 3])>),
}

const BANK_LED_ON: [u8; 3] = [63, 63, 63];
const MISSING: [u8; 3] = [63, 0, 0];
const OFF: [u8; 3] = [0, 0, 0];
/// Top-row index of the Mixer button: stop all sounds.
const TOP_MIXER: u8 = 7;

pub struct Player {
    set: Set,
    samples: HashMap<String, Arc<Sample>>,
    bank: usize,
    /// Pads currently held → the pad id their press triggered.
    held: HashMap<(u8, u8), u16>,
}

impl Player {
    pub fn new(set: Set, samples: HashMap<String, Arc<Sample>>) -> Player {
        Player {
            set,
            samples,
            bank: 0,
            held: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn bank(&self) -> usize {
        self.bank
    }

    /// Pad id unique across banks, so voices survive bank switches.
    fn pad_id(bank: usize, x: u8, y: u8) -> u16 {
        (bank * 64 + y as usize * 8 + x as usize) as u16
    }

    /// Mixer setup from the set; send once before playing.
    pub fn mixer_commands(&self) -> Vec<Command> {
        let mut cmds: Vec<Command> = self
            .set
            .mixer
            .groups
            .iter()
            .enumerate()
            .map(|(i, g)| Command::SetGroup {
                group: i as u8,
                params: kit::group_params(g),
            })
            .collect();
        cmds.push(Command::SetMasterGain(db_to_gain(self.set.mixer.master_db)));
        cmds
    }

    /// Full LED frame: pad colours (missing samples red, empty off) and the active bank button.
    pub fn paint(&self) -> Vec<(u8, [u8; 3])> {
        let bank = &self.set.banks[self.bank];
        let mut leds = Vec::with_capacity(72);
        for y in 0..8 {
            for x in 0..8 {
                let colour = match bank.pad(x, y) {
                    Some(p) if self.samples.contains_key(&p.sample) => p.color,
                    Some(_) => MISSING,
                    None => OFF,
                };
                leds.push((lp::grid_led(x, y), colour));
            }
        }
        for i in 0..8u8 {
            leds.push((
                lp::side_led(i),
                if i as usize == self.bank {
                    BANK_LED_ON
                } else {
                    OFF
                },
            ));
        }
        leds
    }

    pub fn on_event(&mut self, ev: LpEvent) -> Vec<Action> {
        match ev {
            LpEvent::PadDown { x, y } => {
                let Some(pad) = self.set.banks[self.bank].pad(x, y) else {
                    return vec![];
                };
                let Some(sample) = self.samples.get(&pad.sample) else {
                    return vec![];
                };
                let id = Self::pad_id(self.bank, x, y);
                self.held.insert((x, y), id);
                let params = kit::play_params(pad, sample);
                vec![Action::Audio(Command::Trigger {
                    pad: id,
                    sample: sample.clone(),
                    params,
                })]
            }
            LpEvent::PadUp { x, y } => match self.held.remove(&(x, y)) {
                Some(id) => vec![Action::Audio(Command::Release { pad: id })],
                None => vec![],
            },
            LpEvent::SideDown(i) if (i as usize) < self.set.banks.len() => {
                self.bank = i as usize;
                vec![Action::Leds(self.paint())]
            }
            LpEvent::TopDown(TOP_MIXER) => vec![Action::Audio(Command::StopAll)],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpd_core::audio::{Command, Sample};
    use lpd_core::devices::launchpad::{self as lp, LpEvent};
    use lpd_core::kit::{Pad, Set};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn player() -> Player {
        let mut set = Set::new();
        set.banks[0].set_pad(Pad::new(0, 0, "samples/a.wav", [63, 0, 0]));
        set.banks[0].set_pad(Pad::new(1, 0, "samples/missing.wav", [0, 63, 0]));
        let mut hold = Pad::new(2, 0, "samples/a.wav", [0, 0, 63]);
        hold.mode = lpd_core::kit::PadMode::Hold;
        set.banks[0].set_pad(hold);
        set.banks[1].set_pad(Pad::new(0, 0, "samples/b.wav", [0, 0, 63]));
        let mut samples = HashMap::new();
        for name in ["samples/a.wav", "samples/b.wav"] {
            samples.insert(
                name.to_string(),
                Arc::new(Sample {
                    rate: 48_000,
                    data: vec![0.0; 200],
                }),
            );
        }
        Player::new(set, samples)
    }

    fn triggered_pad(actions: &[Action]) -> Option<u16> {
        actions.iter().find_map(|a| match a {
            Action::Audio(Command::Trigger { pad, .. }) => Some(*pad),
            _ => None,
        })
    }

    #[test]
    fn pad_down_triggers_its_sample() {
        let mut p = player();
        assert_eq!(
            triggered_pad(&p.on_event(LpEvent::PadDown { x: 0, y: 0 })),
            Some(0)
        );
    }

    #[test]
    fn empty_or_missing_pads_do_nothing() {
        let mut p = player();
        assert!(p.on_event(LpEvent::PadDown { x: 5, y: 5 }).is_empty());
        assert!(p.on_event(LpEvent::PadDown { x: 1, y: 0 }).is_empty());
    }

    #[test]
    fn side_button_switches_bank_and_repaints() {
        let mut p = player();
        let actions = p.on_event(LpEvent::SideDown(1));
        assert_eq!(p.bank(), 1);
        let Some(Action::Leds(leds)) = actions.first() else {
            panic!("expected LEDs")
        };
        assert!(leds.contains(&(lp::side_led(1), [63, 63, 63])));
        assert!(leds.contains(&(lp::side_led(0), [0, 0, 0])));
        assert_eq!(
            triggered_pad(&p.on_event(LpEvent::PadDown { x: 0, y: 0 })),
            Some(64)
        );
    }

    #[test]
    fn release_after_bank_switch_targets_the_original_voice() {
        let mut p = player();
        p.on_event(LpEvent::PadDown { x: 2, y: 0 });
        p.on_event(LpEvent::SideDown(1));
        let actions = p.on_event(LpEvent::PadUp { x: 2, y: 0 });
        assert!(matches!(
            actions.as_slice(),
            [Action::Audio(Command::Release { pad: 2 })]
        ));
    }

    #[test]
    fn mixer_button_stops_all() {
        let mut p = player();
        assert!(matches!(
            p.on_event(LpEvent::TopDown(7)).as_slice(),
            [Action::Audio(Command::StopAll)]
        ));
    }

    #[test]
    fn paint_shows_colours_missing_red_empty_off_and_bank() {
        let p = player();
        let leds = p.paint();
        assert_eq!(leds.len(), 72);
        assert!(leds.contains(&(lp::grid_led(0, 0), [63, 0, 0])));
        assert!(leds.contains(&(lp::grid_led(1, 0), [63, 0, 0])));
        assert!(leds.contains(&(lp::grid_led(5, 5), [0, 0, 0])));
        assert!(leds.contains(&(lp::side_led(0), [63, 63, 63])));
    }

    #[test]
    fn mixer_commands_configure_all_groups_and_master() {
        let p = player();
        let cmds = p.mixer_commands();
        assert_eq!(
            cmds.iter()
                .filter(|c| matches!(c, Command::SetGroup { .. }))
                .count(),
            8
        );
        assert!(matches!(cmds.last(), Some(Command::SetMasterGain(_))));
    }
}
