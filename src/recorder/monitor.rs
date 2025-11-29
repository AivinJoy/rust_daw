// src/recorder/monitor.rs

use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use ringbuf::consumer::Consumer;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    Arc,
};

pub struct Monitor {
    _stream: Stream,
    enabled: Arc<AtomicBool>,
}

impl Monitor {
    pub fn new<C>(consumer: C) -> Result<Self>
    where
        C: Consumer<Item = f32> + Send + 'static,
    {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No output device available for monitor"))?;

        let supported_config = device.default_output_config()?;
        let config: StreamConfig = supported_config.clone().into();
        let sample_format = supported_config.sample_format();

        let enabled = Arc::new(AtomicBool::new(false));
        let enabled_cb = enabled.clone();

        let volume = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let current_time_samples = Arc::new(AtomicU64::new(0));

        let err_fn = |err| eprintln!("Monitor output error: {}", err);

        let stream = match sample_format {
            SampleFormat::F32 => build_monitor_stream::<f32, _>(
                device,
                config,
                enabled_cb,
                volume,
                current_time_samples,
                consumer,
                err_fn,
            )?,
            SampleFormat::I16 => build_monitor_stream::<i16, _>(
                device,
                config,
                enabled_cb,
                volume,
                current_time_samples,
                consumer,
                err_fn,
            )?,
            SampleFormat::U16 => build_monitor_stream::<u16, _>(
                device,
                config,
                enabled_cb,
                volume,
                current_time_samples,
                consumer,
                err_fn,
            )?,
            _ => anyhow::bail!("Unsupported monitor sample format"),
        };

        stream.play()?;

        Ok(Self { _stream: stream, enabled })
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    pub fn toggle(&self) {
        let cur = self.enabled.load(Ordering::Relaxed);
        self.enabled.store(!cur, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

/// Apply volume and optional DC filtering if needed
#[inline]
fn process_sample(sample: f32, volume: f32) -> f32 {
    (sample * 0.5) * volume  // -6 dB
}


fn build_monitor_stream<T, C>(
    device: cpal::Device,
    config: StreamConfig,
    enabled: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    current_time_samples: Arc<AtomicU64>,
    mut consumer: C,
    err_fn: fn(cpal::StreamError),
) -> Result<Stream>
where
    T: cpal::Sample + cpal::FromSample<f32> + cpal::SizedSample,
    C: Consumer<Item = f32> + Send + 'static,
{
    let channels = config.channels as usize;

    let stream = device.build_output_stream(
        &config,
        move |data: &mut [T], _info: &cpal::OutputCallbackInfo| {
            let vol = f32::from_bits(volume.load(Ordering::Relaxed));
            let on = enabled.load(Ordering::Relaxed);

            for frame in data.chunks_mut(channels) {
                if on {
                    // For each channel, pop one sample and write it to that channel.
                    for out in frame.iter_mut() {
                        let raw = consumer.try_pop().unwrap_or(0.0);
                        let s = process_sample(raw, vol);
                        *out = T::from_sample(s);
                        current_time_samples.fetch_add(1, Ordering::Relaxed);
                    }
                } else {
                    // Monitoring off → silence
                    for out in frame.iter_mut() {
                        *out = T::from_sample(0.0);
                    }
                }
            }
        },
        err_fn,
        None,
    )?;

    Ok(stream)
}

