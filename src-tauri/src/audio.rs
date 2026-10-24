//! CPAL output stream driving lpd-core Engine, and the thread that owns it.

use crate::config::AudioPrefs;
use crate::logfile;
use crate::state::AppState;
use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use lpd_core::audio::{host, Command, Engine, Event, Taps, Trash};
use rtrb::{Consumer, Producer, RingBuffer};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Time from the MIDI event to the first sample of the buffer (one buffer), the output
/// pipeline's own delay as the driver reports it, and their sum. Milliseconds, one decimal.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Latency {
    pub midi_to_buffer_ms: f32,
    pub output_ms: f32,
    pub total_ms: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AudioStatus {
    /// "running" | "stopped" | "lost" | "error"
    pub state: String,
    /// "WASAPI" | "ASIO"
    pub backend: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub buffer_size: Option<u32>,
    pub channels: u16,
    /// Callback gaps longer than 1.8 buffers since the stream started or the counter was reset.
    /// Filled from `AudioStats` when the status is read.
    pub dropouts: u32,
    pub last_dropout_ago_ms: Option<u64>,
    /// ASIO was asked for but could not be opened; WASAPI runs instead.
    pub asio_fallback: bool,
    /// Filled from `AudioStats` when the status is read.
    pub latency: Latency,
}

