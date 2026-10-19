//! Step sequencer data and timing (plan 1C, Task 13). Pure: the router thread runs the clock.

use serde::{Deserialize, Serialize};

pub const MAX_STEPS: usize = 32;
pub const VEL_ON: u8 = 90;
pub const VEL_ACCENT: u8 = 127;
pub const LENGTHS: [u8; 4] = [4, 8, 16, 32];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Pattern {
    pub lanes: Vec<Lane>,
    pub length: u8,
    pub bpm: f32,
    pub swing: f32,
}

impl Default for Pattern {
    fn default() -> Self {
        Pattern {
            lanes: vec![],
            length: 16,
            bpm: 100.0,
            swing: 50.0,
        }
    }
}

/// One pad's row. `steps` always holds `MAX_STEPS` values: 0 off, 1 on, 2 accent.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Lane {
    pub bank: u8,
    pub x: u8,
    pub y: u8,
    pub mute: bool,
    pub steps: Vec<u8>,
}

/// Length of one 16th note in seconds.
pub fn step_duration(bpm: f32) -> f64 {
    60.0 / f64::from(bpm) / 4.0
}

/// Start of step `i` from the loop start: `i × d`, plus `(swing − 50) / 50 × d` for odd steps
/// (swing 75 puts the odd steps half a step late).
pub fn step_time(i: usize, bpm: f32, swing: f32) -> f64 {
    let d = step_duration(bpm);
    let late = if i % 2 == 1 {
        f64::from(swing - 50.0) / 50.0 * d
    } else {
        0.0
    };
    i as f64 * d + late
}

pub fn loop_duration(length: usize, bpm: f32) -> f64 {
    length as f64 * step_duration(bpm)
}

/// Record rule (UI spec 4.7). `i` is the last step whose time is ≤ `t` (`t` in `[0, loop)`). When
/// `t` is more than half a step past it, the hit belongs to the next step: `(i + 1, true)`,
/// wrapping to `(0, true)` after the last step; `true` means "already played live", so the clock
/// must not fire that step on this pass. Otherwise `(i, false)`: step `i` already fired this
/// pass, so nothing is doubled.
pub fn nearest_step(t: f64, length: usize, bpm: f32, swing: f32) -> (usize, bool) {
    if length == 0 {
        return (0, false);
    }
    let t = t.rem_euclid(loop_duration(length, bpm));
    let i = (0..length)
        .rev()
        .find(|&s| step_time(s, bpm, swing) <= t)
        .unwrap_or(0);
    if t - step_time(i, bpm, swing) > step_duration(bpm) / 2.0 {
        ((i + 1) % length, true)
    } else {
        (i, false)
    }
}

/// The steps whose time falls in `(prev, now]` (seconds since the loop start; `now` may pass the
/// loop end), in order, wrapping. Only steps inside `length` ever fire.
pub fn due_steps(prev: f64, now: f64, length: usize, bpm: f32, swing: f32) -> Vec<usize> {
    let mut due = Vec::new();
    if length == 0 || now <= prev {
        return due;
    }
    let l = loop_duration(length, bpm);
    let first = (prev / l).floor().max(0.0) as u64;
    let last = (now / l).floor().max(0.0) as u64;
    for pass in first..=last {
        let base = pass as f64 * l;
        for i in 0..length {
            let t = base + step_time(i, bpm, swing);
            if t > prev && t <= now {
                due.push(i);
            }
        }
    }
    due
}

/// Growing: in every lane whose steps from the old length on are all 0, the new steps repeat the
/// old length; other lanes keep their steps. Shrinking only changes `length` (hidden steps stay).
pub fn set_length(p: &mut Pattern, new_len: u8) {
    let old = usize::from(p.length);
    let new = usize::from(new_len).clamp(1, MAX_STEPS);
    if new > old && old > 0 {
        for lane in &mut p.lanes {
            lane.steps.resize(MAX_STEPS, 0);
            if lane.steps[old..].iter().all(|&v| v == 0) {
                for j in old..new {
                    lane.steps[j] = lane.steps[j % old];
                }
            }
        }
    }
    p.length = new as u8;
}

pub fn add_lane(p: &mut Pattern, bank: u8, x: u8, y: u8) -> Result<(), &'static str> {
    if p.lanes.iter().any(|l| (l.bank, l.x, l.y) == (bank, x, y)) {
        return Err("Lane already there");
    }
    p.lanes.push(Lane {
        bank,
        x,
        y,
        mute: false,
        steps: vec![0; MAX_STEPS],
    });
    Ok(())
}

pub fn clear_steps(p: &mut Pattern) {
    for lane in &mut p.lanes {
        lane.steps.iter_mut().for_each(|v| *v = 0);
    }
}

/// Check a pattern that came from the UI and clamp tempo and swing into their ranges.
pub fn validate(p: &mut Pattern) -> Result<(), String> {
    if !LENGTHS.contains(&p.length) {
        return Err(format!("Invalid pattern length {}", p.length));
    }
    if !p.bpm.is_finite() || !p.swing.is_finite() {
        return Err("Invalid tempo or swing".into());
    }
    p.bpm = p.bpm.clamp(60.0, 200.0);
    p.swing = p.swing.clamp(50.0, 75.0);
    for (i, lane) in p.lanes.iter().enumerate() {
        if lane.bank >= 8 || lane.x >= 8 || lane.y >= 8 {
            return Err(format!(
                "Lane {} points at a pad that does not exist",
                i + 1
            ));
        }
        if lane.steps.len() != MAX_STEPS || lane.steps.iter().any(|&v| v > 2) {
            return Err(format!(
                "Lane {} needs {MAX_STEPS} steps of 0, 1 or 2",
                i + 1
            ));
        }
        if p.lanes[..i]
            .iter()
            .any(|o| (o.bank, o.x, o.y) == (lane.bank, lane.x, lane.y))
        {
            return Err("Lane already there".into());
        }
    }
    Ok(())
}

/// A pad moved: its lane follows. A lane that already sat on the (empty) target is dropped, so
/// there is one lane per pad. `from`/`to` are `(bank, x, y)`.
pub fn retarget_lanes(p: &mut Pattern, from: (u8, u8, u8), to: (u8, u8, u8)) {
    p.lanes.retain(|l| (l.bank, l.x, l.y) != to);
    for l in p.lanes.iter_mut().filter(|l| (l.bank, l.x, l.y) == from) {
        (l.bank, l.x, l.y) = to;
    }
}

/// Two pads swapped places: their lanes swap too.
pub fn swap_lanes(p: &mut Pattern, a: (u8, u8, u8), b: (u8, u8, u8)) {
    for l in &mut p.lanes {
        let at = (l.bank, l.x, l.y);
        if at == a {
            (l.bank, l.x, l.y) = b;
        } else if at == b {
            (l.bank, l.x, l.y) = a;
        }
    }
}
