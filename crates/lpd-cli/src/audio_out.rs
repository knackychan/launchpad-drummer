//! cpal output stream driving the lpd-core engine (WASAPI by default, ASIO with --asio).

use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use lpd_core::audio::{host, Command, Engine};

pub struct AudioOut {
    pub stream: cpal::Stream,
    pub commands: rtrb::Producer<Command>,
    pub device_name: String,
    pub sample_rate: u32,
    pub buffer: cpal::BufferSize,
}

pub fn start(asio: bool, buffer_frames: Option<u32>) -> Result<AudioOut> {
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
    let mut config: cpal::StreamConfig = supported.into();
    if let Some(frames) = buffer_frames {
        config.buffer_size = cpal::BufferSize::Fixed(frames);
    }
    let sample_rate = config.sample_rate.0;
    let (engine, _events) = Engine::new(sample_rate);
    let (commands, rx) = rtrb::RingBuffer::new(1024);
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, engine, rx)?,
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, engine, rx)?,
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, engine, rx)?,
        other => bail!("unsupported device sample format {other:?}"),
    };
    stream.play()?;
    Ok(AudioOut {
        stream,
        commands,
        device_name,
        sample_rate,
        buffer: config.buffer_size,
    })
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut engine: Engine,
    mut rx: rtrb::Consumer<Command>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut scratch = host::scratch();
    // f32 staging buffer; a whole number of frames.
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
        |err| eprintln!("audio stream error: {err}"),
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
