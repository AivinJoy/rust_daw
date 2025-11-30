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
    output_sample_rate: u32,
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
        // Same buffer size as AudioPlayer.
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

    pub fn seek(&self, pos: Duration) {
        let _ = self.seek_tx.send(DecoderCmd::Seek(pos));
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

    pub fn output_sample_rate(&self) -> u32 {
        self.output_sample_rate
    }

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
        // For now, use engine format as "source" format.
        let source_sample_rate = engine_sample_rate;
        let source_channels = engine_channels;

        let decoder = DecoderHandle::new_for_engine(
            path.clone(),
            source_channels,
            engine_channels,
            source_sample_rate,
            engine_sample_rate,
        )?;

        Ok(Self {
            id,
            name: path,
            gain: 1.0,
            pan: 0.0,
            muted: false,
            solo: false,
            state: TrackState::Stopped,
            decoder,
        })
    }

    pub fn set_state(&mut self, st: TrackState) {
        self.state = st;
        self.decoder
            .set_playing(matches!(st, TrackState::Playing));
    }

    pub fn seek(&mut self, pos: Duration) {
        self.decoder.seek(pos);
    }

    pub fn is_audible(&self) -> bool {
        matches!(self.state, TrackState::Playing) && !self.muted && self.gain > 0.0
    }

    /// Pull `frames` of interleaved f32 into `dst`. Returns actually written frames.
    pub fn render_into(&mut self, dst: &mut [f32], channels: usize) -> usize {
        dst.fill(0.0);

        if !self.is_audible() {
            return dst.len() / channels;
        }

        let frames = dst.len() / channels;

        // Read from decoder.
        let written_frames = self.decoder.read_interleaved(dst, frames, channels);

        // Apply gain and simple pan in-place.
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
                let idx = f;
                dst[idx] *= gain;
            } else {
                let base = f * channels;
                dst[base] *= gain * pan_l;
                dst[base + 1] *= gain * pan_r;
                for ch in 2..channels {
                    dst[base + ch] *= gain;
                }
            }
        }

        written_frames
    }
}
