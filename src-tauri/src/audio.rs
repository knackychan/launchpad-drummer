//! CPAL output stream driving lpd-core Engine.

use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use lpd_core::audio::{host, Command, Engine, Event};
use rtrb::{Consumer, Producer, RingBuffer};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatus {
    pub state: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub buffer_size: Option<u32>,
    pub channels: u16,
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
    let (engine, events) = Engine::new(sample_rate);
    let (commands, rx) = RingBuffer::new(1024);
    let health = Arc::new(StreamHealth::default());
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, engine, rx, health.clone())?,
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, engine, rx, health.clone())?,
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, engine, rx, health.clone())?,
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
        },
    })
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut engine: Engine,
    mut rx: Consumer<Command>,
    health: Arc<StreamHealth>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut scratch = host::scratch();
    let mut staging = vec![0.0f32; 4096 * channels];
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
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
