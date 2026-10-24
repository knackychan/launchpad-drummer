//! Band-limited wavetables (keys synth spec §3.3, plan K15). Built once at startup, read-only
//! afterwards; the audio thread only reads them.

use super::params::TableId;
use crate::audio::fft::fft_complex;
use std::f32::consts::TAU;
use std::sync::OnceLock;

/// Samples of a source cycle (the time-domain generators are sampled at this resolution).
pub const FRAME: usize = 2048;
/// Band-limited copies per frame, one per octave.
pub const LEVELS: usize = 10;
/// Harmonics kept by level 0.
pub const TOP_HARMONIC: usize = 512;
const MIN_LEN: usize = 256;

pub struct Wavetable {
    frames: usize,
    /// Per level: samples per frame, and the frames back to back, each followed by a copy of its
    /// first sample (so interpolation never wraps).
    levels: Vec<(usize, Vec<f32>)>,
}

impl Wavetable {
    /// Harmonics kept by `level`.
    pub fn harmonics(level: usize) -> usize {
        (TOP_HARMONIC >> level).max(1)
    }

    /// Samples per frame at `level`.
    pub fn level_len(level: usize) -> usize {
        (4 * Self::harmonics(level)).max(MIN_LEN)
    }

    pub fn frames(&self) -> usize {
        self.frames
    }

    /// The first level whose top harmonic stays below Nyquist for a fundamental of `hz`.
    pub fn level_for(hz: f32, rate: f32) -> usize {
        let limit = rate / (2.0 * hz.max(1e-3));
        (0..LEVELS)
            .find(|&k| Self::harmonics(k) as f32 <= limit)
            .unwrap_or(LEVELS - 1)
    }

    /// `pos` 0–1 across the frames (neighbours crossfaded), `phase` 0–1 inside a frame (linear).
    #[inline]
    pub fn read(&self, level: usize, pos: f32, phase: f32) -> f32 {
        let (len, data) = &self.levels[level];
        let (len, stride) = (*len, *len + 1);
        let fp = pos.clamp(0.0, 1.0) * (self.frames - 1) as f32;
        let f0 = (fp as usize).min(self.frames - 1);
        let f1 = (f0 + 1).min(self.frames - 1);
        let ff = fp - f0 as f32;
        let x = phase.clamp(0.0, 1.0) * len as f32;
        let i = (x as usize).min(len - 1);
        let fr = x - i as f32;
        let a = f0 * stride + i;
        let b = f1 * stride + i;
        let va = data[a] + (data[a + 1] - data[a]) * fr;
        let vb = data[b] + (data[b + 1] - data[b]) * fr;
        va + (vb - va) * ff
    }

    /// One frame of one level (without the wrap sample).
    pub fn level_data(&self, level: usize, frame: usize) -> &[f32] {
        let (len, data) = &self.levels[level];
        let start = frame * (len + 1);
        &data[start..start + len]
    }

    /// Every frame of level 0, resampled to `points` samples (for the OSC display).
    pub fn display(&self, points: usize) -> Vec<Vec<f32>> {
        (0..self.frames)
            .map(|f| {
                let pos = if self.frames > 1 {
                    f as f32 / (self.frames - 1) as f32
                } else {
                    0.0
                };
                (0..points)
                    .map(|i| self.read(0, pos, i as f32 / points as f32))
                    .collect()
            })
            .collect()
    }

    /// `spectrum(frame, h)` = (cosine, sine) amplitude of harmonic `h` (1-based). Every frame is
    /// scaled so that its level-0 copy peaks at 1.0; the same gain applies to all its levels.
    fn from_spectra(frames: usize, spectrum: impl Fn(usize, usize) -> (f32, f32)) -> Wavetable {
        let mut levels: Vec<(usize, Vec<f32>)> = (0..LEVELS)
            .map(|k| {
                let len = Self::level_len(k);
                (len, vec![0.0; frames * (len + 1)])
            })
            .collect();
        let mut re_buf = vec![0.0f32; FRAME];
        let mut im_buf = vec![0.0f32; FRAME];
        for f in 0..frames {
            let spec: Vec<(f32, f32)> = (1..=TOP_HARMONIC).map(|h| spectrum(f, h)).collect();
            let mut gain = 1.0;
            for (k, (len, data)) in levels.iter_mut().enumerate() {
                let len = *len;
                let (re, im) = (&mut re_buf[..len], &mut im_buf[..len]);
                re.fill(0.0);
                im.fill(0.0);
                let half = len as f32 / 2.0;
                for h in 1..=Self::harmonics(k) {
                    let (c, s) = spec[h - 1];
                    re[h] = c * half;
                    im[h] = -s * half;
                    re[len - h] = c * half;
                    im[len - h] = s * half;
                }
                fft_complex(re, im, true);
                if k == 0 {
                    let peak = re.iter().fold(0.0f32, |m, v| m.max(v.abs()));
                    gain = if peak > 1e-9 { 1.0 / peak } else { 1.0 };
                }
                let base = f * (len + 1);
                for n in 0..len {
                    data[base + n] = re[n] * gain;
                }
                data[base + len] = data[base];
            }
        }
        Wavetable { frames, levels }
    }

