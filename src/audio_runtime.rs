// src/audio_runtime.rs

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::Stream;

use crate::audio::setup_output_device;
use crate::engine::Engine;
use crate::session::{Session, commands::{SetTrackGain, SetTrackPan, SetTrackMute}}; // Import Session & Commands

/// Owns Engine + CPAL stream and exposes a simple control API.
pub struct AudioRuntime {
    engine: Arc<Mutex<Engine>>,
    master_gain: Arc<Mutex<f32>>,
    session: Mutex<Session>, // New: The Session Manager
    _stream: Stream,
}

pub struct TrackSnapshot {
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
}

pub struct EngineSnapshot {
    pub tracks: Vec<TrackSnapshot>,
}

impl AudioRuntime {
    /// Create engine + output stream. Optionally add one initial track.
    pub fn new(initial_track: Option<String>) -> anyhow::Result<Self> {
        let output = setup_output_device()?;
        let sample_rate = output.output_sample_rate;
        let channels = output.output_channels;

        let master_gain = Arc::new(Mutex::new(1.0_f32));
        let mut engine = Engine::new(sample_rate, channels);

        if let Some(path) = initial_track {
            let _ = engine.add_track(path)?;
            engine.play();
        }

        let engine = Arc::new(Mutex::new(engine));
        let session = Mutex::new(Session::new()); // Initialize Session

        // Build CPAL stream that pulls from Engine::render
        let device = output.device;
        let config = output.config;
        let err_fn = |err| eprintln!("AudioRuntime output error: {err}");
        let engine_cb = engine.clone();
        let gain_cb = master_gain.clone();

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                if let Ok(mut eng) = engine_cb.lock() {
                    eng.render(data);
                    if let Ok(g) = gain_cb.lock() {
                        for s in data.iter_mut() {
                            *s *= *g;
                        }
                    }
                } else {
                    data.fill(0.0);
                }
            },
            err_fn,
            None,
        )?;

        stream.play()?;

        Ok(Self {
            engine,
            master_gain,
            session,
            _stream: stream,
        })
    }

    // --- UNDO / REDO ---

    pub fn undo(&self) {
        if let Ok(mut session) = self.session.lock() {
            if let Ok(success) = session.undo(&self.engine) {
                if success { println!("Using Undo"); }
                else { println!("Nothing to Undo"); }
            }
        }
    }

    pub fn redo(&self) {
        if let Ok(mut session) = self.session.lock() {
            if let Ok(success) = session.redo(&self.engine) {
                if success { println!("Using Redo"); }
                else { println!("Nothing to Redo"); }
            }
        }
    }

    // --- TRANSPORT ---

    pub fn play(&self) {
        if let Ok(mut eng) = self.engine.lock() {
            eng.play();
        }
    }

    pub fn pause(&self) {
        if let Ok(mut eng) = self.engine.lock() {
            eng.pause();
        }
    }

    pub fn toggle_play(&self) {
        if let Ok(mut eng) = self.engine.lock() {
            if eng.transport.playing {
                eng.pause();
            } else {
                eng.play();
            }
        }
    }

    pub fn is_playing(&self) -> bool {
        if let Ok(eng) = self.engine.lock() {
            eng.transport.playing
        } else {
            false
        }
    }

    pub fn seek(&self, pos: Duration) {
        if let Ok(mut eng) = self.engine.lock() {
            eng.seek(pos);
        }
    }

    pub fn position(&self) -> Duration {
        if let Ok(eng) = self.engine.lock() {
            eng.transport.position
        } else {
            Duration::ZERO
        }
    }

    pub fn sample_rate(&self) -> u32 {
        if let Ok(eng) = self.engine.lock() {
            eng.sample_rate
        } else {
            44100
        }
    }

    pub fn add_track(&self, path: String) -> anyhow::Result<()> {
        if let Ok(mut eng) = self.engine.lock() {
            let _ = eng.add_track(path)?;
        }
        Ok(())
    }

    pub fn set_master_gain(&self, gain: f32) {
        if let Ok(mut g) = self.master_gain.lock() {
            *g = gain.clamp(0.0, 2.0);
        }
    }

    pub fn master_gain(&self) -> f32 {
        if let Ok(g) = self.master_gain.lock() {
            *g
        } else {
            1.0
        }
    }

    // --- TRACK CONTROLS (Using Commands) ---

    pub fn toggle_mute(&self, track_index: usize) {
        // 1. Get info needed for the command
        let (track_id, current_mute) = {
            let eng = self.engine.lock().unwrap();
            if let Some(t) = eng.tracks().get(track_index) {
                (t.id, t.muted)
            } else { return; }
        };

        // 2. Create and Apply Command
        let cmd = Box::new(SetTrackMute {
            track_id,
            new_state: !current_mute,
        });

        if let Ok(mut session) = self.session.lock() {
            let _ = session.apply(&self.engine, cmd);
            println!("Track {} mute toggled", track_index);
        }
    }

    pub fn solo_track(&self, solo_index: usize) {
        // Simple solo (not undoable yet for simplicity, or we can make a BulkCommand later)
        if let Ok(mut eng) = self.engine.lock() {
            for (i, track) in eng.tracks_mut().iter_mut().enumerate() {
                if i == solo_index {
                    track.solo = true;
                    track.muted = false;
                } else {
                    track.solo = false;
                    track.muted = true;
                }
            }
            println!("Soloing track {}", solo_index);
        }
    }

    pub fn clear_solo(&self) {
        if let Ok(mut eng) = self.engine.lock() {
            for track in eng.tracks_mut().iter_mut() {
                track.solo = false;
                track.muted = false;
            }
            println!("Solo cleared");
        }
    }

    pub fn adjust_track_gain(&self, track_index: usize, delta: f32) {
        // 1. Get info
        let (track_id, old_gain) = {
            let eng = self.engine.lock().unwrap();
            if let Some(t) = eng.tracks().get(track_index) {
                (t.id, t.gain)
            } else { return; }
        };

        // 2. Create Command
        let new_gain = (old_gain + delta).clamp(0.0, 2.0);
        let cmd = Box::new(SetTrackGain {
            track_id,
            old_gain,
            new_gain,
        });

        // 3. Apply
        if let Ok(mut session) = self.session.lock() {
            let _ = session.apply(&self.engine, cmd);
            println!("Track {} gain: {:.0}%", track_index, new_gain * 100.0);
        }
    }

    pub fn adjust_track_pan(&self, track_index: usize, delta: f32) {
        let (track_id, old_pan) = {
            let eng = self.engine.lock().unwrap();
            if let Some(t) = eng.tracks().get(track_index) {
                (t.id, t.pan)
            } else { return; }
        };

        let new_pan = (old_pan + delta).clamp(-1.0, 1.0);
        let cmd = Box::new(SetTrackPan {
            track_id,
            old_pan,
            new_pan,
        });

        if let Ok(mut session) = self.session.lock() {
            let _ = session.apply(&self.engine, cmd);
            println!("Track {} pan: {:.2}", track_index, new_pan);
        }
    }

    pub fn reset_track_gain(&self, track_index: usize) {
        let (track_id, old_gain) = {
            let eng = self.engine.lock().unwrap();
            if let Some(t) = eng.tracks().get(track_index) {
                (t.id, t.gain)
            } else { return; }
        };

        let cmd = Box::new(SetTrackGain {
            track_id,
            old_gain,
            new_gain: 1.0,
        });

        if let Ok(mut session) = self.session.lock() {
            let _ = session.apply(&self.engine, cmd);
            println!("Track {} gain reset", track_index);
        }
    }

    pub fn reset_track_pan(&self, track_index: usize) {
         let (track_id, old_pan) = {
            let eng = self.engine.lock().unwrap();
            if let Some(t) = eng.tracks().get(track_index) {
                (t.id, t.pan)
            } else { return; }
        };

        let cmd = Box::new(SetTrackPan {
            track_id,
            old_pan,
            new_pan: 0.0,
        });

        if let Ok(mut session) = self.session.lock() {
            let _ = session.apply(&self.engine, cmd);
            println!("Track {} pan reset", track_index);
        }
    }

    pub fn debug_snapshot(&self) -> Option<EngineSnapshot> {
        if let Ok(eng) = self.engine.lock() {
            let tracks = eng
                .tracks()
                .iter()
                .map(|t| TrackSnapshot {
                    gain: t.gain,
                    pan: t.pan,
                    muted: t.muted,
                    solo: t.solo,
                })
                .collect();
            Some(EngineSnapshot { tracks })
        } else {
            None
        }
    }

    // ... inside impl AudioRuntime ...

    pub fn save_session(&self, filename: &str) -> anyhow::Result<()> {
        let master_gain = self.master_gain();
        // Lock session and call save
        if let Ok(session) = self.session.lock() {
            session.save_project(&self.engine, filename, master_gain)?;
            println!("💾 Project saved to {}", filename);
        }
        Ok(())
    }

    pub fn load_session(&self, filename: &str) -> anyhow::Result<()> {
        // Lock session and call load
        if let Ok(mut session) = self.session.lock() {
             // Load returns the saved master gain
            let new_master_gain = session.load_project(&self.engine, filename)?;
            
            // Update master gain
            if let Ok(mut g) = self.master_gain.lock() {
                *g = new_master_gain;
            }
            println!("📂 Project loaded from {}", filename);
        }
        Ok(())
    }

}