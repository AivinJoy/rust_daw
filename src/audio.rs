// src/audio.rs

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig, SizedSample};
use ringbuf::consumer::Consumer;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    Arc,
};

/// Helper struct to hold output device info
pub struct OutputConfig {
    pub device: Device,
    pub config: StreamConfig,
    pub sample_format: SampleFormat,
    pub output_channels: usize,
    pub output_sample_rate: u32,
}

/// Finds the default audio output device and its config.
pub fn setup_output_device() -> Result<OutputConfig, anyhow::Error> {
    let host = cpal::default_host();
    let device = host.default_output_device().expect("No output device available.");
    let supported_config = device.default_output_config()?;
    let sample_format = supported_config.sample_format();
    let config = supported_config.config();
    let output_channels = config.channels as usize;
    let output_sample_rate = config.sample_rate.0;

    println!(
        "🔊 Output device: channels: {}, sample_rate: {:?}",
        output_channels, config.sample_rate
    );

    Ok(OutputConfig {
        device,
        config,
        sample_format,
        output_channels,
        output_sample_rate,
    })
}

/// Build CPAL output stream.
pub fn build_stream<T, C>(
    device: cpal::Device,
    config: StreamConfig,
    is_playing: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    current_time_samples: Arc<AtomicU64>,
    mut consumer: C,
    err_fn: fn(cpal::StreamError),
) -> Result<Stream, anyhow::Error>
where
    T: cpal::Sample + cpal::FromSample<f32> + SizedSample,
    C: Consumer<Item = f32> + Send + 'static,
{
    device
        .build_output_stream(
            &config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let vol_float = f32::from_bits(volume.load(Ordering::Relaxed));
                let playing = is_playing.load(Ordering::Relaxed);

                for out in data.iter_mut() {
                    let s = if playing {
                        let s = consumer.try_pop().unwrap_or(0.0);
                        current_time_samples.fetch_add(1, Ordering::Relaxed);
                        s
                    } else {
                        0.0
                    };
                    *out = T::from_sample(s * vol_float);
                }
            },
            err_fn,
            None,
        )
        .map_err(Into::into)
}