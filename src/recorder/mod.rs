// src/recorder/mod.rs

pub mod input;
pub mod file_writer;
pub mod monitor;
pub mod live_waveform;

use crate::recorder::{
    file_writer::FileWriter,
    input::AudioInput,
    live_waveform::LiveWaveform,
    monitor::Monitor,
};
use anyhow::Result;
use ringbuf::{HeapRb, traits::Split};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
    Mutex,
};
use std::thread;

pub struct Recorder {
    input: AudioInput,
    writer_handle: Option<thread::JoinHandle<()>>,
    monitor: Monitor,
    live_waveform: Arc<Mutex<LiveWaveform>>,
    record_samples: Arc<AtomicU64>,
}

impl Recorder {
    // Use the real input sample rate from AudioInput.
    pub fn start(path: PathBuf) -> Result<Self> {
        // Ring buffer for recording
        let rec_capacity = 192_000;
        let rb_rec = HeapRb::<f32>::new(rec_capacity);
        let (prod_rec, cons_rec) = rb_rec.split();

        // Ring buffer for monitoring (smaller, low-latency)
        let mon_capacity = 48_000 / 20;
        let rb_mon = HeapRb::<f32>::new(mon_capacity);
        let (prod_mon, cons_mon) = rb_mon.split();

        // Input feeds both ring buffers and returns channels + sample rate
        let (input, channels, input_sample_rate) = AudioInput::new(prod_rec, prod_mon)?;

        // Live waveform accumulator (~512 samples per bin)
        let live_waveform = Arc::new(Mutex::new(LiveWaveform::new(512)));
        let wf_clone = live_waveform.clone();

        // Recording sample counter
        let record_samples = Arc::new(AtomicU64::new(0));
        let record_samples_clone = record_samples.clone();

        // Writer thread: write WAV + update waveform + sample counter
        let path_clone = path.clone();
        let writer_handle = thread::spawn(move || {
            let writer =
                FileWriter::new(&path_clone, input_sample_rate, channels)
                    .expect("Failed to create writer");
            writer
                .run_with_waveform(cons_rec, wf_clone, channels, record_samples_clone)
                .expect("Writer failed");
        });

        // Monitor output (initially muted)
        let monitor = Monitor::new(cons_mon)?;
        monitor.set_enabled(false);

        Ok(Self {
            input,
            writer_handle: Some(writer_handle),
            monitor,
            live_waveform,
            record_samples,
        })
    }

    pub fn stop(mut self) {
        // Drop input to stop capture
        drop(self.input);
        if let Some(h) = self.writer_handle.take() {
            let _ = h.join();
        }
    }

    // Recording time based on samples written and input sample rate.
    pub fn get_record_time(&self) -> std::time::Duration {
        let samples = self.record_samples.load(Ordering::Relaxed) as f64;
        let secs = samples / self.input.sample_rate as f64;
        std::time::Duration::from_secs_f64(secs)
    }

    pub fn toggle_monitor(&mut self) -> Result<()> {
        self.monitor.toggle();
        if self.monitor.is_enabled() {
            println!("\n🎧 Monitor ON");
        } else {
            println!("\n🎧 Monitor OFF");
        }
        Ok(())
    }

    /// For UI: clone the Arc so main.rs can snapshot bins.
    pub fn live_waveform(&self) -> Arc<Mutex<LiveWaveform>> {
        self.live_waveform.clone()
    }
}
