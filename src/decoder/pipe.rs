// src/decoder/pipe.rs

use anyhow::{anyhow, Result};
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::{Decoder as SymphDecoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::default::{get_codecs, get_probe};
use std::fs::File;

/// Open a media file and return a format reader plus the default track id.
pub fn open_and_probe(path: &str) -> Result<(Box<dyn FormatReader>, u32)> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = get_probe().format(
        &Default::default(),
        mss,
        &symphonia::core::formats::FormatOptions::default(),
        &symphonia::core::meta::MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| anyhow!("no default audio track"))?;
    Ok((format, track.id))
}

/// Build a decoder from the format’s default track parameters.
pub fn make_decoder(format: &mut dyn FormatReader) -> Result<Box<dyn SymphDecoder>> {
    let track = format
        .default_track()
        .ok_or_else(|| anyhow!("no default track"))?;
    let dec = get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    Ok(dec)
}

/// Ensure a SampleBuffer<f32> is allocated to match the decoded buffer’s spec/capacity.
pub fn ensure_sample_buffer<'a>(
    sample_buf: &'a mut Option<SampleBuffer<f32>>,
    decoded: &AudioBufferRef<'_>,
) -> &'a mut SampleBuffer<f32> {
    if sample_buf.is_none() {
        let capacity = decoded.capacity() as u64;
        *sample_buf = Some(SampleBuffer::<f32>::new(capacity, *decoded.spec()));
    }
    sample_buf.as_mut().unwrap()
}

/// Copy decoded audio into interleaved f32.
#[inline]
pub fn copy_interleaved_into_f32(dst: &mut SampleBuffer<f32>, src: AudioBufferRef<'_>) {
    dst.copy_interleaved_ref(src);
}

/// Seek the format reader accurately to the given time on a specific track.
pub fn seek_time(
    format: &mut dyn FormatReader,
    seconds: u64,
    frac: f64,
    track_id: u32,
) -> Result<()> {
    let time = symphonia::core::units::Time::new(seconds, frac);
    format.seek(SeekMode::Accurate, SeekTo::Time { time, track_id: Some(track_id) })?;
    Ok(())
}

/// Step to the next audio packet for the given track id.
pub fn next_packet_for_track(
    format: &mut dyn FormatReader,
    track_id: u32,
) -> Option<symphonia::core::formats::Packet> {
    loop {
        match format.next_packet() {
            Ok(p) if p.track_id() == track_id => return Some(p),
            Ok(_) => continue,
            Err(SymphoniaError::ResetRequired) => return None,
            Err(_) => return None,
        }
    }
}
