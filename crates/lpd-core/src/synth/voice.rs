//! One note of the synth (spec §3.1–3.9): oscillators, sub, noise, the stereo filter, the amp,
//! three envelopes, two per-voice LFOs, glide and drift. Rendered in chunks of at most TICK samples.

use super::env::{Env, TICK};
use super::filter::{drive_gain, g_for, ladder_k, svf_q, VoiceFilter};
use super::lfo::{rate_hz, Lfo};
use super::matrix::{accumulate, live_norm, pitch_semis, Sources};
use super::osc::{next_bipolar, next_unit, NoiseState, OscState, OscTick, SubState, WarpShape};
use super::params::*;
use super::wavetable::table;
use std::f32::consts::PI;

/// What every voice shares during one chunk.
pub struct VoiceCtx<'a> {
    pub p: &'a SynthParams,
    /// Smoothed knob positions of the destinations.
    pub base: &'a [f32; DESTS],
    pub rate: f32,
    pub wheel: f32,
    /// Pitch wheel −1…1.
    pub bend: f32,
    pub tempo: f32,
    /// Outputs of the LFOs in FREE mode.
    pub free_lfo: [f32; LFOS],
}

/// What a chunk did to the voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rendered {
    Playing,
    Ended,
    /// The output stopped being finite; the voice was silenced and nothing was added.
    Reset,
}

#[derive(Clone, Copy, Debug)]
struct Drift {
    from: f32,
    to: f32,
    t: f32,
    period: f32,
}

impl Drift {
    fn new(rng: &mut u32) -> Drift {
        Drift {
            from: 0.0,
            to: next_bipolar(rng),
            t: 0.0,
            period: 1.0 / (0.1 + 0.2 * next_unit(rng)),
        }
    }

    fn tick(&mut self, rng: &mut u32, dt: f32) -> f32 {
        self.t += dt / self.period;
        if self.t >= 1.0 {
            self.t = 0.0;
            self.from = self.to;
            self.to = next_bipolar(rng);
            self.period = 1.0 / (0.1 + 0.2 * next_unit(rng));
        }
        self.from + (self.to - self.from) * (0.5 - 0.5 * (PI * self.t).cos())
    }
}