    /// `cycle(frame, phase)` sampled at FRAME points and analysed into harmonics (DC dropped).
    fn from_cycles(frames: usize, cycle: impl Fn(usize, f32) -> f32) -> Wavetable {
        let mut spectra = vec![(0.0f32, 0.0f32); frames * TOP_HARMONIC];
        let mut re = vec![0.0f32; FRAME];
        let mut im = vec![0.0f32; FRAME];
        let scale = 2.0 / FRAME as f32;
        for f in 0..frames {
            for (n, v) in re.iter_mut().enumerate() {
                *v = cycle(f, n as f32 / FRAME as f32);
            }
            im.fill(0.0);
            fft_complex(&mut re, &mut im, false);
            for h in 1..=TOP_HARMONIC {
                spectra[f * TOP_HARMONIC + h - 1] = (re[h] * scale, -im[h] * scale);
            }
        }
        Self::from_spectra(frames, |f, h| spectra[f * TOP_HARMONIC + h - 1])
    }
}

/// Formants (Hz, bandwidth Hz, gain) of A, E, I, O, U, applied to a 110 Hz saw spectrum.
const VOWELS: [[(f32, f32, f32); 3]; 5] = [
    [
        (800.0, 80.0, 1.0),
        (1150.0, 90.0, 0.5),
        (2900.0, 120.0, 0.25),
    ],
    [
        (350.0, 80.0, 1.0),
        (2000.0, 90.0, 0.5),
        (2800.0, 120.0, 0.25),
    ],
    [
        (270.0, 80.0, 1.0),
        (2140.0, 90.0, 0.5),
        (2950.0, 120.0, 0.25),
    ],
    [
        (450.0, 80.0, 1.0),
        (800.0, 90.0, 0.5),
        (2830.0, 120.0, 0.25),
    ],
    [
        (325.0, 80.0, 1.0),
        (700.0, 90.0, 0.5),
        (2530.0, 120.0, 0.25),
    ],
];

fn vowel_amp(frame: usize, h: usize) -> f32 {
    let hz = 110.0 * h as f32;
    VOWELS[frame]
        .iter()
        .map(|&(fc, bw, g)| g / (1.0 + ((hz - fc) / bw).powi(2)))
        .sum::<f32>()
        / h as f32
}

/// Drawbars as (harmonic, level): 8′, 4′, 2⅔′, 2′, 1⅗′, 1⅓′, 1′.
const DRAWBARS: [(usize, f32); 7] = [
    (1, 1.0),
    (2, 0.6),
    (3, 0.5),
    (4, 0.4),
    (5, 0.3),
    (6, 0.3),
    (8, 0.3),
];

fn organ_amp(frame: usize, h: usize) -> f32 {
    if frame == 7 {
        return if DRAWBARS.iter().any(|&(dh, _)| dh == h) {
            1.0
        } else {
            0.0
        };
    }
    DRAWBARS
        .iter()
        .take(frame + 1)
        .find(|&&(dh, _)| dh == h)
        .map_or(0.0, |&(_, a)| a)
}

fn build(id: TableId) -> Wavetable {
    match id {
        TableId::Basic => Wavetable::from_cycles(4, |f, p| match f {
            0 => (TAU * p).sin(),
            1 => {
                if p < 0.25 {
                    4.0 * p
                } else if p < 0.75 {
                    2.0 - 4.0 * p
                } else {
                    4.0 * p - 4.0
                }
            }
            2 => 2.0 * p - 1.0,
            _ => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
        }),
        TableId::Pwm => Wavetable::from_cycles(32, |f, p| {
            let width = 0.5 - 0.48 * f as f32 / 31.0;
            if p < width {
                1.0
            } else {
                -1.0
            }
        }),
        TableId::Harmonics => Wavetable::from_spectra(32, |f, h| {
            if h <= f + 1 {
                (0.0, 1.0 / h as f32)
            } else {
                (0.0, 0.0)
            }
        }),
        TableId::Vowels => Wavetable::from_spectra(5, |f, h| (0.0, vowel_amp(f, h))),
        TableId::FmSweep => Wavetable::from_cycles(32, |f, p| {
            let index = 10.0 * f as f32 / 31.0;
            (TAU * p + index * (TAU * 2.0 * p).sin()).sin()
        }),
        TableId::SyncSweep => Wavetable::from_cycles(32, |f, p| {
            let ratio = 1.0 + 7.0 * f as f32 / 31.0;
            2.0 * (p * ratio).fract() - 1.0
        }),
        TableId::Organ => Wavetable::from_spectra(8, |f, h| (0.0, organ_amp(f, h))),
        TableId::Digital => Wavetable::from_cycles(16, |f, p| {
            let t = f as f32 / 15.0;
            let bits = (8.0 - 6.0 * t).round();
            let steps = (64.0 * 0.125f32.powf(t)).round();
            let v = 2.0 * (p * steps).floor() / steps - 1.0;
            let q = 2f32.powf(bits - 1.0) - 1.0;
            (v * q).round() / q
        }),
    }
}

/// Every table, in `TableId` order (a fresh build; tests time it).
pub fn build_all() -> Vec<Wavetable> {
    TableId::ALL.iter().map(|&id| build(id)).collect()
}

/// The shared tables, built on first use (call it once early, off the audio thread).
pub fn registry() -> &'static [Wavetable] {
    static REGISTRY: OnceLock<Vec<Wavetable>> = OnceLock::new();
    REGISTRY.get_or_init(build_all)
}

pub fn table(id: TableId) -> &'static Wavetable {
    &registry()[id as usize]
}
