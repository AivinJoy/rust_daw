// src/engine/mixer.rs

use super::track::Track;

/// Simple in-memory mixer that sums all tracks into a master buffer.
pub struct Mixer {
    channels: usize,
    temp_mix: Vec<f32>,
}

impl Mixer {
    pub fn new(channels: usize) -> Self {
        Self {
            channels,
            temp_mix: Vec::new(),
        }
    }

    /// Prepare temp buffer for a new block.
    pub fn begin_block(&mut self, frames: usize) {
        let needed = frames * self.channels;
        if self.temp_mix.len() != needed {
            self.temp_mix.resize(needed, 0.0);
        } else {
            self.temp_mix.fill(0.0);
        }
    }

    /// Ask a track to render into a temporary buffer and add it into `temp_mix`.
    pub fn render_track(&mut self, track: &mut Track, frames: usize, channels: usize) {
        debug_assert_eq!(channels, self.channels);
        let mut temp = vec![0.0f32; frames * channels];

        let written_frames = track.render_into(&mut temp, channels);
        let samples = written_frames * channels;

        for i in 0..samples {
            self.temp_mix[i] += temp[i];
        }
    }

    /// Copy mixed result into final output buffer.
    pub fn mix_into(&self, out: &mut [f32], channels: usize) {
        debug_assert_eq!(channels, self.channels);
        let len = out.len().min(self.temp_mix.len());
        out[..len].copy_from_slice(&self.temp_mix[..len]);
    }
}
