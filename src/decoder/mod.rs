// src/decoder/mod.rs

mod control;
mod dsp;
mod output;
mod resample;

use anyhow::anyhow;
use ringbuf::traits::Producer as RbProducer;
use rubato::Resampler; // for .reset()
use std::fs::File;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{channel, Receiver, Sender},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::{get_codecs, get_probe};

pub use control::DecoderCmd;

pub struct Decoder<P>
where
    P: RbProducer<Item = f32> + Send + 'static,
{
    path: String,
    producer: P,
    is_playing: Arc<AtomicBool>,
    output_channels: usize,
    source_sample_rate: u32,
    output_sample_rate: u32,
    cmd_rx: Receiver<DecoderCmd>,
    post_seek_fade_samples: usize,
}

impl<P> Decoder<P>
where
    P: RbProducer<Item = f32> + Send + 'static,
{
    pub fn new_with_ctrl(
        path: String,
        producer: P,
        is_playing: Arc<AtomicBool>,
        _source_channels: usize,
        output_channels: usize,
        source_sample_rate: u32,
        output_sample_rate: u32,
        cmd_rx: Receiver<DecoderCmd>,
    ) -> Self {
        Self {
            path,
            producer,
            is_playing,
            output_channels,
            source_sample_rate,
            output_sample_rate,
            cmd_rx,
            post_seek_fade_samples: 0,
        }
    }

    pub fn spawn(self) -> JoinHandle<()> {
        thread::spawn(move || {
            if let Err(e) = self.run() {
                eprintln!("Decoder thread error: {e}");
            }
        })
    }

    fn run(mut self) -> Result<(), anyhow::Error> {
        let file = File::open(&self.path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let probed = get_probe().format(
            &Default::default(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        let mut format = probed.format;

        let track = format
            .default_track()
            .ok_or_else(|| anyhow!("no default audio track"))?;
        let track_id = track.id;

        let mut decoder = get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
        let mut sample_buf: Option<SampleBuffer<f32>> = None;

        let mut resampler =
            resample::build_resampler(self.source_sample_rate, self.output_sample_rate, self.output_channels)?;
        let mut stage_planar: Vec<Vec<f32>> = vec![Vec::with_capacity(4096); self.output_channels];

        loop {
            while let Ok(cmd) = self.cmd_rx.try_recv() {
                match cmd {
                    DecoderCmd::Seek(target) => {
                        let seconds = target.as_secs();
                        let frac = target.subsec_nanos() as f64 / 1_000_000_000f64;
                        let time = symphonia::core::units::Time::new(seconds, frac);
                        format.seek(
                            SeekMode::Accurate,
                            SeekTo::Time {
                                time,
                                track_id: Some(track_id),
                            },
                        )?;

                        sample_buf = None;
                        for ch in &mut stage_planar {
                            ch.clear();
                        }
                        if let Some(r) = &mut resampler {
                            r.reset();
                        }
                        self.post_seek_fade_samples =
                            dsp::fade_samples_ms(self.output_sample_rate, 10) * self.output_channels;
                    }
                }
            }

            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(SymphoniaError::ResetRequired) => break,
                Err(_) => break,
            };

            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    let decoded_ch = decoded.spec().channels.count();

                    if sample_buf.is_none() {
                        let capacity = decoded.capacity() as u64;
                        sample_buf = Some(SampleBuffer::<f32>::new(capacity, *decoded.spec()));
                    }
                    let buf = sample_buf.as_mut().unwrap();

                    copy_interleaved_into_f32(buf, decoded);
                    let src_interleaved = buf.samples();

                    if resampler.is_some() {
                        if decoded_ch == self.output_channels {
                            dsp::append_interleaved_to_planar(
                                src_interleaved,
                                &mut stage_planar,
                                self.output_channels,
                            );
                        } else {
                            let mixed = dsp::updown_mix_interleaved(
                                src_interleaved,
                                decoded_ch,
                                self.output_channels,
                            );
                            dsp::append_interleaved_to_planar(
                                &mixed,
                                &mut stage_planar,
                                self.output_channels,
                            );
                        }

                        while let Some(mut out_block) =
                            resample::try_process_exact(resampler.as_mut().unwrap(), &mut stage_planar)
                        {
                            let interleaved_out = dsp::interleave(out_block.as_mut_slice());
                            output::push_with_fade(
                                &mut self.producer,
                                &interleaved_out,
                                &mut self.post_seek_fade_samples,
                            );
                        }
                    } else {
                        if decoded_ch == self.output_channels {
                            output::push_with_fade(
                                &mut self.producer,
                                src_interleaved,
                                &mut self.post_seek_fade_samples,
                            );
                        } else {
                            let mixed = dsp::updown_mix_interleaved(
                                src_interleaved,
                                decoded_ch,
                                self.output_channels,
                            );
                            output::push_with_fade(
                                &mut self.producer,
                                &mixed,
                                &mut self.post_seek_fade_samples,
                            );
                        }
                    }
                }
                Err(SymphoniaError::IoError(_)) => continue,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(_) => break,
            }

            if !self.is_playing.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(10));
            }
        }

        if let Some(r) = &mut resampler {
            if let Some(mut planar) = resample::drain_remaining_planar(&mut stage_planar) {
                if let Some(mut out) = resample::process_partial_some(r, planar.as_mut_slice())? {
                    let interleaved_out = dsp::interleave(out.as_mut_slice());
                    output::push_with_fade(
                        &mut self.producer,
                        &interleaved_out,
                        &mut self.post_seek_fade_samples,
                    );
                }
            }
            if let Some(mut out) = resample::process_partial_none::<f32>(r)? {
                if !out.is_empty() && !out[0].is_empty() {
                    let interleaved_out = dsp::interleave(out.as_mut_slice());
                    output::push_with_fade(
                        &mut self.producer,
                        &interleaved_out,
                        &mut self.post_seek_fade_samples,
                    );
                }
            }
        }

        Ok(())
    }
}

pub fn spawn_decoder_with_ctrl<P>(
    path: String,
    producer: P,
    is_playing: Arc<AtomicBool>,
    source_channels: usize,
    output_channels: usize,
    source_sample_rate: u32,
    output_sample_rate: u32,
) -> (JoinHandle<()>, Sender<control::DecoderCmd>)
where
    P: RbProducer<Item = f32> + Send + 'static,
{
    let (tx, rx) = channel();
    let handle = Decoder::new_with_ctrl(
        path,
        producer,
        is_playing,
        source_channels,
        output_channels,
        source_sample_rate,
        output_sample_rate,
        rx,
    )
    .spawn();
    (handle, tx)
}

#[allow(dead_code)]
pub fn spawn_decoder<P>(
    path: String,
    producer: P,
    is_playing: Arc<AtomicBool>,
    source_channels: usize,
    output_channels: usize,
    source_sample_rate: u32,
    output_sample_rate: u32,
) -> JoinHandle<()>
where
    P: RbProducer<Item = f32> + Send + 'static,
{
    let (h, _tx) = spawn_decoder_with_ctrl(
        path,
        producer,
        is_playing,
        source_channels,
        output_channels,
        source_sample_rate,
        output_sample_rate,
    );
    h
}

#[inline]
fn copy_interleaved_into_f32(dst: &mut SampleBuffer<f32>, src: AudioBufferRef<'_>) {
    dst.copy_interleaved_ref(src);
}