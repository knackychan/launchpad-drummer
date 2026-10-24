//! ADSR envelope at control rate (spec §3.7). The attack is convex (it aims at 1.3 and stops at
//! 1.0, so it takes exactly ATTACK); decay and release are exponential and reach 1 % of the way
//! to their target in DECAY or RELEASE.

use super::params::EnvParams;

/// Samples per control tick (spec 3.1).
pub const TICK: usize = 16;
const ATTACK_TARGET: f32 = 1.3;
const MIN_TIME: f32 = 0.0005;
const FAST_RELEASE: f32 = 0.005;
/// Below this an envelope is over (−80 dB).
const END: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Stage {
    #[default]
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Env {
    pub stage: Stage,
    pub level: f32,
    fast: bool,
}

impl Env {
    /// Note-on: the attack starts from the current level, so a retriggered note never clicks.
    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
        self.fast = false;
    }

    pub fn gate_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    /// All notes off: a 5 ms release.
    pub fn kill(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
            self.fast = true;
        }
    }

    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    /// 0 idle, 1 attack, 2 decay, 3 sustain, 4 release (the monitor's numbering).
    pub fn stage_index(&self) -> u8 {
        self.stage as u8
    }

    /// Advance by `dt` seconds and return the level.
    pub fn tick(&mut self, p: &EnvParams, dt: f32) -> f32 {
        match self.stage {
            Stage::Idle => self.level = 0.0,
            Stage::Attack => {
                let tau = p.attack.max(MIN_TIME) / (ATTACK_TARGET / (ATTACK_TARGET - 1.0)).ln();
                self.level = ATTACK_TARGET - (ATTACK_TARGET - self.level) * (-dt / tau).exp();
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                let s = p.sustain.clamp(0.0, 1.0);
                self.level = if p.decay <= 0.0 {
                    s
                } else {
                    s + (self.level - s) * (0.01f32.ln() * dt / p.decay).exp()
                };
                if (self.level - s).abs() < END {
                    self.level = s;
                    self.stage = if s <= END {
                        Stage::Idle
                    } else {
                        Stage::Sustain
                    };
                }
            }
            Stage::Sustain => self.level = p.sustain.clamp(0.0, 1.0),
            Stage::Release => {
                let time = if self.fast {
                    FAST_RELEASE
                } else {
                    p.release.max(MIN_TIME)
                };
                self.level *= (0.01f32.ln() * dt / time).exp();
                if self.level < END {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                    self.fast = false;
                }
            }
        }
        self.level
    }
}
