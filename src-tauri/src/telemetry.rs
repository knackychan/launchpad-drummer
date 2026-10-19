//! The telemetry thread: drains the engine taps every 16 ms, runs the FFT and emits `telemetry`
//! while the UI wants it. Nothing here runs on the audio thread.

use crate::audio::AudioStatus;
use crate::logfile;
use crate::state::AppState;
use lpd_core::audio::fft::{bands, to_dbfs, trigger_index, Fft, FFT_SIZE};
use lpd_core::audio::GROUPS;
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub const PERIOD: Duration = Duration::from_millis(16);
/// Samples of the scope trace sent per frame.
pub const SCOPE_POINTS: usize = FFT_SIZE / 2;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryFrame {
    pub scope: Vec<f32>,
    pub bands: Vec<f32>,
    pub peak_hz: f32,
    /// dBFS, left and right.
    pub master: [f32; 2],
    /// Linear peaks 0..1.
    pub groups: Vec<f32>,
}

pub struct Tick {
    /// Present only while the UI asked for the feed.
    pub frame: Option<TelemetryFrame>,
    /// The dropout count differs from the previous tick: `audio-status` should be emitted.
    pub dropouts_changed: bool,
    /// The output stream failed since the previous tick: the status, now with `state: "lost"`.
    pub lost: Option<AudioStatus>,
}

pub struct Telemetry {
    fft: Fft,
    /// Circular buffer of the latest output samples; `head` is the oldest.
    ring: Box<[f32; FFT_SIZE]>,
    head: usize,
    ordered: Box<[f32; FFT_SIZE]>,
    master_peak: [f32; 2],
    group_peak: [f32; GROUPS],
    last_dropouts: u32,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl Telemetry {
    pub fn new() -> Self {
        Telemetry {
            fft: Fft::new(),
            ring: Box::new([0.0; FFT_SIZE]),
            head: 0,
            ordered: Box::new([0.0; FFT_SIZE]),
            master_peak: [0.0; 2],
            group_peak: [0.0; GROUPS],
            last_dropouts: 0,
        }
    }

    /// One 16 ms step: drain the taps, then build a frame when the feed is on.
    pub fn tick(&mut self, state: &AppState) -> Tick {
        let on = state.telemetry_enabled();
        let have_taps = {
            let mut guard = state.taps.lock().unwrap_or_else(|e| e.into_inner());
            match guard.as_mut() {
                Some(taps) => {
                    while let Ok(s) = taps.scope.pop() {
                        self.ring[self.head] = s;
                        self.head = (self.head + 1) % FFT_SIZE;
                    }
                    while let Ok(l) = taps.levels.pop() {
                        for c in 0..2 {
                            self.master_peak[c] = self.master_peak[c].max(l.master[c]);
                        }
                        for (peak, g) in self.group_peak.iter_mut().zip(l.groups) {
                            *peak = peak.max(g);
                        }
                    }
                    true
                }
                None => {
                    // No stream: forget the old trace so it cannot come back frozen.
                    self.ring.fill(0.0);
                    self.head = 0;
                    false
                }
            }
        };
        let frame = (on && have_taps).then(|| self.frame(state.sample_rate()));
        self.master_peak = [0.0; 2];
        self.group_peak = [0.0; GROUPS];

        let dropouts = state.dropout_count();
        let dropouts_changed = dropouts != self.last_dropouts;
        self.last_dropouts = dropouts;
        Tick {
            frame,
            dropouts_changed,
            lost: state.poll_stream_lost(),
        }
    }

    fn frame(&mut self, sample_rate: u32) -> TelemetryFrame {
        for i in 0..FFT_SIZE {
            self.ordered[i] = self.ring[(self.head + i) % FFT_SIZE];
        }
        let start = trigger_index(&self.ordered[..]);
        let scope = self.ordered[start..start + SCOPE_POINTS]
            .iter()
            .map(|s| (s * 1000.0).round() / 1000.0)
            .collect();
        let mags = *self.fft.magnitudes(&self.ordered);
        let (bands, peak_hz) = bands(&mags, sample_rate);
        TelemetryFrame {
            scope,
            bands: bands.to_vec(),
            peak_hz,
            master: [to_dbfs(self.master_peak[0]), to_dbfs(self.master_peak[1])],
            groups: self.group_peak.to_vec(),
        }
    }
}

/// Thread body, spawned in `setup()`.
pub fn run(app: AppHandle, state: AppState) {
    let mut telemetry = Telemetry::new();
    loop {
        std::thread::sleep(PERIOD);
        let tick = telemetry.tick(&state);
        if let Some(frame) = tick.frame {
            let _ = app.emit("telemetry", frame);
        }
        if let Some(status) = tick.lost {
            logfile::log("audio stream lost: the output device stopped answering");
            let _ = app.emit("audio-status", status);
        } else if tick.dropouts_changed {
            let _ = app.emit("audio-status", state.get_audio_status());
        }
    }
}
