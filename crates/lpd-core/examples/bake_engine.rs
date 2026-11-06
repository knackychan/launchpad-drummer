//! Dustline ship engine bake tool (spec `2026-11-05-dustline-ship-engine-design.md`, section 4.2).
//!
//!   cargo run -p lpd-core --release --example bake_engine -- check --patch <file.lbpatch>
//!   cargo run -p lpd-core --release --example bake_engine -- loops --patch <file.lbpatch> --out <dir>
//!   cargo run -p lpd-core --release --example bake_engine -- scenario --patch <file.lbpatch> --frames <frames.json> --out <file.wav>
//!
//! `check` parses the patch the way the app does (`Patch::from_json`) and fails if any value had to be clamped.
//! `loops` renders the eight phase-locked tone loops (4 anchors x on/off load) as 32-bit float stereo WAV files at
//! 48 kHz, 100800 samples each: 0.05 s lead-in, the 2 s loop, then 0.05 s of what follows the loop end, and
//! writes `bake.json` beside them. `scenario` plays a recorded 40 Hz game frame log through `GameEngineVoice`, the
//! LIVE SYNTH voice, so LIVE can be auditioned offline.

use anyhow::{bail, Context, Result};
use lpd_core::instrument::Instrument;
use lpd_core::synth::game::{
    GameEngineVoice, GameFrame, GAME_BEND_RANGE, GAME_NOTE, GAME_VELOCITY,
};
use lpd_core::synth::params::{SynthParams, VoiceMode};
use lpd_core::synth::patch::Patch;
use lpd_core::synth::Synth;
use std::path::{Path, PathBuf};

const RATE: u32 = 48_000;
const BLOCK: usize = 256;
/// Frequency of the held note's fundamental at bend 0: note 48 sounds twice it.
const BEND_ZERO_HZ: f64 = 65.406_391_325_149_66;
const ANCHORS: [u32; 4] = [48, 75, 117, 182];
const LOADS: [(&str, f32); 2] = [("on", 0.85), ("off", 0.2)];
const RENDER_S: f64 = 3.25;
/// Dropped from the note start: whole periods of every integer anchor, so all loops start in phase.
const DROP: usize = 48_000;
const LOOP: usize = 96_000;
const MARGIN: usize = 2_400;
const SEAM: usize = 7_200;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn need(args: &[String], name: &str) -> Result<String> {
    arg(args, name).with_context(|| format!("missing {name}"))
}

/// The patch as LIVE SYNTH plays it: parsed like the app, LEGATO and a 24-semitone bend forced.
fn load_patch(path: &Path) -> Result<(Patch, SynthParams)> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let patch = Patch::from_json(&text)?;
    let mut params = patch.params;
    params.voice.mode = VoiceMode::Legato;
    params.voice.bend_range = GAME_BEND_RANGE;
    Ok((patch, params))
}

/// Bend (−1…1 of 24 semitones) that puts the fundamental at `hz` (spec 5.3).
fn bend_for(hz: f64) -> f32 {
    (12.0 * (hz / BEND_ZERO_HZ).log2() / f64::from(GAME_BEND_RANGE)) as f32
}

fn check(args: &[String]) -> Result<()> {
    let path = PathBuf::from(need(args, "--patch")?);
    let text = std::fs::read_to_string(&path)?;
    let raw: Patch = serde_json::from_str(&text).context("not a patch file")?;
    let clean = Patch::from_json(&text)?;
    let p = (&raw.params, &clean.params);
    let mut changed = Vec::new();
    macro_rules! section {
        ($($f:ident),*) => { $( if p.0.$f != p.1.$f { changed.push(stringify!($f)); } )* };
    }
    section!(osc_a, osc_b, sub, noise, filter, amp, env, lfo, voice, routes, fx);
    if raw.name != clean.name {
        changed.push("name");
    }
    if !changed.is_empty() {
        bail!("values out of range in: {}", changed.join(", "));
    }
    if clean.params.voice.mode != VoiceMode::Legato
        || clean.params.voice.bend_range != GAME_BEND_RANGE
    {
        println!("note: LIVE SYNTH and the bake force LEGATO and a 24-semitone bend");
    }
    println!(
        "patch ok: {} ({}), nothing clamped",
        clean.name,
        path.display()
    );
    Ok(())
}

fn render_note(params: &SynthParams, hz: f64, wheel: f32, seconds: f64) -> (Vec<f32>, Vec<f32>) {
    let mut synth = Synth::new(RATE);
    synth.load_params(params);
    let (mut l, mut r) = (vec![0.0f32; BLOCK], vec![0.0f32; BLOCK]);
    for _ in 0..8 {
        synth.render_add(&mut l, &mut r); // the 5 ms load fade, before the note
    }
    synth.pitch_bend(bend_for(hz));
    synth.modulation(wheel);
    synth.note_on(GAME_NOTE, GAME_VELOCITY);
    let n = (seconds * f64::from(RATE)).round() as usize;
    let (mut outl, mut outr) = (vec![0.0f32; n], vec![0.0f32; n]);
    for (cl, cr) in outl.chunks_mut(BLOCK).zip(outr.chunks_mut(BLOCK)) {
        synth.render_add(cl, cr);
    }
    (outl, outr)
}