impl Default for AudioStatus {
    fn default() -> Self {
        AudioStatus {
            state: "stopped".into(),
            backend: "WASAPI".into(),
            device_name: "none".into(),
            sample_rate: 48_000,
            buffer_size: None,
            channels: 2,
            dropouts: 0,
            last_dropout_ago_ms: None,
            asio_fallback: false,
            latency: Latency::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub name: String,
    pub is_default: bool,
    pub buffer_sizes: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AsioInfo {
    pub available: bool,
    /// "Built without ASIO support" | "No ASIO driver found"
    pub reason: Option<String>,
}

/// Powers of two in 32..=2048 that lie within `[min, max]`.
pub fn buffer_choices(min: u32, max: u32) -> Vec<u32> {
    (5..=11)
        .map(|p| 1u32 << p)
        .filter(|n| (min..=max).contains(n))
        .collect()
}

/// `frames / rate` in milliseconds, rounded to one decimal.
pub fn latency_ms(frames: u32, rate: u32) -> f32 {
    if rate == 0 {
        return 0.0;
    }
    round1(frames as f64 / rate as f64 * 1000.0)
}

fn round1(ms: f64) -> f32 {
    ((ms * 10.0).round() / 10.0) as f32
}

/// The latency readout: one buffer, plus the driver-reported output delay.
pub fn latency(buffer_frames: u32, rate: u32, output_us: u64) -> Latency {
    let midi_to_buffer_ms = latency_ms(buffer_frames, rate);
    let output_ms = round1(output_us as f64 / 1000.0);
    Latency {
        midi_to_buffer_ms,
        output_ms,
        total_ms: round1(midi_to_buffer_ms as f64 + output_ms as f64),
    }
}

/// A callback that arrives later than 1.8 buffer durations after the previous one means the
/// device ran dry.
pub fn is_dropout(interval: Duration, frames: usize, rate: u32) -> bool {
    if rate == 0 {
        return false;
    }
    interval.as_secs_f64() > 1.8 * frames as f64 / rate as f64
}

/// Counters shared between the audio callbacks (writers, atomics only) and the app. One per
/// stream: a late callback or error from a replaced stream only touches its own instance.
pub struct AudioStats {
    started: Instant,
    dropouts: AtomicU32,
    /// Milliseconds since `started` at the last dropout.
    last_dropout_ms: AtomicU64,
    /// Frames of the latest output callback (the buffer size when the driver picks it).
    frames: AtomicU32,
    /// Driver-reported delay between the callback and playback of its buffer, microseconds.
    output_us: AtomicU64,
    /// The stream's error callback fired (device unplugged, driver reset).
    lost: AtomicBool,
}

impl Default for AudioStats {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioStats {
    pub fn new() -> Self {
        AudioStats {
            started: Instant::now(),
            dropouts: AtomicU32::new(0),
            last_dropout_ms: AtomicU64::new(0),
            frames: AtomicU32::new(0),
            output_us: AtomicU64::new(0),
            lost: AtomicBool::new(false),
        }
    }

    /// Safe on the audio thread: two atomic stores, no allocation.
    pub fn record_dropout(&self, now: Instant) {
        let ms = now.saturating_duration_since(self.started).as_millis() as u64;
        self.last_dropout_ms.store(ms, Ordering::Release);
        self.dropouts.fetch_add(1, Ordering::Release);
    }

    pub fn dropouts(&self) -> u32 {
        self.dropouts.load(Ordering::Acquire)
    }

    /// `None` until a dropout has happened (and again after a reset).
    pub fn last_dropout_ago_ms(&self, now: Instant) -> Option<u64> {
        if self.dropouts() == 0 {
            return None;
        }
        let at = self.last_dropout_ms.load(Ordering::Acquire);
        let since_start = now.saturating_duration_since(self.started).as_millis() as u64;
        Some(since_start.saturating_sub(at))
    }

    pub fn reset(&self) {
        self.dropouts.store(0, Ordering::Release);
    }

    /// Atomic stores only; called from the output callback.
    pub fn set_callback_info(&self, frames: usize, output: Option<Duration>) {
        self.frames.store(frames as u32, Ordering::Relaxed);
        if let Some(d) = output {
            self.output_us
                .store(d.as_micros() as u64, Ordering::Relaxed);
        }
    }

    pub fn frames(&self) -> u32 {
        self.frames.load(Ordering::Relaxed)
    }

    pub fn output_us(&self) -> u64 {
        self.output_us.load(Ordering::Relaxed)
    }

    /// Called from the stream's error callback: only sets the flag (no allocation, logging or
    /// I/O there); the telemetry thread picks it up and logs.
    pub fn report_lost(&self) {
        self.lost.store(true, Ordering::Release);
    }

    /// True once per failure burst.
    pub fn take_lost(&self) -> bool {
        self.lost.swap(false, Ordering::AcqRel)
    }
}

/// Everything `start` made. `stream` is `None` only for fakes in tests.
pub struct AudioSystem {
    pub stream: Option<cpal::Stream>,
    pub commands: Producer<Command>,
    pub events: Consumer<Event>,
    pub status: AudioStatus,
    pub taps: Taps,
    pub stats: Arc<AudioStats>,
    /// Instruments the engine replaced come back here, to be dropped off the audio thread.
    pub trash: Option<Consumer<Trash>>,
}

fn is_asio(backend: &str) -> bool {
    backend.eq_ignore_ascii_case("asio")
}

fn host_for(backend: &str) -> Result<cpal::Host> {
    if is_asio(backend) {
        asio_host()
    } else {
        Ok(cpal::default_host())
    }
}

/// Open the output stream the prefs ask for. ASIO that cannot be opened falls back to WASAPI
/// (`asio_fallback`); a device that is not there falls back to the host's default device.
pub fn start(prefs: &AudioPrefs) -> Result<AudioSystem> {
    let mut asio_fallback = false;
    let (audio_host, asio) = if is_asio(&prefs.backend) {
        match asio_host() {
            Ok(h) => (h, true),
            Err(e) => {
                logfile::log(format!("ASIO unavailable, using WASAPI: {e:#}"));
                asio_fallback = true;
                (cpal::default_host(), false)
            }
        }
    } else {
        (cpal::default_host(), false)
    };
    let wanted = prefs.device.as_deref().filter(|_| !asio_fallback);
    let device = wanted
        .and_then(|name| {
            audio_host
                .output_devices()
                .ok()?
                .find(|d| d.name().is_ok_and(|n| n == name))
        })
        .or_else(|| {
            if let Some(name) = wanted {
                logfile::log(format!(
                    "Audio device {name:?} not found, using the default"
                ));
            }
            audio_host.default_output_device()
        })
        .context("no audio output device")?;
    let device_name = device.name().unwrap_or_else(|_| "unknown".into());
    let supported = device.default_output_config()?;
    let format = supported.sample_format();
    let channels = supported.channels();
    let mut config: cpal::StreamConfig = supported.into();
    if let Some(frames) = prefs.buffer_frames {
        config.buffer_size = cpal::BufferSize::Fixed(frames);
    }
    let sample_rate = config.sample_rate.0;
    let (mut engine, events, taps) = Engine::with_taps(sample_rate);
    let trash = engine.take_trash();
    let (commands, rx) = RingBuffer::new(1024);
    let stats = Arc::new(AudioStats::new());
    let cb = Callback {
        engine,
        rx,
        stats: stats.clone(),
        rate: sample_rate,
    };
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, cb)?,
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, cb)?,
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, cb)?,
        other => bail!("unsupported device sample format {other:?}"),
    };
    stream.play()?;
    let buffer_size = match config.buffer_size {
        cpal::BufferSize::Fixed(n) => Some(n),
        cpal::BufferSize::Default => None,
    };
    Ok(AudioSystem {
        stream: Some(stream),
        commands,
        events,
        status: AudioStatus {
            state: "running".into(),
            backend: if asio { "ASIO" } else { "WASAPI" }.into(),
            device_name,
            sample_rate,
            buffer_size,
            channels,
            asio_fallback,
            ..Default::default()
        },
        taps,
        stats,
        trash,
    })
}

