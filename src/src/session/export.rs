// src/session/export.rs

use crate::session::serialization::ProjectManifest;
use crate::decoder::{pipe, resample};
use anyhow::Result;
use hound::{WavSpec, WavWriter, SampleFormat};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::formats::FormatReader;
use symphonia::core::codecs::Decoder;
use rubato::Resampler; 

pub struct ExportVoice {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    resampler: Option<rubato::SincFixedIn<f32>>,
    
    // Buffers
    input_sample_buf: Option<SampleBuffer<f32>>,
    
    // Accumulators for smooth resampling
    resampler_input_buffer: Vec<f32>, // Interleaved input waiting to be processed
    output_buffer: Vec<f32>,          // Interleaved output waiting to be mixed
    
    // State
    finished: bool,
    source_channels: usize,
    
    // Settings
    gain: f32,
    pan: f32,
    muted: bool,
}

impl ExportVoice {
    pub fn new(path: &str, target_sample_rate: u32) -> Result<Self> {
        let (format, track_id) = pipe::open_and_probe(path)?;
        
        let track = format.tracks().iter().find(|t| t.id == track_id).unwrap();
        let source_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let source_channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);

        let mut format = format; 
        let decoder = pipe::make_decoder(&mut *format)?;
        
        // Build resampler if rates differ
        let resampler = if source_rate != target_sample_rate {
            resample::build_resampler(source_rate, target_sample_rate, 2)?
        } else {
            None
        };

        Ok(Self {
            format,
            decoder,
            track_id,
            resampler,
            input_sample_buf: None,
            resampler_input_buffer: Vec::with_capacity(4096),
            output_buffer: Vec::new(),
            finished: false,
            source_channels,
            gain: 1.0,
            pan: 0.0,
            muted: false,
        })
    }

    /// Decodes packets until we have enough data in output_buffer OR we hit EOF.
    fn prepare_samples(&mut self, frames_needed: usize) -> Result<bool> {
        // While we don't have enough output data...
        while self.output_buffer.len() < frames_needed * 2 {
            
            // If we are finished and buffer is empty, stop.
            if self.finished && self.resampler_input_buffer.is_empty() {
                break;
            }

            // 1. Fill the Accumulator (Input) if it's low
            let chunk_size = if let Some(r) = &self.resampler { r.input_frames_next() } else { 0 };
            let needed_for_resample = if chunk_size > 0 { chunk_size * 2 } else { 0 }; // *2 for stereo

            // If we need more raw data from disk, fetch it
            if !self.finished && (self.resampler.is_none() || self.resampler_input_buffer.len() < needed_for_resample) {
                // FIX: Loop now returns Option<Packet>
                let packet_opt = loop {
                    match self.format.next_packet() {
                        Ok(p) => {
                            if p.track_id() == self.track_id {
                                break Some(p);
                            }
                        },
                        Err(_) => {
                            self.finished = true;
                            break None; // Return None instead of a dummy packet
                        }
                    }
                };

                // FIX: Only process if we got a valid packet
                if let Some(packet) = packet_opt {
                    let decoded = match self.decoder.decode(&packet) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };

                    // Copy to interleaved f32
                    if self.input_sample_buf.is_none() {
                        let spec = *decoded.spec();
                        // Force a large buffer to avoid reallocation/truncation on large packets
                        self.input_sample_buf = Some(SampleBuffer::new(decoded.capacity() as u64 + 4096, spec));
                    }
                    let buf = self.input_sample_buf.as_mut().unwrap();
                    buf.copy_interleaved_ref(decoded);
                    let samples = buf.samples();

                    // Handle Mono -> Stereo expansion immediately during load
                    if self.source_channels == 1 {
                        for s in samples {
                            self.resampler_input_buffer.push(*s);
                            self.resampler_input_buffer.push(*s);
                        }
                    } else {
                        // Assume Stereo (or drop extra channels for now)
                        self.resampler_input_buffer.extend_from_slice(samples);
                    }
                }
            }

            // 2. Process the Accumulator -> Output
            if let Some(r) = &mut self.resampler {
                let chunk_frames = r.input_frames_next();
                let chunk_samples = chunk_frames * 2;

                // Process FULL chunks
                while self.resampler_input_buffer.len() >= chunk_samples {
                    // Extract one chunk
                    let chunk_slice = &self.resampler_input_buffer[0..chunk_samples];
                    
                    // De-interleave for Rubato
                    let mut planar = vec![Vec::with_capacity(chunk_frames); 2];
                    for i in 0..chunk_frames {
                        planar[0].push(chunk_slice[i*2]);
                        planar[1].push(chunk_slice[i*2+1]);
                    }

                    // Process
                    if let Ok(resampled) = r.process(&planar, None) {
                         let out_frames = resampled[0].len();
                         for i in 0..out_frames {
                             self.output_buffer.push(resampled[0][i]);
                             self.output_buffer.push(resampled[1][i]);
                         }
                    }

                    // Remove processed samples
                    self.resampler_input_buffer.drain(0..chunk_samples);
                }

                // If finished and we have leftovers, FLUSH partial
                if self.finished && !self.resampler_input_buffer.is_empty() {
                    let remaining_samples = self.resampler_input_buffer.len();
                    let remaining_frames = remaining_samples / 2;
                    
                    if remaining_frames > 0 {
                        let mut planar = vec![Vec::with_capacity(remaining_frames); 2];
                        for i in 0..remaining_frames {
                            planar[0].push(self.resampler_input_buffer[i*2]);
                            planar[1].push(self.resampler_input_buffer[i*2+1]);
                        }
                        
                        // Use process_partial for the final bit with explicit type annotation
                        if let Ok(resampled) = r.process_partial::<Vec<f32>>(Some(&planar), None) {
                             if !resampled.is_empty() {
                                 let out_frames = resampled[0].len();
                                 for i in 0..out_frames {
                                     self.output_buffer.push(resampled[0][i]);
                                     self.output_buffer.push(resampled[1][i]);
                                 }
                             }
                        }
                    }
                    self.resampler_input_buffer.clear();
                }

            } else {
                // No Resampling: Move everything from Accumulator to Output directly
                self.output_buffer.append(&mut self.resampler_input_buffer);
                
                if self.finished && self.output_buffer.is_empty() {
                    break;
                }
            }

            // Break loop if we have enough output, or if we are totally done
            if self.output_buffer.len() >= frames_needed * 2 {
                break;
            }
            if self.finished && self.resampler_input_buffer.is_empty() {
                break;
            }
        }
        
        Ok(!self.output_buffer.is_empty())
    }

    pub fn add_to_mix(&mut self, out_buf: &mut [f32], frames: usize) -> Result<()> {
        if self.muted { return Ok(()); }
        
        self.prepare_samples(frames)?;

        let samples_to_take = (frames * 2).min(self.output_buffer.len());
        
        let pan = self.pan.clamp(-1.0, 1.0);
        let (pan_l, pan_r) = if self.pan != 0.0 {
            let angle = (pan + 1.0) * 0.25 * std::f32::consts::PI;
            (angle.cos(), angle.sin())
        } else {
            (1.0, 1.0)
        };

        for i in 0..(samples_to_take / 2) {
            let l = self.output_buffer[i*2] * self.gain * pan_l;
            let r = self.output_buffer[i*2+1] * self.gain * pan_r;
            
            out_buf[i*2] += l;
            out_buf[i*2+1] += r;
        }

        if samples_to_take > 0 {
             self.output_buffer.drain(0..samples_to_take);
        }

        Ok(())
    }
    
    pub fn is_finished(&self) -> bool {
        self.finished && self.output_buffer.is_empty() && self.resampler_input_buffer.is_empty()
    }
}

