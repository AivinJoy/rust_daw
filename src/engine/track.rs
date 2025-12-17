// src/engine/track.rs

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
    Arc,
};
use std::thread::JoinHandle;
use std::time::Duration;

use ringbuf::traits::Split;
use ringbuf::HeapRb;
use ringbuf::wrap::caching::Caching;
use ringbuf::storage::Heap;
use ringbuf::SharedRb;
use ringbuf::traits::Consumer;

use crate::decoder::{spawn_decoder_with_ctrl, DecoderCmd};

/// Identifier for a track.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TrackId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackState {
    Stopped,
    Playing,
    Paused,
}

/// Concrete decoder handle for one track:
/// owns decoder thread + ringbuffer consumer.
pub struct DecoderHandle {
    consumer: Caching<Arc<SharedRb<Heap<f32>>>, false, true>,
    _decoder_thread: JoinHandle<()>,
    is_playing: Arc<AtomicBool>,
    seek_tx: Sender<DecoderCmd>,
    #[allow(dead_code)]
    output_sample_rate: u32,
    #[allow(dead_code)]
    output_channels: usize,
}

impl DecoderHandle {
    pub fn new_for_engine(
        path: String,
        source_channels: usize,
        output_channels: usize,
        source_sample_rate: u32,
        output_sample_rate: u32,
    ) -> anyhow::Result<Self> {
        let rb = HeapRb::<f32>::new(131_072);
        let (producer, consumer) = rb.split();

        let is_playing = Arc::new(AtomicBool::new(true));

        // spawn_decoder_with_ctrl returns (JoinHandle, Sender<DecoderCmd>).
        let (decoder_thread, seek_tx) = spawn_decoder_with_ctrl(
            path,
            producer,
            is_playing.clone(),
            source_channels,
            output_channels,
            source_sample_rate,
            output_sample_rate,
        );

        Ok(Self {
            consumer,
            _decoder_thread: decoder_thread,
            is_playing,
            seek_tx,
            output_sample_rate,
            output_channels,
        })
    }

    pub fn set_playing(&self, playing: bool) {
        self.is_playing.store(playing, Ordering::Relaxed);
    }

    // --- UPDATED: Seek now clears buffer to fix delay ---
    pub fn seek(&mut self, pos: Duration) {
        // 1. Tell decoder to seek
        let _ = self.seek_tx.send(DecoderCmd::Seek(pos));
        
        // 2. Clear buffer instantly to remove old audio
        // FIX: Use try_pop() instead of pop()
        while self.consumer.try_pop().is_some() {}
    }

    /// Read up to `frames` of interleaved f32 into `dst`. Returns frames actually written.
    pub fn read_interleaved(&mut self, dst: &mut [f32], frames: usize, channels: usize) -> usize {
        let samples_needed = frames * channels;
        let mut written = 0usize;

        while written < samples_needed {
            match self.consumer.try_pop() {
                Some(s) => {
                    dst[written] = s;
                    written += 1;
                }
                None => {
                    break; // non‑blocking: stop when buffer is empty
                }
            }
        }

        let full_samples = written - (written % channels);
        if full_samples < written {
            for i in full_samples..written {
                dst[i] = 0.0;
            }
        }
        full_samples / channels
    }

    #[allow(dead_code)]
    pub fn output_sample_rate(&self) -> u32 {
        self.output_sample_rate
    }
    #[allow(dead_code)]
    pub fn output_channels(&self) -> usize {
        self.output_channels
    }
}

/// A single audio track in the engine.
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub gain: f32,
    pub pan: f32, // -1.0 left, 0 center, +1.0 right
    pub muted: bool,
    pub solo: bool,

    // --- Track Start Time (for Drag & Drop) ---
    pub start_time: Duration,

    state: TrackState,
    decoder: DecoderHandle,
}

impl Track {
    /// `engine_sample_rate` and `engine_channels` should match the output device.
    pub fn new(
        id: TrackId,
        path: String,
        engine_sample_rate: u32,
        engine_channels: usize,
    ) -> anyhow::Result<Self> {
        let decoder = DecoderHandle::new_for_engine(
            path.clone(),
            engine_channels, 
            engine_channels,
            engine_sample_rate,
            engine_sample_rate,
        )?;

        Ok(Self {
            id,
            name: path,
            gain: 1.0,
            pan: 0.0,
            muted: false,
            solo: false,
            start_time: Duration::ZERO,
            state: TrackState::Stopped,
            decoder,
        })
    }

    pub fn state(&self) -> TrackState {
        self.state
    }

    pub fn set_state(&mut self, st: TrackState) {
        self.state = st;
        self.decoder
            .set_playing(matches!(st, TrackState::Playing));
    }

    pub fn seek(&mut self, global_pos: Duration) {
        // Seek relative to track start
        let track_pos = global_pos.saturating_sub(self.start_time);
        self.decoder.seek(track_pos);
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, TrackState::Playing) && self.gain > 0.0
    }

    /// Pull `frames` of interleaved f32 into `dst`.
    /// Handles start_time offset logic.
    pub fn render_into(
        &mut self, 
        dst: &mut [f32], 
        channels: usize, 
        engine_time: Duration, 
        sample_rate: u32
    ) -> usize {
        dst.fill(0.0);

        if !self.is_active() {
            return 0;
        }

        // 1. Calculate time overlap
        let start_secs = self.start_time.as_secs_f64();
        let current_secs = engine_time.as_secs_f64();
        let buffer_duration = (dst.len() / channels) as f64 / sample_rate as f64;
        let end_secs = current_secs + buffer_duration;

        // If the track hasn't started yet in this block
        if end_secs <= start_secs {
            return 0; 
        }

        // 2. Calculate Offset (Silence before track starts within this block)
        let mut offset_frames = 0;
        if current_secs < start_secs {
            let silence_duration = start_secs - current_secs;
            offset_frames = (silence_duration * sample_rate as f64).round() as usize;
        }

        if offset_frames * channels >= dst.len() {
            return 0;
        }

        // 3. Read Audio into the remaining part of the buffer
        let audio_dst = &mut dst[(offset_frames * channels)..];
        let frames_to_read = audio_dst.len() / channels;
        let written_frames = self.decoder.read_interleaved(audio_dst, frames_to_read, channels);

        // 4. Apply Gain/Pan only to the audio part
        let gain = self.gain;
        let pan = self.pan.clamp(-1.0, 1.0);
        let (pan_l, pan_r) = if channels >= 2 {
            let angle = (pan + 1.0) * 0.25 * std::f32::consts::PI;
            (angle.cos(), angle.sin())
        } else {
            (1.0, 1.0)
        };

        for f in 0..written_frames {
            if channels == 1 {
                audio_dst[f] *= gain;
            } else {
                let base = f * channels;
                audio_dst[base] *= gain * pan_l;
                audio_dst[base + 1] *= gain * pan_r;
                for ch in 2..channels {
                    audio_dst[base + ch] *= gain;
                }
            }
        }

        offset_frames + written_frames
    }
}