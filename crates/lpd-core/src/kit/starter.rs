//! The starter set behind the default set: the mockup's 808 kit (three banks), its mix groups and
//! its five-lane groove. The samples are synthesised here and written as WAV files, so a fresh
//! set sounds out of the box and stays an ordinary folder of files.

use super::{Pad, PadMode, Set, SAMPLES_DIR};
use crate::seq::{Lane, MAX_STEPS};
use anyhow::{Context, Result};
use std::f32::consts::TAU;
use std::fs;
use std::path::Path;

/// Folder name of the default set.
pub const DEFAULT_SET_NAME: &str = "Default Set";

const RATE: u32 = 44_100;
const GROUP_NAMES: [&str; 8] = ["KICK", "SNARE", "HATS", "TOMS", "PERC", "CYMB", "FX", "VOX"];

/// Bank rows, top row (y = 7) first, as drawn in the mockup.
const BANK_KIT: [[&str; 8]; 8] = [
    [
        "riser_8bar",
        "impact",
        "reverse_sweep",
        "zap",
        "vox_hey",
        "",
        "",
        "",
    ],
    ["crash_1", "china", "", "", "", "", "ride_1", "ride_bell"],
    [
        "conga_lo", "conga_hi", "cowbell", "clave", "tamb", "shaker", "", "",
    ],
    [
        "tom_low", "tom_mid", "tom_high", "", "", "tom_low", "tom_mid", "tom_high",
    ],
    [
        "hat_closed",
        "hat_pedal",
        "hat_open",
        "",
        "",
        "hat_closed",
        "hat_pedal",
        "hat_open",
    ],
    [
        "snare_808",
        "rim_808",
        "clap_808",
        "",
        "",
        "clap_808",
        "rim_808",
        "snare_909",
    ],
    [
        "kick_808", "kick_sub", "", "", "", "", "kick_sub", "kick_909",
    ],
    ["kick_acoustic", "", "", "", "", "", "", "kick_acoustic"],
];
const BANK_PERC: [[&str; 8]; 4] = [
    ["shaker", "tamb", "", "", "", "", "tamb", "shaker"],
    ["cowbell", "clave", "", "", "", "", "clave", "cowbell"],
    [
        "conga_hi", "conga_lo", "", "", "", "", "conga_lo", "conga_hi",
    ],
    ["kick_909", "", "", "", "", "", "", "kick_909"],
];
const BANK_FX: [[&str; 8]; 3] = [
    ["vox_hey", "vox_yeah", "vox_ho", "", "", "", "", ""],
    ["zap", "impact", "", "", "", "", "", ""],
    ["riser_8bar", "reverse_sweep", "", "", "", "", "", ""],
];

/// (bank, x, y, pattern): `X` accent, `x` on, `.` off; repeated to fill the 32 steps.
const LANES: [(u8, u8, u8, &str); 5] = [
    (0, 0, 1, "X.....x...x....."),
    (0, 0, 2, "....X.......X..."),
    (0, 2, 2, "............x..x"),
    (0, 0, 3, "x.x.X.x.x.x.X.x."),
    (0, 2, 3, "..............x."),
];

#[derive(Clone, Copy, PartialEq)]
enum Cat {
    Kick,
    Snare,
    Hat,
    Tom,
    Perc,
    Cym,
    Fx,
    Vox,
}

fn category(name: &str) -> Cat {
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    if has(&["kick"]) {
        Cat::Kick
    } else if has(&["snare", "clap", "rim"]) {
        Cat::Snare
    } else if has(&["hat", "shaker"]) {
        Cat::Hat
    } else if has(&["tom"]) {
        Cat::Tom
    } else if has(&["crash", "ride", "china"]) {
        Cat::Cym
    } else if has(&["vox"]) {
        Cat::Vox
    } else if has(&["riser", "impact", "zap", "sweep"]) {
        Cat::Fx
    } else {
        Cat::Perc
    }
}

