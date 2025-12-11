// src/waveform/mod.rs
pub mod terminal;
use anyhow::{anyhow, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::{get_codecs, get_probe};
use std::fs::File;

pub struct WaveformLevel {
    pub min: Vec<Vec<f32>>,
    pub max: Vec<Vec<f32>>,
}

pub struct Waveform {
    pub sample_rate: u32,
    pub channels: usize,
    pub duration_secs: f64,
    pub base_bin: usize,
    pub levels: Vec<WaveformLevel>,
}

impl Waveform {
    /// 1. Optimized Builder: Uses pre-decoded samples from adapter.rs
    /// This ensures Waveform duration == Audio duration exactly.
    pub fn build_from_samples(
        samples: &[f32],
        sample_rate: u32,
        channels: usize,
        base_bin: usize,
    ) -> Self {
        let mut lvl0_min = vec![Vec::<f32>::new(); channels];
        let mut lvl0_max = vec![Vec::<f32>::new(); channels];
        let mut cur_min = vec![f32::INFINITY; channels];
        let mut cur_max = vec![f32::NEG_INFINITY; channels];
        let mut in_bin = 0usize;
        let mut global_peak = 0.0f32;

        for frame in samples.chunks(channels) {
            for (c, &sample) in frame.iter().enumerate() {
                if sample < cur_min[c] { cur_min[c] = sample; }
                if sample > cur_max[c] { cur_max[c] = sample; }
                if sample.abs() > global_peak { global_peak = sample.abs(); }
            }
            
            in_bin += 1;
            if in_bin == base_bin {
                for c in 0..channels {
                    lvl0_min[c].push(cur_min[c]);
                    lvl0_max[c].push(cur_max[c]);
                    cur_min[c] = f32::INFINITY;
                    cur_max[c] = f32::NEG_INFINITY;
                }
                in_bin = 0;
            }
        }

        // Flush remainder
        if in_bin > 0 {
            for c in 0..channels {
                lvl0_min[c].push(if cur_min[c].is_finite() { cur_min[c] } else { 0.0 });
                lvl0_max[c].push(if cur_max[c].is_finite() { cur_max[c] } else { 0.0 });
            }
        }

        // Normalize
        if global_peak > 0.0 {
            let scale = 1.0 / global_peak;
            for c in 0..channels {
                for v in &mut lvl0_min[c] { *v *= scale; }
                for v in &mut lvl0_max[c] { *v *= scale; }
            }
        }

        // Exact duration calculation
        let total_frames = samples.len() / channels;
        let duration_secs = total_frames as f64 / sample_rate as f64;

        Self::build_mipmaps(sample_rate, channels, duration_secs, base_bin, lvl0_min, lvl0_max)
    }

    /// 2. Legacy Builder: Included for compatibility, but updated to count samples
    pub fn build_from_path(path: &str, base_bin: usize) -> Result<Self> {
        // ... (This basically duplicates the logic above but with decoding)
        // For brevity, I recommend using build_from_samples via the main.rs flow
        // but here is the quick fix for this one too:
        
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let probed = get_probe().format(&Default::default(), mss, &FormatOptions::default(), &MetadataOptions::default())?;
        let mut format = probed.format;
        let track = format.default_track().ok_or_else(|| anyhow!("no audio track"))?;
        let track_id = track.id;
        let codec_params = track.codec_params.clone();

        let mut sr = codec_params.sample_rate.unwrap_or(44100);
        let mut channels = codec_params.channels.map(|c| c.count()).unwrap_or(2);
        let mut decoder = get_codecs().make(&codec_params, &DecoderOptions::default())?;

        let mut lvl0_min = vec![Vec::<f32>::new(); channels];
        let mut lvl0_max = vec![Vec::<f32>::new(); channels];
        let mut cur_min = vec![f32::INFINITY; channels];
        let mut cur_max = vec![f32::NEG_INFINITY; channels];
        let mut in_bin = 0usize;
        let mut sample_buf: Option<SampleBuffer<f32>> = None;
        let mut total_frames_decoded = 0u64;
        let mut format_verified = false;

        loop {
            let packet = match format.next_packet() {
                Ok(p) => p, Err(_) => break,
            };
            if packet.track_id() != track_id { continue; }
            let decoded = match decoder.decode(&packet) {
                Ok(d) => d, Err(_) => continue,
            };

            // FIX: Check real format
            if !format_verified {
                let spec = decoded.spec();
                if spec.rate != sr || spec.channels.count() != channels {
                    sr = spec.rate;
                    channels = spec.channels.count();
                    // Reset if format changed
                    lvl0_min = vec![Vec::new(); channels];
                    lvl0_max = vec![Vec::new(); channels];
                    cur_min = vec![f32::INFINITY; channels];
                    cur_max = vec![f32::NEG_INFINITY; channels];
                }
                format_verified = true;
            }

            if sample_buf.is_none() {
                sample_buf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec()));
            }
            let buf = sample_buf.as_mut().unwrap();
            buf.copy_interleaved_ref(decoded);
            let samples = buf.samples();
            let frames = samples.len() / channels;
            total_frames_decoded += frames as u64;

            for f in 0..frames {
                for c in 0..channels {
                    if c >= channels { break; } // Safety
                    let s = samples[f * channels + c];
                    if s < cur_min[c] { cur_min[c] = s; }
                    if s > cur_max[c] { cur_max[c] = s; }
                }
                in_bin += 1;
                if in_bin == base_bin {
                    for c in 0..channels {
                        lvl0_min[c].push(cur_min[c]);
                        lvl0_max[c].push(cur_max[c]);
                        cur_min[c] = f32::INFINITY;
                        cur_max[c] = f32::NEG_INFINITY;
                    }
                    in_bin = 0;
                }
            }
        }
        
        if in_bin > 0 {
            for c in 0..channels {
                lvl0_min[c].push(if cur_min[c].is_finite() { cur_min[c] } else { 0.0 });
                lvl0_max[c].push(if cur_max[c].is_finite() { cur_max[c] } else { 0.0 });
            }
        }

        let duration_secs = total_frames_decoded as f64 / sr as f64;
        Ok(Self::build_mipmaps(sr, channels, duration_secs, base_bin, lvl0_min, lvl0_max))
    }

    fn build_mipmaps(
        sample_rate: u32,
        channels: usize,
        duration_secs: f64,
        base_bin: usize,
        lvl0_min: Vec<Vec<f32>>,
        lvl0_max: Vec<Vec<f32>>,
    ) -> Self {
        let mut levels = Vec::new();
        levels.push(WaveformLevel { min: lvl0_min, max: lvl0_max });

        loop {
            let prev = levels.last().unwrap();
            let bins = prev.min[0].len();
            if bins <= 1 { break; }
            let next_bins = bins / 2;
            let mut next_min = vec![Vec::with_capacity(next_bins); channels];
            let mut next_max = vec![Vec::with_capacity(next_bins); channels];
            for c in 0..channels {
                let pm = &prev.min[c];
                let px = &prev.max[c];
                let mut i = 0usize;
                while i + 1 < pm.len() {
                    let m = pm[i].min(pm[i + 1]);
                    let x = px[i].max(px[i + 1]);
                    next_min[c].push(m);
                    next_max[c].push(x);
                    i += 2;
                }
                if i < pm.len() {
                    next_min[c].push(pm[i]);
                    next_max[c].push(px[i]);
                }
            }
            levels.push(WaveformLevel { min: next_min, max: next_max });
            if next_bins <= 1 { break; }
        }

        Self {
            sample_rate,
            channels,
            duration_secs,
            base_bin,
            levels,
        }
    }

    pub fn bins_for(
        &self,
        samples_per_pixel: f64,
        channel: usize,
        start_bin: usize,
        columns: usize,
    ) -> (&[f32], &[f32], usize) {
        let mut level_idx = 0usize;
        let mut bin_size = self.base_bin as f64;
        while level_idx + 1 < self.levels.len() && bin_size * 2.0 <= samples_per_pixel {
            level_idx += 1;
            bin_size *= 2.0;
        }
        let lvl = &self.levels[level_idx];
        let total_bins = lvl.min[0].len();
        let end = (start_bin + columns).min(total_bins);
        if channel < lvl.min.len() {
            (&lvl.min[channel][start_bin..end], &lvl.max[channel][start_bin..end], level_idx)
        } else {
            (&[], &[], level_idx)
        }
    }
}