/// Output devices of a backend with the buffer sizes each supports.
pub fn list_devices(backend: &str) -> Result<Vec<AudioDevice>> {
    let audio_host = host_for(backend)?;
    let default = audio_host
        .default_output_device()
        .and_then(|d| d.name().ok());
    let mut out = Vec::new();
    for device in audio_host.output_devices()? {
        let Ok(name) = device.name() else { continue };
        let buffer_sizes = match device.default_output_config() {
            Ok(c) => match c.buffer_size() {
                cpal::SupportedBufferSize::Range { min, max } => buffer_choices(*min, *max),
                cpal::SupportedBufferSize::Unknown => vec![],
            },
            Err(_) => vec![],
        };
        out.push(AudioDevice {
            is_default: default.as_deref() == Some(name.as_str()),
            name,
            buffer_sizes,
        });
    }
    Ok(out)
}

#[cfg(feature = "asio")]
pub fn asio_info() -> AsioInfo {
    let found = asio_host()
        .ok()
        .and_then(|h| h.output_devices().ok().map(|mut d| d.next().is_some()))
        .unwrap_or(false);
    AsioInfo {
        available: found,
        reason: (!found).then(|| "No ASIO driver found".into()),
    }
}

#[cfg(not(feature = "asio"))]
pub fn asio_info() -> AsioInfo {
    AsioInfo {
        available: false,
        reason: Some("Built without ASIO support".into()),
    }
}

/// Everything the output callback owns.
struct Callback {
    engine: Engine,
    rx: Consumer<Command>,
    stats: Arc<AudioStats>,
    rate: u32,
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    cb: Callback,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let Callback {
        mut engine,
        mut rx,
        stats,
        rate,
    } = cb;
    let channels = config.channels as usize;
    let mut scratch = host::scratch();
    let mut staging = vec![0.0f32; 4096 * channels];
    // Time and frame count of the previous callback: a gap is judged against the buffer it followed.
    let mut last_callback: Option<(Instant, usize)> = None;
    let on_error = stats.clone();
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
            let now = Instant::now();
            let frames = data.len() / channels.max(1);
            if let Some((prev, prev_frames)) = last_callback {
                if is_dropout(now.duration_since(prev), prev_frames, rate) {
                    stats.record_dropout(now);
                }
            }
            last_callback = Some((now, frames));
            let ts = info.timestamp();
            stats.set_callback_info(frames, ts.playback.duration_since(&ts.callback));
            for chunk in data.chunks_mut(staging.len()) {
                let buf = &mut staging[..chunk.len()];
                host::process(&mut engine, &mut rx, &mut scratch, buf, channels);
                for (d, s) in chunk.iter_mut().zip(buf.iter()) {
                    *d = T::from_sample(*s);
                }
            }
        },
        move |_| on_error.report_lost(),
        None,
    )?;
    Ok(stream)
}

