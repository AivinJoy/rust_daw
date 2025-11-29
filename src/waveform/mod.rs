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
    pub min: Vec<Vec<f32>>, // [channel][bin]
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
    pub fn build_from_path(path: &str, base_bin: usize) -> Result<Self> {
        // Probe input and get a FormatReader.
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let probed = get_probe().format(
            &Default::default(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        let mut format = probed.format; // owned, mutable FormatReader [web:43]

        // Copy out track_id and codec_params in a short block to drop the borrow of `format`.
        let (track_id, codec_params) = {
            let track = format
                .default_track()
                .ok_or_else(|| anyhow!("no audio track"))?; // immutable borrow ends at block end [web:43]
            (track.id, track.codec_params.clone())
        }; // borrow of `format` ends here, so we can call next_packet() mutably [web:135]

        let sr = codec_params
            .sample_rate
            .ok_or_else(|| anyhow!("missing sample_rate"))?; // owned data from clone [web:43]
        let channels = codec_params
            .channels
            .ok_or_else(|| anyhow!("missing channels"))?
            .count(); // owned data from clone [web:43]

        // Create decoder from owned codec_params (no borrow to `format`).
        let mut decoder = get_codecs().make(&codec_params, &DecoderOptions::default())?; // [web:43]

        // Best-effort duration from n_frames if available.
        let n_frames = codec_params.n_frames.unwrap_or(0);
        let duration_secs = if n_frames > 0 {
            n_frames as f64 / sr as f64
        } else {
            0.0
        }; // [web:43]

        // Level-0 accumulation (min/max) per channel.
        let mut lvl0_min = vec![Vec::<f32>::new(); channels];
        let mut lvl0_max = vec![Vec::<f32>::new(); channels];
        let mut cur_min = vec![f32::INFINITY; channels];
        let mut cur_max = vec![f32::NEG_INFINITY; channels];
        let mut in_bin = 0usize;

        let mut sample_buf: Option<SampleBuffer<f32>> = None;

        loop {
            // Mutably borrow `format` here; no overlapping immutable borrow remains.
            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(_) => break,
            }; // [web:43]

            if packet.track_id() != track_id {
                continue;
            }

            // Decode packet to an AudioBufferRef.
            let decoded = match decoder.decode(&packet) {
                Ok(d) => d,
                Err(_) => continue,
            }; // [web:43]

            // Lazily allocate interleaved f32 buffer once per stream spec.
            if sample_buf.is_none() {
                let capacity = decoded.capacity() as u64;
                sample_buf = Some(SampleBuffer::<f32>::new(capacity, *decoded.spec()));
            } // [web:115]

            // Copy samples interleaved into f32 slice.
            let buf = sample_buf.as_mut().unwrap();
            buf.copy_interleaved_ref(decoded); // interleaved f32 copy [web:115][web:112]
            let samples = buf.samples();
            let frames = samples.len() / channels;

            // Accumulate min/max into fixed-size bins for level 0.
            for f in 0..frames {
                for c in 0..channels {
                    let s = samples[f * channels + c];
                    if s < cur_min[c] {
                        cur_min[c] = s;
                    }
                    if s > cur_max[c] {
                        cur_max[c] = s;
                    }
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

        // Flush a partial bin if present.
        if in_bin > 0 {
            for c in 0..channels {
                lvl0_min[c].push(if cur_min[c].is_finite() { cur_min[c] } else { 0.0 });
                lvl0_max[c].push(if cur_max[c].is_finite() { cur_max[c] } else { 0.0 });
            }
        }

        // Build higher-resolution mip levels by folding pairs of bins (min of mins, max of maxes).
        let mut levels = Vec::new();
        levels.push(WaveformLevel { min: lvl0_min, max: lvl0_max });

        loop {
            let prev = levels.last().unwrap();
            let bins = prev.min[0].len();
            if bins <= 1 {
                break;
            }
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
            if next_bins <= 1 {
                break;
            }
        } // [web:117]

        Ok(Waveform {
            sample_rate: sr,
            channels,
            duration_secs,
            base_bin,
            levels,
        })
    }

    // Choose a level near samples-per-pixel and return slices for a channel.
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
        (&lvl.min[channel][start_bin..end], &lvl.max[channel][start_bin..end], level_idx)
    }
}
