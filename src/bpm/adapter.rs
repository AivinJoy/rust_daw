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
    
    // Default values (will be overwritten by first_packet logic)
    let mut sample_rate = 44100;
    let mut channels = 2;
    let mut first_packet = true;

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

        // 1. Get spec from the DECODED packet (AudioBufferRef)
        // This is the source of truth.
        let spec = decoded.spec();
        let current_channels = spec.channels.count();

        // 2. Set the real format based on the first packet
        // This fixes the "Stretched Waveform" bug by correcting the channel count.
        if first_packet {
            sample_rate = spec.rate;
            channels = current_channels;
            first_packet = false;
        }

        // 3. Prepare the buffer
        if sample_buf.is_none() {
            sample_buf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, *spec));
        }
        let buf = sample_buf.as_mut().unwrap();
        
        // 4. Copy samples
        buf.copy_interleaved_ref(decoded);
        
        // 5. Append to output
        // FIX: We check `current_channels` (variable), not `buf.spec()` (method doesn't exist).
        // This ensures we only append data that matches our expected channel count.
        if current_channels == channels {
             out.extend_from_slice(buf.samples());
        }
    }
    
    Ok((out, sample_rate, channels))
}