/// Correlation of two stereo segments (both channels together).
fn correlation(a: (&[f32], &[f32]), b: (&[f32], &[f32])) -> f64 {
    let (mut ab, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
    for (x, y) in a.0.iter().zip(b.0).chain(a.1.iter().zip(b.1)) {
        let (x, y) = (f64::from(*x), f64::from(*y));
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    if aa == 0.0 || bb == 0.0 {
        0.0
    } else {
        ab / (aa * bb).sqrt()
    }
}

/// The loop file: lead-in, the loop with its end crossfaded into what precedes the loop start, then the loop start
/// again (what follows the seam). The crossfade keeps the power constant for the measured correlation `rho`.
fn make_loop(x: &[f32], rho: f64) -> Vec<f32> {
    let a = DROP;
    let b = a + LOOP;
    let mut body = x[a..b].to_vec();
    for k in 0..SEAM {
        let th = (k as f64 + 0.5) / SEAM as f64 * std::f64::consts::FRAC_PI_2;
        let (we, wp) = (th.cos(), th.sin());
        let g = 1.0 / (we * we + wp * wp + 2.0 * rho * we * wp).sqrt();
        let i = LOOP - SEAM + k;
        body[i] = (g * (we * f64::from(x[a + i]) + wp * f64::from(x[a - SEAM + k]))) as f32;
    }
    let mut out = Vec::with_capacity(LOOP + 2 * MARGIN);
    out.extend_from_slice(&x[a - MARGIN..a]);
    out.extend_from_slice(&body);
    out.extend_from_slice(&x[a..a + MARGIN]);
    out
}

fn write_wav(path: &Path, l: &[f32], r: &[f32]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for (a, b) in l.iter().zip(r) {
        w.write_sample(*a)?;
        w.write_sample(*b)?;
    }
    w.finalize()?;
    Ok(())
}

fn loops(args: &[String]) -> Result<()> {
    let (patch, params) = load_patch(Path::new(&need(args, "--patch")?))?;
    let out = PathBuf::from(need(args, "--out")?);
    std::fs::create_dir_all(&out)?;
    let mut manifest = Vec::new();
    for hz in ANCHORS {
        for (load, wheel) in LOADS {
            let (l, r) = render_note(&params, f64::from(hz), wheel, RENDER_S);
            let (a, b) = (DROP, DROP + LOOP);
            let rho = correlation(
                (&l[b - SEAM..b], &r[b - SEAM..b]),
                (&l[a - SEAM..a], &r[a - SEAM..a]),
            );
            let (ll, rr) = (make_loop(&l, rho), make_loop(&r, rho));
            let id = format!("core-{hz:03}-{load}");
            write_wav(&out.join(format!("{id}.wav")), &ll, &rr)?;
            let peak = ll.iter().chain(&rr).fold(0.0f32, |m, v| m.max(v.abs()));
            println!("{id}: {hz} Hz, wheel {wheel}, seam correlation {rho:.3}, peak {peak:.3}");
            manifest.push(serde_json::json!({
                "id": id, "anchorHz": hz, "load": load, "wheel": wheel, "bend": bend_for(f64::from(hz)),
                "rate": RATE, "samples": ll.len(), "loopStart": MARGIN, "loopEnd": MARGIN + LOOP,
                "seamCorrelation": rho, "peak": peak
            }));
        }
    }
    let doc = serde_json::json!({ "patch": patch.name, "loops": manifest });
    std::fs::write(
        out.join("bake.json"),
        serde_json::to_string_pretty(&doc)? + "\n",
    )?;
    println!(
        "wrote {} loops to {}",
        ANCHORS.len() * LOADS.len(),
        out.display()
    );
    Ok(())
}

/// Frames: `{"rate": 40, "frames": [[on, bend, wheel, gain], ...]}`, one every 1/40 s from t = 0.
fn scenario(args: &[String]) -> Result<()> {
    let (_, params) = load_patch(Path::new(&need(args, "--patch")?))?;
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(need(args, "--frames")?)?)?;
    if doc["rate"].as_u64() != Some(40) {
        bail!("frames must be at 40 Hz");
    }
    let frames = doc["frames"].as_array().context("no frames")?;
    let per = RATE as usize / 40;
    let mut voice = GameEngineVoice::new(RATE, &params);
    let (mut l, mut r) = (
        vec![0.0f32; frames.len() * per + RATE as usize / 2],
        vec![0.0f32; frames.len() * per + RATE as usize / 2],
    );
    for (i, f) in frames.iter().enumerate() {
        let v = |k: usize| f[k].as_f64().unwrap_or(f64::NAN) as f32;
        voice.frame(GameFrame {
            on: f[0].as_bool().unwrap_or(false),
            bend: v(1),
            wheel: v(2),
            gain: v(3),
        });
        voice.render_add(
            &mut l[i * per..(i + 1) * per],
            &mut r[i * per..(i + 1) * per],
        );
    }
    let tail = frames.len() * per;
    voice.render_add(&mut l[tail..], &mut r[tail..]);
    let out = PathBuf::from(need(args, "--out")?);
    write_wav(&out, &l, &r)?;
    println!(
        "wrote {} ({} frames, {:.2} s)",
        out.display(),
        frames.len(),
        l.len() as f64 / f64::from(RATE)
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") => check(&args),
        Some("loops") => loops(&args),
        Some("scenario") => scenario(&args),
        _ => bail!("usage: bake_engine check|loops|scenario --patch <file> [--out <dir|file>] [--frames <json>]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, seconds: f64) -> Vec<f32> {
        let n = (seconds * f64::from(RATE)) as usize;
        (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * hz * i as f64 / f64::from(RATE)).sin() as f32)
            .collect()
    }

    #[test]
    fn the_bend_puts_the_fundamental_on_the_anchor() {
        assert!(bend_for(BEND_ZERO_HZ).abs() < 1e-7);
        for hz in [41.0, 48.0, 75.0, 117.0, 182.0, 191.0] {
            let back = BEND_ZERO_HZ
                * 2f64.powf(f64::from(bend_for(hz)) * f64::from(GAME_BEND_RANGE) / 12.0);
            assert!((back / hz - 1.0).abs() < 1e-6, "{hz}");
            assert!(bend_for(hz).abs() <= 1.0);
        }
    }

    #[test]
    fn a_periodic_signal_loops_unchanged_and_seamless() {
        let x = sine(75.0, RENDER_S);
        let (a, b) = (DROP, DROP + LOOP);
        let rho = correlation(
            (&x[b - SEAM..b], &x[b - SEAM..b]),
            (&x[a - SEAM..a], &x[a - SEAM..a]),
        );
        assert!(rho > 0.999, "{rho}");
        let out = make_loop(&x, rho);
        assert_eq!(out.len(), LOOP + 2 * MARGIN);
        for (i, v) in out.iter().enumerate() {
            assert!((v - x[DROP - MARGIN + i]).abs() < 1e-3, "sample {i}");
        }
        // across the seam the loop continues the sine: the step equals the step inside the loop
        let step = |p: f32, q: f32| (q - p).abs();
        let max_step = out.windows(2).map(|w| step(w[0], w[1])).fold(0.0, f32::max);
        assert!(step(out[MARGIN + LOOP - 1], out[MARGIN]) <= max_step * 1.01);
        // the margins: what precedes the loop start, and the loop start again after the loop end
        assert_eq!(&out[LOOP + MARGIN..], &out[MARGIN..2 * MARGIN]);
    }

    #[test]
    fn uncorrelated_ends_keep_their_power_through_the_crossfade() {
        let mut seed = 7u32;
        let x: Vec<f32> = (0..(RENDER_S * f64::from(RATE)) as usize)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
            })
            .collect();
        let (a, b) = (DROP, DROP + LOOP);
        let rho = correlation(
            (&x[b - SEAM..b], &x[b - SEAM..b]),
            (&x[a - SEAM..a], &x[a - SEAM..a]),
        );
        assert!(rho.abs() < 0.05, "{rho}");
        let out = make_loop(&x, rho);
        let rms = |s: &[f32]| {
            (s.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / s.len() as f64).sqrt()
        };
        let body = rms(&out[MARGIN..MARGIN + LOOP - SEAM]);
        let mid = rms(&out[MARGIN + LOOP - SEAM / 2 - 1200..MARGIN + LOOP - SEAM / 2 + 1200]);
        assert!(
            (20.0 * (mid / body).log10()).abs() < 0.5,
            "crossfade {mid} body {body}"
        );
    }

    #[test]
    fn correlation_is_one_for_equal_and_minus_one_for_inverted() {
        let x = sine(48.0, 0.5);
        let y: Vec<f32> = x.iter().map(|v| -v).collect();
        assert!((correlation((&x, &x), (&x, &x)) - 1.0).abs() < 1e-9);
        assert!((correlation((&x, &x), (&y, &y)) + 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_rendered_note_sits_on_its_anchor() {
        let mut p = SynthParams::default();
        p.osc_a.on = false;
        p.osc_b.on = false;
        p.sub.on = true;
        p.sub.oct = -1;
        p.voice.mode = VoiceMode::Legato;
        p.voice.bend_range = GAME_BEND_RANGE;
        let (l, _) = render_note(&p, 75.0, 0.5, RENDER_S);
        let body = &l[DROP..DROP + LOOP];
        let crossings = body
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        assert!(
            (149..=151).contains(&crossings),
            "{crossings} upward crossings in 2 s"
        );
    }
}