#[inline]
fn value(d: Dest, live: &[f32; DESTS]) -> f32 {
    match d.param() {
        Some(id) => id.meta().kind.from_norm(live[d as usize]),
        None => 0.0,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Voice {
    pub note: u8,
    /// 0–1.
    pub velocity: f32,
    /// The key is down.
    pub gate: bool,
    /// The key went up while the pedal was down.
    pub sustained: bool,
    pub age: u64,
    pub rand: f32,
    /// Current (glided) pitch as a note number.
    pub pitch: f32,
    target: f32,
    glide_left: f32,
    pub env: [Env; ENVS],
    pub lfo: [Lfo; LFOS],
    rate_mod: [f32; LFOS],
    osc: [OscState; 2],
    sub: SubState,
    noise: NoiseState,
    filter: VoiceFilter,
    drift: [Drift; 2],
    rng: u32,
    amp: f32,
    g: f32,
    /// The last chunk's values, for the monitor.
    pub src: Sources,
    pub live: [f32; DESTS],
    pub cutoff_hz: f32,
}

impl Voice {
    pub fn idle(seed: u32) -> Voice {
        let mut rng = seed.max(1);
        let drift = [Drift::new(&mut rng), Drift::new(&mut rng)];
        Voice {
            note: 0,
            velocity: 0.0,
            gate: false,
            sustained: false,
            age: 0,
            rand: 0.0,
            pitch: 60.0,
            target: 60.0,
            glide_left: 0.0,
            env: [Env::default(); ENVS],
            lfo: [Lfo::new(seed ^ 0x9E37_79B9), Lfo::new(seed ^ 0x7F4A_7C15)],
            rate_mod: [0.0; LFOS],
            osc: [OscState::default(); 2],
            sub: SubState::default(),
            noise: NoiseState::new(seed ^ 0x51ED_270B),
            filter: VoiceFilter::default(),
            drift,
            rng,
            amp: 0.0,
            g: 0.0,
            src: [0.0; SOURCES],
            live: [0.0; DESTS],
            cutoff_hz: 0.0,
        }
    }

    pub fn active(&self) -> bool {
        !self.env[0].is_idle()
    }

    /// A fresh note on an idle or stolen voice: phases restart per PHASE RAND, the filter is clean,
    /// envelopes start from 0. `glide_from` = the last note played, when GLIDE should apply.
    pub fn start(
        &mut self,
        p: &SynthParams,
        note: u8,
        velocity: u8,
        age: u64,
        glide_from: Option<f32>,
    ) {
        self.note = note;
        self.velocity = f32::from(velocity.min(127)) / 127.0;
        self.gate = true;
        self.sustained = false;
        self.age = age;
        self.rand = next_bipolar(&mut self.rng);
        self.target = f32::from(note);
        match glide_from {
            Some(from) if p.voice.glide > 0.0 => {
                self.pitch = from;
                self.glide_left = p.voice.glide;
            }
            _ => {
                self.pitch = self.target;
                self.glide_left = 0.0;
            }
        }
        self.osc[0].reset(p.osc_a.phase_rand, &mut self.rng);
        self.osc[1].reset(p.osc_b.phase_rand, &mut self.rng);
        self.sub = SubState::default();
        self.filter.reset();
        self.amp = 0.0;
        self.g = 0.0;
        self.rate_mod = [0.0; LFOS];
        for e in &mut self.env {
            e.level = 0.0;
            e.gate_on();
        }
        for (lfo, lp) in self.lfo.iter_mut().zip(&p.lfo) {
            lfo.restart(lp);
        }
    }

    /// The same voice moves to `note`: envelopes and LFOs restart (from their level) when
    /// `retrigger`; the pitch glides when `glide` and GLIDE > 0; `velocity: None` keeps the old one.
    pub fn retarget(
        &mut self,
        p: &SynthParams,
        note: u8,
        velocity: Option<u8>,
        retrigger: bool,
        glide: bool,
        age: u64,
    ) {
        self.note = note;
        self.target = f32::from(note);
        self.gate = true;
        self.sustained = false;
        self.age = age;
        if let Some(v) = velocity {
            self.velocity = f32::from(v.min(127)) / 127.0;
        }
        if glide && p.voice.glide > 0.0 {
            self.glide_left = p.voice.glide;
        } else {
            self.pitch = self.target;
            self.glide_left = 0.0;
        }
        if retrigger {
            self.rand = next_bipolar(&mut self.rng);
            for e in &mut self.env {
                e.gate_on();
            }
            for (lfo, lp) in self.lfo.iter_mut().zip(&p.lfo) {
                lfo.restart(lp);
            }
        }
    }

    pub fn release(&mut self) {
        self.gate = false;
        self.sustained = false;
        for e in &mut self.env {
            e.gate_off();
        }
    }

    /// All notes off: a 5 ms release.
    pub fn kill(&mut self) {
        self.gate = false;
        self.sustained = false;
        for e in &mut self.env {
            e.kill();
        }
    }

    /// Stop at once (patch load, unstable output).
    pub fn silence(&mut self) {
        self.env = [Env::default(); ENVS];
        self.filter.reset();
        self.gate = false;
        self.sustained = false;
        self.amp = 0.0;
        self.g = 0.0;
    }

    #[doc(hidden)]
    pub fn debug_poison(&mut self) {
        self.filter.debug_poison();
    }

    /// Render `len` (≤ TICK) samples and add them into `out_l` / `out_r`.
    pub fn render(
        &mut self,
        ctx: &VoiceCtx,
        len: usize,
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) -> Rendered {
        let p = ctx.p;
        let rate = ctx.rate;
        let dt = len as f32 / rate;

        // Sources.
        let e = [
            self.env[0].tick(&p.env[0], dt),
            self.env[1].tick(&p.env[1], dt),
            self.env[2].tick(&p.env[2], dt),
        ];
        let mut lfo = [0.0; LFOS];
        for i in 0..LFOS {
            lfo[i] = if p.lfo[i].mode == LfoMode::Free {
                ctx.free_lfo[i]
            } else {
                let d = if i == 0 {
                    Dest::Lfo1Rate
                } else {
                    Dest::Lfo2Rate
                };
                let hz = ParamId::Lfo1Rate
                    .meta()
                    .kind
                    .from_norm(live_norm(ctx.base[d as usize], self.rate_mod[i]));
                self.lfo[i].tick(&p.lfo[i], rate_hz(&p.lfo[i], hz, ctx.tempo), dt)
            };
        }
        self.src = [
            e[0],
            e[1],
            e[2],
            lfo[0],
            lfo[1],
            self.velocity,
            ((f32::from(self.note) - 60.0) / 60.0).clamp(-1.0, 1.0),
            self.rand,
            ctx.wheel,
            ctx.bend,
        ];

        // Matrix and live values.
        let mut m = [0.0; DESTS];
        accumulate(&p.routes, p, &self.src, &mut m);
        self.rate_mod = [m[Dest::Lfo1Rate as usize], m[Dest::Lfo2Rate as usize]];
        for &d in Dest::ALL {
            let i = d as usize;
            self.live[i] = if d.is_pitch() {
                0.5 + 0.5 * m[i].clamp(-1.0, 1.0)
            } else {
                live_norm(ctx.base[i], m[i])
            };
        }
        let v = |d: Dest| value(d, &self.live);

        // Glide and drift.
        if self.glide_left > 0.0 {
            let k = (dt / self.glide_left).min(1.0);
            self.pitch += (self.target - self.pitch) * k;
            self.glide_left = (self.glide_left - dt).max(0.0);
            if self.glide_left == 0.0 {
                self.pitch = self.target;
            }
        }
        let drift_semis = p.voice.drift * 0.08 * self.drift[0].tick(&mut self.rng, dt);
        let drift_oct = p.voice.drift * 0.3 * self.drift[1].tick(&mut self.rng, dt);
        let common = ctx.bend * f32::from(p.voice.bend_range)
            + drift_semis
            + pitch_semis(m[Dest::OscPitch as usize]);
        let hz_at =
            |offset: f32| 440.0 * ((self.pitch + offset.clamp(-48.0, 48.0) - 69.0) / 12.0).exp2();

        // Oscillator settings for this chunk.
        let warp_mod = |o: &OscParams| matches!(o.warp, Warp::Fm | Warp::Rm);
        let run_a = p.osc_a.on || (p.osc_b.on && warp_mod(&p.osc_b));
        let run_b = p.osc_b.on || (p.osc_a.on && warp_mod(&p.osc_a));
        let tick_for = |o: &OscParams, dests: [Dest; 9]| {
            let [pos, pitch, fine, warp, detune, blend, width, level, pan] = dests;
            let offset = common
                + f32::from(o.oct) * 12.0
                + f32::from(o.semi)
                + v(fine) / 100.0
                + pitch_semis(m[pitch as usize]);
            OscTick::new(
                table(o.table),
                rate,
                hz_at(offset),
                v(pos),
                usize::from(o.unison),
                v(detune),
                v(blend),
                v(width),
                v(pan),
                if o.on { v(level) } else { 0.0 },
                WarpShape::new(o.warp, v(warp)),
            )
        };
        use Dest::*;
        let ta = run_a.then(|| {
            tick_for(
                &p.osc_a,
                [
                    OscAPos, OscAPitch, OscAFine, OscAWarp, OscADetune, OscABlend, OscAWidth,
                    OscALevel, OscAPan,
                ],
            )
        });
        let tb = run_b.then(|| {
            tick_for(
                &p.osc_b,
                [
                    OscBPos, OscBPitch, OscBFine, OscBWarp, OscBDetune, OscBBlend, OscBWidth,
                    OscBLevel, OscBPan,
                ],
            )
        });
        let sub_inc = hz_at(common + f32::from(p.sub.oct) * 12.0) / rate;
        let sub_gain = v(SubLevel) * v(SubLevel);
        let noise_gain = v(NoiseLevel) * v(NoiseLevel);

        // Filter and amp for this chunk.
        let top = 20_000.0f32.min(0.45 * rate);
        let cutoff_oct = v(Cutoff).log2()
            + p.filter.env_amt * 8.0 * e[1]
            + p.filter.key_track * (self.pitch - 60.0) / 12.0
            + drift_oct;
        self.cutoff_hz = cutoff_oct.exp2().clamp(20.0, top);
        self.filter.set_kind(p.filter.kind);
        let res = if p.filter.kind == FilterType::Lp24 {
            ladder_k(v(Reso))
        } else {
            svf_q(v(Reso))
        };
        let dg = drive_gain(v(Drive));
        let g_end = g_for(self.cutoff_hz, rate);
        let g_start = if self.g > 0.0 { self.g } else { g_end };
        let lvl = v(AmpLevel);
        let vel = (1.0 - p.amp.vel_sens) + p.amp.vel_sens * self.velocity * self.velocity;
        let amp_end = e[0] * lvl * lvl * vel;
        let amp_start = self.amp;

        let mut bl = [0.0f32; TICK];
        let mut br = [0.0f32; TICK];
        for i in 0..len {
            let t = (i + 1) as f32 / len as f32;
            let (ma, mb) = (self.osc[1].last_centre, self.osc[0].last_centre);
            let (mut fl, mut fr, mut dl, mut dr) = (0.0, 0.0, 0.0, 0.0);
            if let Some(t) = &ta {
                let (l, r) = self.osc[0].sample(t, ma);
                if p.osc_a.to_filter {
                    fl += l;
                    fr += r;
                } else {
                    dl += l;
                    dr += r;
                }
            }
            if let Some(t) = &tb {
                let (l, r) = self.osc[1].sample(t, mb);
                if p.osc_b.to_filter {
                    fl += l;
                    fr += r;
                } else {
                    dl += l;
                    dr += r;
                }
            }
            if p.sub.on {
                let s = self.sub.sample(p.sub.shape, sub_inc) * sub_gain;
                if p.sub.to_filter {
                    fl += s;
                    fr += s;
                } else {
                    dl += s;
                    dr += s;
                }
            }
            if p.noise.on {
                let s = self.noise.sample(p.noise.color) * noise_gain;
                if p.noise.to_filter {
                    fl += s;
                    fr += s;
                } else {
                    dl += s;
                    dr += s;
                }
            }
            let (yl, yr) = if p.filter.on {
                self.filter
                    .process(fl, fr, g_start + (g_end - g_start) * t, res, dg)
            } else {
                (fl, fr)
            };
            let amp = amp_start + (amp_end - amp_start) * t;
            bl[i] = (yl + dl) * amp;
            br[i] = (yr + dr) * amp;
        }
        self.g = g_end;
        self.amp = amp_end;

        if !bl[..len].iter().chain(&br[..len]).all(|x| x.is_finite()) || !self.filter.is_finite() {
            self.silence();
            return Rendered::Reset;
        }
        for i in 0..len {
            out_l[i] += bl[i];
            out_r[i] += br[i];
        }
        if self.active() {
            Rendered::Playing
        } else {
            Rendered::Ended
        }
    }
}