impl Cat {
    /// Launchpad LED colour (0..=63 per channel) and mixer group.
    fn look(self) -> ([u8; 3], u8) {
        match self {
            Cat::Kick => ([63, 0, 0], 0),
            Cat::Snare => ([63, 20, 0], 1),
            Cat::Hat => ([63, 63, 0], 2),
            Cat::Tom => ([0, 63, 0], 3),
            Cat::Perc => ([0, 63, 63], 4),
            Cat::Cym => ([0, 30, 63], 5),
            Cat::Fx => ([45, 0, 63], 6),
            Cat::Vox => ([63, 10, 30], 7),
        }
    }
}

fn pad(x: u8, y: u8, name: &str) -> Pad {
    let cat = category(name);
    let (color, group) = cat.look();
    let mut p = Pad::new(x, y, format!("{SAMPLES_DIR}/{name}.wav"), color);
    p.mix_group = group;
    if name.starts_with("hat_") {
        p.choke_group = Some(1);
    }
    if name.contains("riser") {
        p.mode = PadMode::Hold;
    }
    p
}

/// A new set filled with the starter kit and groove.
pub fn starter_set() -> Set {
    let mut set = Set::new();
    for (bank, name, rows) in [
        (0, "KITS 808", &BANK_KIT[..]),
        (1, "PERC", &BANK_PERC[..]),
        (2, "FX", &BANK_FX[..]),
    ] {
        set.banks[bank].name = name.into();
        // The shorter banks fill the rows from the bottom up (y = 0 is the bottom row).
        for (i, row) in rows.iter().rev().enumerate() {
            let y = i as u8;
            for (x, sample) in row.iter().enumerate().filter(|(_, s)| !s.is_empty()) {
                set.banks[bank].set_pad(pad(x as u8, y, sample));
            }
        }
    }
    set.banks[1].lights.ripple = true;
    for (group, name) in set.mixer.groups.iter_mut().zip(GROUP_NAMES) {
        group.name = name.into();
    }
    set.pattern.lanes = LANES
        .iter()
        .map(|&(bank, x, y, pat)| {
            let mut steps = vec![0u8; MAX_STEPS];
            for (i, c) in pat.chars().enumerate() {
                let v = match c {
                    'X' => 2,
                    'x' => 1,
                    _ => 0,
                };
                steps[i] = v;
                steps[i + 16] = v;
            }
            Lane {
                bank,
                x,
                y,
                mute: false,
                steps,
            }
        })
        .collect();
    set
}

