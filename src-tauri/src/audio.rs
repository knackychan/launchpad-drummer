//! CPAL output stream driving lpd-core Engine.

use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use lpd_core::audio::{host, Command, Engine, Event, Taps};
use rtrb::{Consumer, Producer, RingBuffer};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatus {
    pub state: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub buffer_size: Option<u32>,
    pub channels: u16,
    /// Callback gaps longer than 1.8 buffers since the stream started or the counter was reset.
    /// Filled from `AudioStats` when the status is read.
    #[serde(default)]
    pub dropouts: u32,
    #[serde(default)]
    pub last_dropout_ago_ms: Option<u64>,
}

/// A callback that arrives later than 1.8 buffer durations after the previous one means the
/// device ran dry.
pub fn is_dropout(interval: Duration, frames: usize, rate: u32) -> bool {
    if rate == 0 {
        return false;
    }
    interval.as_secs_f64() > 1.8 * frames as f64 / rate as f64
}

/// Dropout counter shared between the audio callback (writer, atomics only) and the app.
pub struct AudioStats {
    started: Instant,
    dropouts: AtomicU32,
    /// Milliseconds since `started` at the last dropout.
    last_dropout_ms: AtomicU64,
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
        }
    }

    /// Safe on the audio thread: two atomic stores, no allocation.
    pub fn record_dropout(&self, now: Instant) {
        let ms = now.saturating_duration_since(self.started).as_millis() as u64;
        self.last_dropout_ms.store(ms, Ordering::Relaxed);
        self.dropouts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dropouts(&self) -> u32 {
        self.dropouts.load(Ordering::Relaxed)
    }

    /// `None` until a dropout has happened (and again after a reset).
    pub fn last_dropout_ago_ms(&self, now: Instant) -> Option<u64> {
        if self.dropouts() == 0 {
            return None;
        }
        let at = self.last_dropout_ms.load(Ordering::Relaxed);
        let since_start = now.saturating_duration_since(self.started).as_millis() as u64;
        Some(since_start.saturating_sub(at))
    }

    pub fn reset(&self) {
        self.dropouts.store(0, Ordering::Relaxed);
    }
}

/// Failure flag of one output stream. The stream's error callback only sets it (no allocation,
/// logging or I/O); the audio owner polls it. A replaced stream gets a new flag, so a late error
/// from the old stream cannot affect the new one.
#[derive(Default)]
pub struct StreamHealth(AtomicBool);

impl StreamHealth {
    pub fn report_failure(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// True once per failure burst.
    pub fn take_failure(&self) -> bool {
        self.0.swap(false, Ordering::AcqRel)
    }
}

/// Move a reported stream failure into the application status. Returns the new status when the
/// UI should be told (the first failure only).
pub fn poll_stream_health(
    health: &StreamHealth,
    state: &crate::state::AppState,
) -> Option<AudioStatus> {
    if health.take_failure() {
        state.audio_stream_failed()
    } else {
        None
    }
}

pub struct AudioSystem {
    pub stream: cpal::Stream,
    pub health: Arc<StreamHealth>,
    pub commands: Producer<Command>,
    pub events: Consumer<Event>,
    pub status: AudioStatus,
    pub taps: Taps,
    pub stats: Arc<AudioStats>,
}

pub fn start(asio: bool, buffer_frames: Option<u32>) -> Result<AudioSystem> {
    let audio_host = if asio {
        asio_host()?
    } else {
        cpal::default_host()
    };
    let device = audio_host
        .default_output_device()
        .context("no audio output device")?;
    let device_name = device.name().unwrap_or_else(|_| "unknown".into());
    let supported = device.default_output_config()?;
    let format = supported.sample_format();
    let channels = supported.channels();
    let mut config: cpal::StreamConfig = supported.into();
    if let Some(frames) = buffer_frames {
        config.buffer_size = cpal::BufferSize::Fixed(frames);
    }
    let sample_rate = config.sample_rate.0;
    let (engine, events, taps) = Engine::with_taps(sample_rate);
    let (commands, rx) = RingBuffer::new(1024);
    let health = Arc::new(StreamHealth::default());
    let stats = Arc::new(AudioStats::new());
    let cb = Callback {
        engine,
        rx,
        health: health.clone(),
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
        stream,
        health,
        commands,
        events,
        status: AudioStatus {
            state: "running".into(),
            device_name,
            sample_rate,
            buffer_size,
            channels,
            dropouts: 0,
            last_dropout_ago_ms: None,
        },
        taps,
        stats,
    })
}

/// Everything the output callback owns.
struct Callback {
    engine: Engine,
    rx: Consumer<Command>,
    health: Arc<StreamHealth>,
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
        health,
        stats,
        rate,
    } = cb;
    let channels = config.channels as usize;
    let mut scratch = host::scratch();
    let mut staging = vec![0.0f32; 4096 * channels];
    let mut last_callback: Option<Instant> = None;
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            let now = Instant::now();
            if let Some(prev) = last_callback {
                if is_dropout(now.duration_since(prev), data.len() / channels.max(1), rate) {
                    stats.record_dropout(now);
                }
            }
            last_callback = Some(now);
            for chunk in data.chunks_mut(staging.len()) {
                let buf = &mut staging[..chunk.len()];
                host::process(&mut engine, &mut rx, &mut scratch, buf, channels);
                for (d, s) in chunk.iter_mut().zip(buf.iter()) {
                    *d = T::from_sample(*s);
                }
            }
        },
        move |_| health.report_failure(),
        None,
    )?;
    Ok(stream)
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
}