// ---- the thread that owns the stream ----------------------------------------------------------

pub enum AudioRequest {
    /// Replace the running stream (if any) with one for these prefs. The reply is optional: a
    /// boot start does not wait.
    Start(
        AudioPrefs,
        Option<mpsc::Sender<Result<AudioStatus, String>>>,
    ),
}

/// Builds a stream for the prefs. Runs on the owner thread only (a `cpal::Stream` is not `Send`).
pub type Starter = Box<dyn FnMut(&AudioPrefs) -> Result<AudioSystem> + Send>;

/// Told the status every time a start has finished, whether or not anyone still waits for the
/// reply (the app emits `audio-status` from it).
pub type Announce = Box<dyn Fn(AudioStatus) + Send>;

/// How long a command waits for the owner thread.
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// Handle to the audio owner thread. The thread owns the `cpal::Stream`; every restart goes
/// through it, never through the router or UI threads.
#[derive(Clone)]
pub struct AudioController {
    tx: mpsc::Sender<AudioRequest>,
}

/// Start with the prefs; a fixed buffer the device rejects is retried once with AUTO, so a bad
/// saved buffer size cannot leave the app without sound.
fn start_with_fallback(starter: &mut Starter, prefs: &AudioPrefs) -> Result<AudioSystem> {
    match starter(prefs) {
        Err(e) if prefs.buffer_frames.is_some() => {
            logfile::log(format!(
                "Audio start with buffer {:?} failed ({e:#}); retrying with AUTO",
                prefs.buffer_frames
            ));
            starter(&AudioPrefs {
                buffer_frames: None,
                ..prefs.clone()
            })
        }
        other => other,
    }
}

fn panic_text(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown".into())
}

impl AudioController {
    /// Spawn the owner thread with the real cpal starter.
    pub fn spawn(state: AppState, announce: Announce) -> Self {
        Self::spawn_with(state, Box::new(start), announce)
    }

    pub fn spawn_with(state: AppState, mut starter: Starter, announce: Announce) -> Self {
        let (tx, rx) = mpsc::channel::<AudioRequest>();
        std::thread::Builder::new()
            .name("audio-owner".into())
            .spawn(move || {
                let mut stream: Option<cpal::Stream> = None;
                for request in rx {
                    let AudioRequest::Start(prefs, reply) = request;
                    // The old stream goes first: ASIO devices cannot be opened twice.
                    drop(stream.take());
                    // A panic in a driver call becomes an error status; the thread lives on.
                    let work = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        match start_with_fallback(&mut starter, &prefs) {
                            Ok(sys) => {
                                state.install_trash(sys.trash);
                                let status = state.install_audio(
                                    sys.commands,
                                    sys.events,
                                    sys.taps,
                                    sys.stats,
                                    sys.status,
                                );
                                Ok((sys.stream, status))
                            }
                            Err(e) => Err(format!("{e:#}")),
                        }
                    }));
                    let result = match work {
                        Ok(Ok((new_stream, status))) => {
                            stream = new_stream;
                            logfile::log(format!("Audio: {status:?}"));
                            Ok(status)
                        }
                        Ok(Err(message)) => Err(message),
                        Err(p) => Err(format!("The audio driver panicked: {}", panic_text(&*p))),
                    };
                    if let Err(message) = &result {
                        logfile::log(format!("Audio start failed: {message}"));
                        state.audio_failed(&prefs);
                    }
                    announce(state.get_audio_status());
                    if let Some(reply) = reply {
                        let _ = reply.send(result);
                    }
                }
            })
            .expect("spawn the audio owner thread");
        AudioController { tx }
    }

    /// Restart the stream with `prefs`. Blocks up to 5 s: call from a worker, not the UI. After
    /// a timeout the thread still finishes the start and announces the result.
    pub fn start(&self, prefs: AudioPrefs) -> Result<AudioStatus, String> {
        let (reply, wait) = mpsc::channel();
        self.tx
            .send(AudioRequest::Start(prefs, Some(reply)))
            .map_err(|_| "The audio thread is not running".to_string())?;
        wait.recv_timeout(REPLY_TIMEOUT)
            .map_err(|_| "The audio device did not answer in time".to_string())?
    }

    /// Start without waiting (app launch): the result arrives as an announcement.
    pub fn start_detached(&self, prefs: AudioPrefs) {
        let _ = self.tx.send(AudioRequest::Start(prefs, None));
    }

    /// `set_audio`: remember the prefs, then restart with them.
    pub fn apply(&self, state: &AppState, prefs: AudioPrefs) -> Result<AudioStatus, String> {
        state.set_audio_prefs(prefs.clone())?;
        self.start(prefs)
    }

    /// `audio_reconnect`: restart with the saved prefs, only when the stream is lost, stopped or
    /// failed to start; a running stream is left alone.
    pub fn reconnect(&self, state: &AppState) -> Result<AudioStatus, String> {
        let status = state.get_audio_status();
        if matches!(status.state.as_str(), "lost" | "stopped" | "error") {
            self.start(state.get_config().audio)
        } else {
            Ok(status)
        }
    }
}

