// src/recorder/input.rs

use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use ringbuf::producer::Producer;

/// AudioInput holds the CPAL input stream. The Producers are moved into the input callback.
// src/recorder/input.rs

pub struct AudioInput {
    pub stream: Stream,
    #[allow(dead_code)]
    channels: usize,
    pub sample_rate: u32, // <--- add this
}

impl AudioInput {
    pub fn new<PRec, PMon>(producer_rec: PRec, producer_mon: PMon)
        -> Result<(Self, usize, u32)>            // <--- return sample_rate too
    where
        PRec: Producer<Item = f32> + Send + 'static,
        PMon: Producer<Item = f32> + Send + 'static,
    {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device available"))?;

        let supported_config = device.default_input_config()?;
        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate.0;   // <--- real input rate

        let stream = match sample_format {
            SampleFormat::F32 => build_stream_f32(&device, &config, producer_rec, producer_mon)?,
            SampleFormat::I16 => build_stream_i16(&device, &config, producer_rec, producer_mon)?,
            SampleFormat::U16 => build_stream_u16(&device, &config, producer_rec, producer_mon)?,
            other => anyhow::bail!("Unsupported sample format: {:?}", other),
        };

        Ok((
            Self { stream, channels, sample_rate },
            channels,
            sample_rate,
        ))
    }
}


/// Build input stream when device sample format is f32 (no conversion needed).
fn build_stream_f32<PRec, PMon>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut producer_rec: PRec,
    mut producer_mon: PMon,
) -> Result<Stream>
where
    PRec: Producer<Item = f32> + Send + 'static,
    PMon: Producer<Item = f32> + Send + 'static,
{
    let err_fn = |err| eprintln!("Input stream error: {:?}", err);

    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _| {
            // Push into recorder buffer; mirror into monitor buffer.
            let mut pushed = 0usize;
            while pushed < data.len() {
                let slice = &data[pushed..];
                let n = producer_rec.push_slice(slice);
                if n == 0 {
                    // recorder buffer full -> drop remainder
                    break;
                }
                // Best-effort push into monitor buffer for same region
                let _ = producer_mon.push_slice(&slice[..n]);
                pushed += n;
            }
        },
        err_fn,
        None,
    )?;

    stream.play()?;
    Ok(stream)
}

/// Build input stream for i16 samples; convert to f32 before pushing.
fn build_stream_i16<PRec, PMon>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut producer_rec: PRec,
    mut producer_mon: PMon,
) -> Result<Stream>
where
    PRec: Producer<Item = f32> + Send + 'static,
    PMon: Producer<Item = f32> + Send + 'static,
{
    let err_fn = |err| eprintln!("Input stream error: {:?}", err);

    let stream = device.build_input_stream(
        config,
        move |data: &[i16], _| {
            let mut conv = Vec::with_capacity(data.len());
            for &s in data.iter() {
                conv.push(s as f32 / i16::MAX as f32);
            }

            let mut pushed = 0usize;
            while pushed < conv.len() {
                let slice = &conv[pushed..];
                let n = producer_rec.push_slice(slice);
                if n == 0 {
                    break;
                }
                let _ = producer_mon.push_slice(&slice[..n]);
                pushed += n;
            }
        },
        err_fn,
        None,
    )?;

    stream.play()?;
    Ok(stream)
}

/// Build input stream for u16 samples; convert to f32 before pushing.
fn build_stream_u16<PRec, PMon>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut producer_rec: PRec,
    mut producer_mon: PMon,
) -> Result<Stream>
where
    PRec: Producer<Item = f32> + Send + 'static,
    PMon: Producer<Item = f32> + Send + 'static,
{
    let err_fn = |err| eprintln!("Input stream error: {:?}", err);

    let stream = device.build_input_stream(
        config,
        move |data: &[u16], _| {
            let mut conv = Vec::with_capacity(data.len());
            for &s in data.iter() {
                let f = (s as f32 / u16::MAX as f32) * 2.0 - 1.0;
                conv.push(f);
            }

            let mut pushed = 0usize;
            while pushed < conv.len() {
                let slice = &conv[pushed..];
                let n = producer_rec.push_slice(slice);
                if n == 0 {
                    break;
                }
                let _ = producer_mon.push_slice(&slice[..n]);
                pushed += n;
            }
        },
        err_fn,
        None,
    )?;

    stream.play()?;
    Ok(stream)
}
