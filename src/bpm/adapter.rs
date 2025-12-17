// src/bpm/adapter.rs
use anyhow::{anyhow, Result};
use std::fs::File;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::{get_codecs, get_probe};
use crate::bpm::{BpmDetector, BpmOptions};

pub fn analyze_bpm_for_file(path: &str) -> Result<Option<f32>> {
    let (samples, sample_rate, channels) = decode_to_vec(path)?;
    let mut det = BpmDetector::new(2048);
    let opts = BpmOptions { compute_beats: true, ..Default::default() };
    if let Some(res) = det.detect(&samples, channels, sample_rate, opts) {
        Ok(Some(res.bpm))
    } else {
        Ok(None)
    }
}

pub fn decode_to_vec(path: &str) -> Result<(Vec<f32>, u32, usize)> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = get_probe().format(&Default::default(), mss, &FormatOptions::default(), &MetadataOptions::default())?;
    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| anyhow!("no default audio track"))?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    
    let mut decoder = get_codecs().make(&codec_params, &DecoderOptions::default())?;
    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut out = Vec::<f32>::new();
    
    let mut sample_rate = 44100;
    let mut channels = 2;
    let mut format_locked = false;

    // println!("📂 [Analyzer] Opening file: {}", path);

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };
        if packet.track_id() != track_id { continue; }
        
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let spec = decoded.spec();
        let current_channels = spec.channels.count();
        let current_rate = spec.rate;

        // 1. Lock format on the first valid packet
        if !format_locked {
            if decoded.frames() > 0 {
                sample_rate = current_rate;
                channels = current_channels;
                format_locked = true;
                println!("🔍 [Analyzer] Locked Format: {} Hz / {} Ch", sample_rate, channels);
            } else {
                continue;
            }
        }

        // 2. Prepare Buffer
        if sample_buf.is_none() || sample_buf.as_ref().unwrap().capacity() < decoded.capacity() {
            sample_buf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, *spec));
        }
        let buf = sample_buf.as_mut().unwrap();
        buf.copy_interleaved_ref(decoded);
        let new_samples = buf.samples();

        // 3. Handle Channel Mismatch (The Fix)
        if current_channels == channels {
            // Perfect match
            out.extend_from_slice(new_samples);
        } else if current_channels == 1 && channels == 2 {
            // File is Stereo, packet is Mono -> Duplicate L to R
            for &s in new_samples {
                out.push(s);
                out.push(s);
            }
        } else if current_channels == 2 && channels == 1 {
            // File is Mono, packet is Stereo -> Downmix
            for pair in new_samples.chunks(2) {
                let mono = (pair[0] + pair[1]) * 0.5;
                out.push(mono);
            }
        } else {
            // Complex mismatch (e.g. 5.1 to Stereo) - Skip to avoid crash
            // (In a real scenario, you might want to map channels here)
        }
    }
    
    println!("📊 [Analyzer] Decoded {} samples", out.len());
    Ok((out, sample_rate, channels))
}