/// Synthesise and write every sample the set references that is not on disk yet.
pub fn write_starter_samples(dir: &Path, set: &Set) -> Result<()> {
    for pad in set.banks.iter().flat_map(|b| &b.pads) {
        let path = dir.join(&pad.sample);
        if path.exists() {
            continue;
        }
        let name = Path::new(&pad.sample)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        fs::create_dir_all(path.parent().unwrap_or(dir))?;
        fs::write(&path, wav16(&synth(name)))
            .with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}

// ---- synthesis ---------------------------------------------------------------------------------

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

#[derive(Clone, Copy)]
enum Filter {
    Low,
    High,
    Band,
}

/// RBJ biquad.
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}
impl Biquad {
    fn new(kind: Filter, hz: f32, q: f32) -> Biquad {
        let w = TAU * hz.min(RATE as f32 * 0.45) / RATE as f32;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q);
        let (b0, b1, b2) = match kind {
            Filter::Low => ((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0),
            Filter::High => ((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0),
            Filter::Band => (alpha, 0.0, -alpha),
        };
        let a0 = 1.0 + alpha;
        Biquad {
            b: [b0 / a0, b1 / a0, b2 / a0],
            a: [-2.0 * c / a0, (1.0 - alpha) / a0],
            z: [0.0; 2],
        }
    }
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

fn len(seconds: f32) -> usize {
    (seconds * RATE as f32) as usize
}

fn time(i: usize) -> f32 {
    i as f32 / RATE as f32
}

/// Exponential decay reaching -60 dB at `dec` seconds.
fn env(t: f32, dec: f32) -> f32 {
    (-6.9 * t / dec).exp()
}

/// Sine with an exponential pitch sweep `f0 -> f1` over `sweep` seconds, decaying over `dec`.
fn sweep_tone(f0: f32, f1: f32, sweep: f32, dec: f32, seconds: f32) -> Vec<f32> {
    let mut phase = 0.0;
    (0..len(seconds))
        .map(|i| {
            let t = time(i);
            let f = f1 + (f0 - f1) * (-t / sweep).exp();
            phase += TAU * f / RATE as f32;
            phase.sin() * env(t, dec)
        })
        .collect()
}

fn noise(seed: u32, kind: Filter, hz: f32, q: f32, dec: f32, seconds: f32) -> Vec<f32> {
    let mut rng = Rng(seed);
    let mut f = Biquad::new(kind, hz, q);
    (0..len(seconds))
        .map(|i| f.run(rng.next()) * env(time(i), dec))
        .collect()
}

/// Sum `src` into `dst` at `gain`, growing `dst` if needed.
fn mix(dst: &mut Vec<f32>, src: &[f32], gain: f32) {
    if dst.len() < src.len() {
        dst.resize(src.len(), 0.0);
    }
    for (d, s) in dst.iter_mut().zip(src) {
        *d += s * gain;
    }
}

fn square_tone(freqs: &[f32], dec: f32, seconds: f32) -> Vec<f32> {
    (0..len(seconds))
        .map(|i| {
            let t = time(i);
            let sum: f32 = freqs
                .iter()
                .map(|f| if (t * f).fract() < 0.5 { 1.0 } else { -1.0 })
                .sum();
            sum / freqs.len() as f32 * env(t, dec)
        })
        .collect()
}

fn synth(name: &str) -> Vec<f32> {
    let seed = name.bytes().fold(2_166_136_261u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16_777_619)
    }) | 1;
    let mut out: Vec<f32> = Vec::new();
    match name {
        "kick_808" => mix(&mut out, &sweep_tone(150.0, 45.0, 0.07, 0.6, 0.7), 1.0),
        "kick_909" => {
            mix(&mut out, &sweep_tone(190.0, 52.0, 0.04, 0.4, 0.5), 1.0);
            mix(
                &mut out,
                &noise(seed, Filter::High, 3000.0, 0.7, 0.012, 0.05),
                0.4,
            );
        }
        "kick_acoustic" => {
            mix(&mut out, &sweep_tone(120.0, 62.0, 0.03, 0.3, 0.4), 1.0);
            mix(
                &mut out,
                &noise(seed, Filter::Low, 900.0, 0.7, 0.05, 0.1),
                0.5,
            );
        }
        "kick_sub" => mix(&mut out, &sweep_tone(85.0, 40.0, 0.09, 0.9, 1.0), 1.0),
        "snare_808" => {
            mix(
                &mut out,
                &noise(seed, Filter::Band, 1800.0, 0.8, 0.22, 0.3),
                0.9,
            );
            mix(&mut out, &sweep_tone(190.0, 150.0, 0.05, 0.12, 0.2), 0.5);
        }
        "snare_909" => {
            mix(
                &mut out,
                &noise(seed, Filter::High, 2200.0, 0.7, 0.2, 0.3),
                0.8,
            );
            mix(&mut out, &sweep_tone(230.0, 170.0, 0.03, 0.1, 0.2), 0.6);
        }
        "rim_808" => {
            mix(&mut out, &square_tone(&[1700.0, 520.0], 0.02, 0.06), 0.8);
            mix(
                &mut out,
                &noise(seed, Filter::Band, 3000.0, 2.0, 0.015, 0.04),
                0.4,
            );
        }
        "clap_808" => {
            let hits = noise(seed, Filter::Band, 1200.0, 1.2, 0.012, 0.03);
            for at in [0.0, 0.011, 0.022] {
                let mut burst = vec![0.0; len(at)];
                burst.extend_from_slice(&hits);
                mix(&mut out, &burst, 0.8);
            }
            let mut tail = vec![0.0; len(0.033)];
            tail.extend(noise(seed ^ 7, Filter::Band, 1200.0, 1.2, 0.15, 0.25));
            mix(&mut out, &tail, 0.9);
        }
        "hat_closed" => mix(
            &mut out,
            &noise(seed, Filter::High, 7000.0, 0.7, 0.05, 0.1),
            0.8,
        ),
        "hat_pedal" => mix(
            &mut out,
            &noise(seed, Filter::High, 6000.0, 0.7, 0.08, 0.14),
            0.7,
        ),
        "hat_open" => mix(
            &mut out,
            &noise(seed, Filter::High, 7000.0, 0.7, 0.4, 0.6),
            0.8,
        ),
        "shaker" => mix(
            &mut out,
            &noise(seed, Filter::Band, 6500.0, 1.5, 0.1, 0.16),
            0.9,
        ),
        "tom_low" => mix(&mut out, &sweep_tone(130.0, 80.0, 0.12, 0.5, 0.6), 1.0),
        "tom_mid" => mix(&mut out, &sweep_tone(180.0, 110.0, 0.1, 0.45, 0.55), 1.0),
        "tom_high" => mix(&mut out, &sweep_tone(250.0, 160.0, 0.09, 0.4, 0.5), 1.0),
        "conga_lo" => mix(&mut out, &sweep_tone(230.0, 190.0, 0.02, 0.25, 0.3), 1.0),
        "conga_hi" => mix(&mut out, &sweep_tone(380.0, 320.0, 0.02, 0.2, 0.25), 1.0),
        "cowbell" => mix(&mut out, &square_tone(&[540.0, 800.0], 0.25, 0.4), 0.7),
        "clave" => mix(&mut out, &sweep_tone(2600.0, 2400.0, 0.01, 0.06, 0.1), 1.0),
        "tamb" => {
            mix(
                &mut out,
                &noise(seed, Filter::High, 5000.0, 0.8, 0.12, 0.2),
                0.6,
            );
            mix(
                &mut out,
                &noise(seed ^ 3, Filter::Band, 9000.0, 3.0, 0.2, 0.25),
                0.5,
            );
        }
        "crash_1" => mix(
            &mut out,
            &noise(seed, Filter::High, 5200.0, 0.5, 1.6, 2.0),
            0.8,
        ),
        "china" => mix(
            &mut out,
            &noise(seed, Filter::Band, 4200.0, 0.6, 1.0, 1.4),
            0.9,
        ),
        "ride_1" => {
            mix(
                &mut out,
                &noise(seed, Filter::High, 7500.0, 0.6, 1.0, 1.4),
                0.4,
            );
            mix(&mut out, &square_tone(&[3100.0, 4700.0], 0.6, 1.0), 0.15);
        }
        "ride_bell" => {
            for (f, g) in [(1130.0, 0.5), (2280.0, 0.35), (3410.0, 0.25)] {
                mix(&mut out, &sweep_tone(f, f, 1.0, 0.9, 1.1), g);
            }
            mix(
                &mut out,
                &noise(seed, Filter::High, 8000.0, 0.6, 0.15, 0.2),
                0.3,
            );
        }
        "riser_8bar" => {
            let n = len(2.0);
            let mut rng = Rng(seed);
            out = (0..n)
                .map(|i| {
                    let p = i as f32 / n as f32;
                    rng.next() * p * p
                })
                .collect();
            // One-pole low-pass opening with the sweep.
            let mut y = 0.0;
            for (i, s) in out.iter_mut().enumerate() {
                let p = i as f32 / n as f32;
                y += (0.02 + 0.9 * p * p) * (*s - y);
                *s = y;
            }
        }
        "impact" => {
            mix(
                &mut out,
                &noise(seed, Filter::Low, 1800.0, 0.7, 0.8, 1.5),
                0.6,
            );
            mix(&mut out, &sweep_tone(90.0, 35.0, 0.3, 1.2, 1.6), 1.0);
        }
        "reverse_sweep" => {
            let mut rev = noise(seed, Filter::Band, 2500.0, 1.5, 0.5, 1.2);
            rev.reverse();
            mix(&mut out, &rev, 0.9);
        }
        "zap" => mix(&mut out, &sweep_tone(3200.0, 120.0, 0.08, 0.25, 0.35), 0.9),
        _ if name.starts_with("vox_") => {
            let (f0, f1) = match name {
                "vox_hey" => (320.0, 260.0),
                "vox_yeah" => (260.0, 200.0),
                _ => (190.0, 150.0),
            };
            let n = len(0.5);
            let mut phase = 0.0f32;
            let saw: Vec<f32> = (0..n)
                .map(|i| {
                    let t = time(i);
                    phase += (f0 + (f1 - f0) * t / 0.5) / RATE as f32;
                    (phase.fract() * 2.0 - 1.0) * (t * 60.0).min(1.0) * env(t, 0.4)
                })
                .collect();
            for (hz, q, g) in [(700.0, 5.0, 1.0), (1200.0, 6.0, 0.7), (2600.0, 8.0, 0.4)] {
                let mut f = Biquad::new(Filter::Band, hz, q);
                mix(
                    &mut out,
                    &saw.iter().map(|&x| f.run(x)).collect::<Vec<_>>(),
                    g * 2.0,
                );
            }
        }
        _ => mix(&mut out, &sweep_tone(440.0, 440.0, 1.0, 0.2, 0.3), 0.8),
    }
    // Normalise to -1 dBFS and fade the last millisecond so no sample ends on a click.
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    let g = 0.89 / peak;
    let fade = len(0.002).min(out.len());
    let total = out.len();
    for (i, s) in out.iter_mut().enumerate() {
        *s *= g;
        if i + fade >= total {
            *s *= (total - i) as f32 / fade as f32;
        }
    }
    out
}

