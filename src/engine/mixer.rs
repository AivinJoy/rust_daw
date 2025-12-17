// src/engine/mixer.rs

use super::track::Track;
use std::time::Duration;

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

    pub fn begin_block(&mut self, frames: usize) {
        let needed = frames * self.channels;
        if self.temp_mix.len() != needed {
            self.temp_mix.resize(needed, 0.0);
        } else {
            self.temp_mix.fill(0.0);
        }
    }

    // UPDATED Signature
    pub fn render_track(
        &mut self, 
        track: &mut Track, 
        frames: usize, 
        channels: usize, 
        engine_time: Duration, 
        sample_rate: u32
    ) {
        debug_assert_eq!(channels, self.channels);
        let mut temp = vec![0.0f32; frames * channels];

        // Pass time info to track
        let written_frames = track.render_into(&mut temp, channels, engine_time, sample_rate);
        let samples = written_frames * channels;

        for i in 0..samples {
            self.temp_mix[i] += temp[i];
        }
    }

    pub fn mix_into(&self, out: &mut [f32], channels: usize) {
        debug_assert_eq!(channels, self.channels);
        let len = out.len().min(self.temp_mix.len());
        
        for i in 0..len {
            let sample = self.temp_mix[i];
            if sample.abs() < 1e-10 {
                out[i] = 0.0;
                continue;
            }
            out[i] = sample.tanh();
        }
    }
}