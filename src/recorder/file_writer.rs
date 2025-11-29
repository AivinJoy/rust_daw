// src/recorder/file_writer.rs

use hound::{SampleFormat, WavSpec, WavWriter};
use ringbuf::consumer::Consumer;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use crate::recorder::live_waveform::LiveWaveform;
use anyhow::Result;

/// FileWriter owns a WavWriter and writes samples coming from the ringbuffer consumer.
/// The consumer is generic and constrained so its Item == f32.
pub struct FileWriter {
    writer: WavWriter<BufWriter<File>>,
    #[allow(dead_code)]
    channels: u16,
}

impl FileWriter {
    pub fn new(path: &Path, sample_rate: u32, channels: usize) -> Result<Self> {
        let spec = WavSpec {
            channels: channels as u16,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        let file = File::create(path)?;
        let buf_writer = BufWriter::new(file);
        let writer = WavWriter::new(buf_writer, spec)?;

        Ok(Self {
            writer,
            channels: channels as u16,
        })
    }

    /// Run the writer consuming f32 samples from the ring buffer consumer.
    /// C must implement ringbuf::consumer::Consumer with Item = f32.
    pub fn run<C>(mut self, mut consumer: C) -> Result<()>
    where
        C: Consumer<Item = f32>,
    {
        // Temporary buffer for popping samples from consumer.
        let mut tmp = vec![0.0f32; 4096];

        // We'll only allow the writer to exit once we've written at least one sample.
        // This prevents an immediate exit at startup when the buffer is naturally empty.
        let mut wrote_any = false;

        // When we've written samples and then see the buffer empty for this duration,
        // assume the producer was dropped and exit.
        const GRACEFUL_IDLE_MS: u128 = 500;
        let mut idle_start: Option<Instant> = None;

        loop {
            // pop_slice expects &mut [C::Item] -> &mut [f32]
            let popped = consumer.pop_slice(tmp.as_mut_slice());

            if popped == 0 {
                // No data right now: wait a bit
                thread::sleep(Duration::from_millis(5));

                // If we have previously written samples, start/continue idle timer.
                if wrote_any {
                    idle_start.get_or_insert_with(Instant::now);

                    if let Some(start) = idle_start {
                        if start.elapsed().as_millis() >= GRACEFUL_IDLE_MS {
                            // Buffer has been idle long enough after we wrote data:
                            // assume producer was dropped and exit loop to finalize file.
                            break;
                        }
                    }
                }

                // If we haven't written anything yet, just continue waiting.
                continue;
            }

            // We received samples — reset idle timer and mark we've written data.
            idle_start = None;
            wrote_any = true;

            // Write popped samples as 16-bit signed ints.
            for &s in &tmp[..popped] {
                // clamp and convert
                let samp = if s.is_finite() {
                    (s.max(-1.0).min(1.0) * (i16::MAX as f32)) as i16
                } else {
                    0i16
                };
                self.writer.write_sample(samp)?;
            }
        }

        // finalize WAV (write header sizes, etc.)
        self.writer.finalize()?;
        Ok(())
    }

    pub fn run_with_waveform<C>(
        mut self,
        mut consumer: C,
        live_waveform: Arc<Mutex<LiveWaveform>>,
        channels: usize,
        record_samples: Arc<AtomicU64>,
    ) -> Result<()>
    where
        C: Consumer<Item = f32>,
    {
        let mut tmp = vec![0.0f32; 4096];
        let mut wrote_any = false;
        const GRACEFUL_IDLE_MS: u128 = 500;
        let mut idle_start: Option<Instant> = None;
    
        loop {
            let popped = consumer.pop_slice(tmp.as_mut_slice());
        
            if popped == 0 {
                thread::sleep(Duration::from_millis(5));
                if wrote_any {
                    idle_start.get_or_insert_with(Instant::now);
                    if let Some(start) = idle_start {
                        if start.elapsed().as_millis() >= GRACEFUL_IDLE_MS {
                            break;
                        }
                    }
                }
                continue;
            }
        
            idle_start = None;
            wrote_any = true;
        
            // 1) Write WAV and count samples
            for &s in &tmp[..popped] {
                let samp = if s.is_finite() {
                    (s.max(-1.0).min(1.0) * (i16::MAX as f32)) as i16
                } else {
                    0i16
                };
                self.writer.write_sample(samp)?;
                record_samples.fetch_add(1, Ordering::Relaxed);
            }
        
            // 2) Update live waveform using channel 0 from interleaved data
            {
                let mut wf = live_waveform.lock().unwrap();
                wf.add_block(&tmp[..popped], channels);
            }
        }
    
        self.writer.finalize()?;
        Ok(())
    }


}
