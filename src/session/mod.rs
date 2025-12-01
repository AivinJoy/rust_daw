pub mod commands;
pub mod serialization; // <--- ADD THIS
pub mod export;

use crate::engine::Engine;
use commands::{Command, CommandManager};
use serialization::{ProjectManifest, TrackState}; // <--- USE THIS
use std::sync::{Arc, Mutex};
use anyhow::Result;

pub struct Session {
    pub command_manager: CommandManager,
}

impl Session {
    pub fn new() -> Self {
        Self {
            command_manager: CommandManager::new(100),
        }
    }

    pub fn apply(&mut self, engine: &Arc<Mutex<Engine>>, cmd: Box<dyn Command>) -> Result<()> {
        let mut guard = engine.lock().unwrap();
        self.command_manager.push(cmd, &mut guard)
    }

    pub fn undo(&mut self, engine: &Arc<Mutex<Engine>>) -> Result<bool> {
        let mut guard = engine.lock().unwrap();
        self.command_manager.undo(&mut guard)
    }

    pub fn redo(&mut self, engine: &Arc<Mutex<Engine>>) -> Result<bool> {
        let mut guard = engine.lock().unwrap();
        self.command_manager.redo(&mut guard)
    }

    // --- SAVE / LOAD IMPLEMENTATION ---

    pub fn save_project(&self, engine: &Arc<Mutex<Engine>>, path: &str, master_gain: f32) -> Result<()> {
        let eng = engine.lock().unwrap();

        // 1. Gather state from Engine tracks
        let tracks: Vec<TrackState> = eng.tracks().iter().map(|t| TrackState {
            path: t.name.clone(), // We assume name holds the file path
            gain: t.gain,
            pan: t.pan,
            muted: t.muted,
            solo: t.solo,
        }).collect();

        // 2. Create Manifest
        let manifest = ProjectManifest {
            version: 1,
            master_gain,
            tracks,
        };

        // 3. Write to disk
        manifest.save_to_disk(path)?;
        Ok(())
    }

    pub fn load_project(&mut self, engine: &Arc<Mutex<Engine>>, path: &str) -> Result<f32> {
        // 1. Load Manifest from disk
        let manifest = ProjectManifest::load_from_disk(path)?;

        let mut eng = engine.lock().unwrap();

        // 2. Clear existing engine state
        // (We don't have a clear_tracks() method yet, so we iterate and remove, or just drop)
        // Ideally, Engine should support `clear()`. For now, we rely on the fact that `tracks` is a Vec.
        eng.clear_tracks();
        
        // Clear history on load, otherwise undo might try to modify deleted tracks
        self.command_manager = CommandManager::new(100);

        // 3. Rebuild Tracks
        for t_state in manifest.tracks {
            // This spawns new decoders
            let id = eng.add_track(t_state.path)?;
            
            // Apply settings
            if let Some(track) = eng.tracks_mut().iter_mut().find(|t| t.id == id) {
                track.gain = t_state.gain;
                track.pan = t_state.pan;
                track.muted = t_state.muted;
                track.solo = t_state.solo;
            }
        }

        Ok(manifest.master_gain)
    }
}