/// Mono 16-bit PCM WAV.
fn wav16(samples: &[f32]) -> Vec<u8> {
    let data = samples.len() as u32 * 2;
    let mut w = Vec::with_capacity(44 + data as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&RATE.to_le_bytes());
    w.extend_from_slice(&(RATE * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        w.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_set_matches_the_mockup() {
        let set = starter_set();
        assert_eq!(set.banks[0].name, "KITS 808");
        assert_eq!(set.banks[0].pads.len(), 39);
        assert_eq!(
            set.banks[0].pad(0, 1).unwrap().sample,
            "samples/kick_808.wav"
        );
        assert_eq!(set.banks[0].pad(0, 3).unwrap().choke_group, Some(1));
        assert_eq!(set.mixer.groups[0].name, "KICK");
        assert_eq!(set.pattern.lanes.len(), 5);
        assert_eq!(set.pattern.lanes[0].steps[0], 2);
        assert_eq!(set.pattern.lanes[0].steps[16], 2);
        crate::seq::validate(&mut set.pattern.clone()).unwrap();
        Set::from_json(&set.to_json()).unwrap();
    }

    #[test]
    fn every_starter_sample_decodes() {
        let tmp = tempfile::tempdir().unwrap();
        let set = starter_set();
        write_starter_samples(tmp.path(), &set).unwrap();
        let (ok, failed) = super::super::load_samples(tmp.path(), &set);
        assert!(failed.is_empty(), "{failed:?}");
        assert!(ok.len() >= 30);
        assert!(ok.values().all(|s| !s.data.is_empty()));
    }
}