#[cfg(feature = "asio")]
fn asio_host() -> Result<cpal::Host> {
    cpal::host_from_id(cpal::HostId::Asio)
        .map_err(|e| anyhow::anyhow!("ASIO host unavailable: {e}"))
}

#[cfg(not(feature = "asio"))]
fn asio_host() -> Result<cpal::Host> {
    bail!("built without ASIO support; rebuild with `--features asio`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropout_threshold_is_one_point_eight_buffers() {
        // 256 frames at 48 kHz = 5.33 ms; threshold 9.6 ms.
        assert!(is_dropout(Duration::from_millis(12), 256, 48_000));
        assert!(!is_dropout(Duration::from_millis(6), 256, 48_000));
        assert!(!is_dropout(Duration::from_millis(9), 256, 48_000));
        assert!(!is_dropout(Duration::from_millis(100), 256, 0));
    }

    #[test]
    fn buffer_choices_are_powers_of_two_within_the_range() {
        assert_eq!(buffer_choices(64, 1024), vec![64, 128, 256, 512, 1024]);
        assert_eq!(
            buffer_choices(1, 4096),
            vec![32, 64, 128, 256, 512, 1024, 2048]
        );
        assert_eq!(buffer_choices(100, 120), Vec::<u32>::new());
        assert_eq!(buffer_choices(32, 32), vec![32]);
    }

    #[test]
    fn latency_is_frames_over_rate_in_ms_with_one_decimal() {
        assert_eq!(latency_ms(256, 48_000), 5.3);
        assert_eq!(latency_ms(128, 48_000), 2.7);
        assert_eq!(latency_ms(256, 0), 0.0);
        let l = latency(256, 48_000, 10_000);
        assert_eq!(
            l,
            Latency {
                midi_to_buffer_ms: 5.3,
                output_ms: 10.0,
                total_ms: 15.3
            }
        );
    }

    #[test]
    fn status_serializes_with_the_documented_names() {
        let json = serde_json::to_value(AudioStatus::default()).unwrap();
        for key in [
            "state",
            "backend",
            "deviceName",
            "sampleRate",
            "bufferSize",
            "channels",
            "dropouts",
            "lastDropoutAgoMs",
            "asioFallback",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert!(json["latency"].get("midiToBufferMs").is_some());
        assert!(json["latency"].get("outputMs").is_some());
        assert!(json["latency"].get("totalMs").is_some());
    }

    #[test]
    fn a_stream_error_flag_is_taken_once_and_belongs_to_one_stream() {
        let old = AudioStats::new();
        let new = AudioStats::new();
        for _ in 0..50 {
            old.report_lost();
        }
        assert!(old.take_lost());
        assert!(!old.take_lost());
        old.report_lost();
        assert!(!new.take_lost());
    }
}
