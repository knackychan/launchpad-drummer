//! Spectrum analysis for the telemetry display. Runs on the telemetry thread, never on the
//! audio thread. Everything is allocated in `Fft::new`.

use std::f32::consts::PI;

pub const FFT_SIZE: usize = 2048;
pub const BANDS: usize = 28;
const BAND_LOW_HZ: f32 = 40.0;
const BAND_HIGH_HZ: f32 = 16_000.0;
const FLOOR_DB: f32 = -100.0;

pub struct Fft {
    window: Vec<f32>,
    cos: Vec<f32>,
    sin: Vec<f32>,
    rev: Vec<u16>,
    re: Vec<f32>,
    im: Vec<f32>,
    mags: [f32; FFT_SIZE / 2],
}

impl Default for Fft {
    fn default() -> Self {
        Self::new()
    }
}

impl Fft {
    pub fn new() -> Self {
        let n = FFT_SIZE;
        let bits = n.trailing_zeros();
        Fft {
            window: (0..n)
                .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / n as f32).cos())
                .collect(),
            cos: (0..n / 2)
                .map(|k| (2.0 * PI * k as f32 / n as f32).cos())
                .collect(),
            sin: (0..n / 2)
                .map(|k| -(2.0 * PI * k as f32 / n as f32).sin())
                .collect(),
            rev: (0..n)
                .map(|i| (i.reverse_bits() >> (usize::BITS - bits)) as u16)
                .collect(),
            re: vec![0.0; n],
            im: vec![0.0; n],
            mags: [0.0; FFT_SIZE / 2],
        }
    }

    /// Hann-windowed magnitude of each bin, scaled by 1 / (N / 4) so a full-scale sine on a bin
    /// reads about 1.0.
    pub fn magnitudes(&mut self, input: &[f32; FFT_SIZE]) -> &[f32; FFT_SIZE / 2] {
        let n = FFT_SIZE;
        for i in 0..n {
            let j = self.rev[i] as usize;
            self.re[j] = input[i] * self.window[i];
            self.im[j] = 0.0;
        }
        let mut len = 2;
        while len <= n {
            let half = len / 2;
            let step = n / len;
            for start in (0..n).step_by(len) {
                for k in 0..half {
                    let (wr, wi) = (self.cos[k * step], self.sin[k * step]);
                    let (a, b) = (start + k, start + k + half);
                    let tr = self.re[b] * wr - self.im[b] * wi;
                    let ti = self.re[b] * wi + self.im[b] * wr;
                    self.re[b] = self.re[a] - tr;
                    self.im[b] = self.im[a] - ti;
                    self.re[a] += tr;
                    self.im[a] += ti;
                }
            }
            len *= 2;
        }
        let scale = 4.0 / n as f32;
        for (k, m) in self.mags.iter_mut().enumerate() {
            *m = (self.re[k] * self.re[k] + self.im[k] * self.im[k]).sqrt() * scale;
        }
        &self.mags
    }
}

/// 28 log-spaced bands from 40 Hz to 16 kHz; each band = max magnitude of its bins in dBFS (floor
/// −100); a band narrower than one bin takes the nearest bin. Second value: frequency of the
/// loudest bin overall (0 when silent). The DC bin is ignored for the peak.
pub fn bands(mags: &[f32; FFT_SIZE / 2], sample_rate: u32) -> ([f32; BANDS], f32) {
    let bin_hz = sample_rate as f32 / FFT_SIZE as f32;
    let last = mags.len() - 1;
    let edge = |i: usize| BAND_LOW_HZ * (BAND_HIGH_HZ / BAND_LOW_HZ).powf(i as f32 / BANDS as f32);
    let mut out = [FLOOR_DB; BANDS];
    for (i, band) in out.iter_mut().enumerate() {
        let (lo_hz, hi_hz) = (edge(i), edge(i + 1));
        let lo = (lo_hz / bin_hz).ceil() as usize;
        let hi = ((hi_hz / bin_hz).ceil() as usize).min(last + 1);
        let peak = if lo < hi {
            mags[lo.min(last)..hi].iter().cloned().fold(0.0, f32::max)
        } else {
            let centre = (lo_hz * hi_hz).sqrt();
            mags[((centre / bin_hz).round() as usize).min(last)]
        };
        *band = to_dbfs(peak);
    }
    let mut top = 0.0;
    let mut top_bin = 0;
    for (k, &m) in mags.iter().enumerate().skip(1) {
        if m > top {
            top = m;
            top_bin = k;
        }
    }
    (out, top_bin as f32 * bin_hz)
}

/// First index i in 1..buf.len()/2 with buf[i-1] < 0 <= buf[i]; 0 if none. The upper half is
/// left alone so a window of buf.len()/2 samples always fits after the trigger.
pub fn trigger_index(buf: &[f32]) -> usize {
    (1..buf.len() / 2)
        .find(|&i| buf[i - 1] < 0.0 && buf[i] >= 0.0)
        .unwrap_or(0)
}

pub fn to_dbfs(x: f32) -> f32 {
    if x <= 0.0 {
        FLOOR_DB
    } else {
        (20.0 * x.log10()).max(FLOOR_DB)
    }
}
