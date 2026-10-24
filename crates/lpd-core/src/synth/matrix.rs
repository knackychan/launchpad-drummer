//! Modulation matrix maths (spec §4.3, §4.4).

use super::params::{Dest, LfoMode, Route, RouteTable, Source, SynthParams, DESTS, SOURCES};

/// Source values of one voice at one control tick, indexed by `Source as usize`.
pub type Sources = [f32; SOURCES];

/// Sources shared by every voice: WHEEL, BEND and an LFO in FREE mode.
pub fn is_global(source: Source, p: &SynthParams) -> bool {
    match source {
        Source::Wheel | Source::Bend => true,
        Source::Lfo1 => p.lfo[0].mode == LfoMode::Free,
        Source::Lfo2 => p.lfo[1].mode == LfoMode::Free,
        _ => false,
    }
}

/// Routes the LFO rules keep out of a voice's sum: a route to the RATE of an LFO in SYNC, or to
/// the RATE of a FREE LFO (that LFO is global; see `accumulate_global`).
fn per_voice_skip(r: &Route, p: &SynthParams) -> bool {
    match r.dest {
        Some(Dest::Lfo1Rate) => p.lfo[0].sync || p.lfo[0].mode == LfoMode::Free,
        Some(Dest::Lfo2Rate) => p.lfo[1].sync || p.lfo[1].mode == LfoMode::Free,
        _ => false,
    }
}

#[inline]
fn contribution(r: &Route, src: &Sources) -> f32 {
    let aux = r.aux.map_or(1.0, |a| src[a as usize]);
    r.amount * src[r.source as usize] * aux
}

/// Σ amount × source × aux per destination for one voice.
pub fn accumulate(routes: &RouteTable, p: &SynthParams, src: &Sources, out: &mut [f32; DESTS]) {
    out.fill(0.0);
    for r in routes.iter() {
        let Some(dest) = r.dest else { continue };
        if r.bypass || per_voice_skip(r, p) {
            continue;
        }
        out[dest as usize] += contribution(r, src);
    }
}

/// The modulation of a FREE LFO's RATE (`lfo` 0 or 1): only routes whose source and aux are global.
pub fn accumulate_global(routes: &RouteTable, p: &SynthParams, src: &Sources, lfo: usize) -> f32 {
    let dest = if lfo == 0 {
        Dest::Lfo1Rate
    } else {
        Dest::Lfo2Rate
    };
    if p.lfo[lfo].sync {
        return 0.0;
    }
    routes
        .iter()
        .filter(|r| {
            r.dest == Some(dest)
                && !r.bypass
                && is_global(r.source, p)
                && r.aux.is_none_or(|a| is_global(a, p))
        })
        .map(|r| contribution(r, src))
        .sum()
}

/// A knob destination's live position: the knob plus its modulation, clamped to 0–1.
#[inline]
pub fn live_norm(base: f32, m: f32) -> f32 {
    (base + m).clamp(0.0, 1.0)
}

/// The pitch destinations: 100 % = ±24 semitones.
#[inline]
pub fn pitch_semis(m: f32) -> f32 {
    24.0 * m
}

/// Knob position of every destination (0 for the pitch destinations).
pub fn base_norms(p: &SynthParams) -> [f32; DESTS] {
    let mut out = [0.0; DESTS];
    for (i, d) in Dest::ALL.iter().enumerate() {
        if let Some(id) = d.param() {
            out[i] = id.meta().kind.to_norm(id.get(p));
        }
    }
    out
}
