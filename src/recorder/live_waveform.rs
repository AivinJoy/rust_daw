// src/recorder/live_waveform.rs

pub struct LiveWaveform {
    base_bin: usize,
    cur_min: f32,
    cur_max: f32,
    in_bin: usize,
    mins: Vec<f32>,
    maxs: Vec<f32>,
}

impl LiveWaveform {
    pub fn new(base_bin: usize) -> Self {
        Self {
            base_bin,
            cur_min: f32::INFINITY,
            cur_max: f32::NEG_INFINITY,
            in_bin: 0,
            mins: Vec::new(),
            maxs: Vec::new(),
        }
    }

    /// Add one mono sample (we’ll use channel 0 from interleaved data).
    pub fn add_sample(&mut self, s: f32) {
        if s < self.cur_min {
            self.cur_min = s;
        }
        if s > self.cur_max {
            self.cur_max = s;
        }
        self.in_bin += 1;
        if self.in_bin >= self.base_bin {
            self.mins.push(self.cur_min);
            self.maxs.push(self.cur_max);
            self.cur_min = f32::INFINITY;
            self.cur_max = f32::NEG_INFINITY;
            self.in_bin = 0;
        }
    }

    /// Add interleaved block, using channel 0 only.
    pub fn add_block(&mut self, samples: &[f32], channels: usize) {
        if channels == 0 {
            return;
        }
        for frame in samples.chunks(channels) {
            let s0 = frame[0];
            self.add_sample(s0);
        }
    }

    /// Snapshot current mins/maxs for UI (cloned to avoid holding lock).
    pub fn snapshot(&self) -> (Vec<f32>, Vec<f32>) {
        (self.mins.clone(), self.maxs.clone())
    }

    /// Returns the number of bins currently stored.
    /// This is used by the UI controller to check for updates efficiently.
    pub fn len(&self) -> usize {
        self.mins.len()
    }

    /// Returns true if there are no bins stored.
    pub fn is_empty(&self) -> bool {
        self.mins.is_empty()
    }
    
}