pub fn export_project_to_wav(manifest: &ProjectManifest, output_path: &str) -> Result<()> {
    println!("🚀 Starting Offline Export (Safe Mode)...");

    let sample_rate = 44100;
    let spec = WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(output_path, spec)?;

    let mut voices = Vec::new();
    for t_state in &manifest.tracks {
        println!("   -> Loading: {}", t_state.path);
        if let Ok(mut v) = ExportVoice::new(&t_state.path, sample_rate) {
            v.gain = t_state.gain;
            v.pan = t_state.pan;
            v.muted = t_state.muted; 
            voices.push(v);
        } else {
             eprintln!("   ⚠️ Failed to load {}", t_state.path);
        }
    }

    // Solo Logic
    let any_solo = manifest.tracks.iter().any(|t| t.solo);
    if any_solo {
        for (i, v) in voices.iter_mut().enumerate() {
            if i < manifest.tracks.len() {
                if !manifest.tracks[i].solo {
                    v.muted = true;
                } else {
                    v.muted = false; 
                }
            }
        }
    }

    let block_size = 1024;
    let mut mix_buffer = vec![0.0; block_size * 2]; 
    let mut total_frames = 0;

    loop {
        if voices.iter().all(|v| v.is_finished()) {
            break;
        }

        mix_buffer.fill(0.0);

        for v in &mut voices {
            v.add_to_mix(&mut mix_buffer, block_size)?;
        }

        for sample in &mix_buffer {
             let val = *sample;
             // Soft Clip
             let soft_clipped = val.tanh(); 
             let s = (soft_clipped * i16::MAX as f32) as i16;
             writer.write_sample(s)?;
        }
        
        total_frames += block_size;
        // Print progress less frequently (every 5 seconds) to avoid spam
        if total_frames % (44100 * 5) == 0 {
            print!(".");
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    }

    println!("\n✅ Export Complete! Saved to {}", output_path);
    writer.finalize()?;
    Ok(())
}