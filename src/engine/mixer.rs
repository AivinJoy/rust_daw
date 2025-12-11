// src/engine/mixer.rs

use super::track::Track;

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

    pub fn render_track(&mut self, track: &mut Track, frames: usize, channels: usize) {
        debug_assert_eq!(channels, self.channels);
        let mut temp = vec![0.0f32; frames * channels];

        let written_frames = track.render_into(&mut temp, channels);
        let samples = written_frames * channels;

        for i in 0..samples {
            self.temp_mix[i] += temp[i];
        }
    }

    /// UPDATED: Mix with Soft Clipping and Denormal Protection
    pub fn mix_into(&self, out: &mut [f32], channels: usize) {
        debug_assert_eq!(channels, self.channels);
        let len = out.len().min(self.temp_mix.len());
        
        for i in 0..len {
            let sample = self.temp_mix[i];

            // 1. Silence Clamping (Denormal Protection)
            // Very small floats (e.g. 1e-30) can hurt CPU performance. 
            // If it's effectively silent, force it to exactly 0.0.
            if sample.abs() < 1e-10 {
                out[i] = 0.0;
                continue;
            }

            // 2. Soft Clipping (Analog Saturation)
            // Instead of hard clamping at +/- 1.0 (which sounds bad), 
            // we use tanh() to round off the peaks smoothly.
            // Result is always between -1.0 and 1.0.
            out[i] = sample.tanh();
        }
    }
}