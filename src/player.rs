// src/player.rs

use crate::audio::{build_stream, setup_output_device, OutputConfig};
use crate::decoder::{spawn_decoder_with_ctrl, DecoderCmd};
use anyhow::Context;
use cpal::traits::StreamTrait;
use cpal::{SampleFormat, Stream};
use ringbuf::{traits::Split, HeapRb};
use std::fs::File;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    mpsc::Sender,
    Arc,
};
use std::thread::JoinHandle;
use std::time::Duration;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::get_probe;

/// Audio player state and API
pub struct AudioPlayer {
    _stream: Stream,
    _decoder_handle: JoinHandle<()>,
    is_playing: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    total_duration: Duration,
    current_time_samples: Arc<AtomicU64>,
    output_sample_rate: u32,
    output_channels: u16,
    // New: control channel to decoder for seek.
    seek_tx: Sender<DecoderCmd>,
}

impl AudioPlayer {
    /// Creates a new AudioPlayer and starts playing the given audio file.
    pub fn new(path: &str) -> Result<Self, anyhow::Error> {
        // --- 1. Probe File ---
        let file = File::open(path).context("opening audio file")?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let probed = get_probe().format(
            &Default::default(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        let track = probed
            .format
            .default_track()
            .context("no default audio track found")?;
        let source_sample_rate = track
            .codec_params
            .sample_rate
            .context("missing sample rate")?;
        let source_channels = track
            .codec_params
            .channels
            .context("missing channel layout")?
            .count();
        let total_duration = {
            let n_frames = track.codec_params.n_frames.unwrap_or(0);
            let rate = source_sample_rate;
            let seconds = n_frames as f64 / rate as f64;
            Duration::from_secs_f64(seconds)
        };

        println!(
            "🎧 File info: channels: {}, sample_rate: {}, duration: {:?}",
            source_channels, source_sample_rate, total_duration
        );

        // --- 2. Create Ring Buffer ---
        let rb = HeapRb::<f32>::new(131_072);
        let (producer, consumer) = rb.split();

        // --- 3. Shared State ---
        let is_playing = Arc::new(AtomicBool::new(true));
        let volume = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let current_time_samples = Arc::new(AtomicU64::new(0));

        // --- 4. Output device ---
        let output = setup_output_device()?;

        // --- 5. Spawn Decoder (with control) ---
        let (decoder_handle, seek_tx) = spawn_decoder_with_ctrl(
            path.to_string(),
            producer,
            is_playing.clone(),
            source_channels,
            output.output_channels,
            source_sample_rate,
            output.output_sample_rate,
        );

        // --- 6. Build and play CPAL stream ---
        let err_fn = |err| eprintln!("An error occurred on the output audio stream: {}", err);
        let is_playing_callback = is_playing.clone();
        let volume_callback = volume.clone();
        let current_time_callback = current_time_samples.clone();
        let OutputConfig {
            device,
            config,
            sample_format,
            ..
        } = output;

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32, _>(
                device,
                config,
                is_playing_callback,
                volume_callback,
                current_time_callback,
                consumer,
                err_fn,
            )?,
            SampleFormat::I16 => build_stream::<i16, _>(
                device,
                config,
                is_playing_callback,
                volume_callback,
                current_time_callback,
                consumer,
                err_fn,
            )?,
            SampleFormat::U16 => build_stream::<u16, _>(
                device,
                config,
                is_playing_callback,
                volume_callback,
                current_time_callback,
                consumer,
                err_fn,
            )?,
            _ => anyhow::bail!("Unsupported sample format: {:?}", sample_format),
        };

        stream.play()?;

        Ok(Self {
            _stream: stream,
            _decoder_handle: decoder_handle,
            is_playing,
            volume,
            total_duration,
            current_time_samples,
            output_sample_rate: output.output_sample_rate,
            output_channels: output.output_channels as u16,
            seek_tx,
        })
    }

    /// Optional constructor that allows skipping player if no path is provided
    pub fn try_new(path: Option<&str>) -> Result<Option<Self>, anyhow::Error> {
        if let Some(p) = path {
            Ok(Some(Self::new(p)?))
        } else {
            Ok(None)
        }
    }


    // --- Public control functions ---
    pub fn get_total_duration(&self) -> Duration {
        self.total_duration
    }

    pub fn get_current_time(&self) -> Duration {
        let samples = self.current_time_samples.load(Ordering::Relaxed) as f64;
        let frames = samples / self.output_channels as f64;
        let seconds = frames / self.output_sample_rate as f64;
        Duration::from_secs_f64(seconds)
    }

    pub fn pause(&self) {
        self.is_playing.store(false, Ordering::Relaxed);
    }

    pub fn play(&self) {
        self.is_playing.store(true, Ordering::Relaxed);
    }

    pub fn toggle_playback(&self) {
        let was_playing = self.is_playing.fetch_xor(true, Ordering::Relaxed);
        if was_playing {
            println!("\r⏸️ Paused ");
        } else {
            println!("\r▶️ Playing ");
        }
    }

    pub fn set_volume(&self, level: f32) {
        let new_float = level.clamp(0.0, 1.0);
        self.volume.store(new_float.to_bits(), Ordering::Relaxed);
        println!("\r🔊 Volume: {:.0}% ", new_float * 100.0);
    }

    pub fn get_volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    // Absolute seek.
    pub fn seek(&self, pos: Duration) -> Result<(), anyhow::Error> {
        self.seek_tx.send(DecoderCmd::Seek(pos))?;
        // Update UI time immediately for responsiveness.
        let seconds = pos.as_secs_f64();
        let frames = (seconds * self.output_sample_rate as f64).round();
        let samples = (frames as u64) * (self.output_channels as u64);
        self.current_time_samples.store(samples, Ordering::Relaxed);
        Ok(())
    }

    // Relative seek in seconds (signed).
    pub fn seek_by_secs(&self, delta_secs: i64) -> Result<(), anyhow::Error> {
        let cur = self.get_current_time();
        let cur_secs = cur.as_secs_f64();
        let target_secs = (cur_secs + delta_secs as f64).max(0.0).min(self.total_duration.as_secs_f64());
        self.seek(Duration::from_secs_f64(target_secs))
    }
}
