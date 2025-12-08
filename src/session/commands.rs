// src/session/commands.rs

use crate::engine::{Engine, TrackId};
use anyhow::Result;

/// The Command trait defines an action that can be executed and undone.
/// We require Send + Sync so commands can be moved between threads if necessary.
pub trait Command: Send + Sync {
    /// Apply the change to the engine.
    fn execute(&self, engine: &mut Engine) -> Result<()>;
    
    /// Revert the change on the engine.
    fn undo(&self, engine: &mut Engine) -> Result<()>;
    
    /// A description for the UI (e.g., "Set Volume")
    fn name(&self) -> &str;
}

/// Manages the history of commands.
pub struct CommandManager {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
    // We can set a max limit later to save memory, e.g., 50 steps.
    max_history: usize,
}

impl CommandManager {
    pub fn new(max_history: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history,
        }
    }

    /// Execute a new command and push it onto the undo stack.
    /// Clears the redo stack because a new history branch is created.
    pub fn push(&mut self, command: Box<dyn Command>, engine: &mut Engine) -> Result<()> {
        command.execute(engine)?;
        self.undo_stack.push(command);
        self.redo_stack.clear();
        
        // Trim history if too long
        if self.undo_stack.len() > self.max_history {
            self.undo_stack.remove(0);
        }
        Ok(())
    }

    pub fn undo(&mut self, engine: &mut Engine) -> Result<bool> {
        if let Some(cmd) = self.undo_stack.pop() {
            cmd.undo(engine)?;
            self.redo_stack.push(cmd);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn redo(&mut self, engine: &mut Engine) -> Result<bool> {
        if let Some(cmd) = self.redo_stack.pop() {
            cmd.execute(engine)?;
            self.undo_stack.push(cmd);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }
}

// ==========================================
// CONCRETE COMMANDS
// ==========================================

pub struct SetTrackGain {
    pub track_id: TrackId,
    pub old_gain: f32,
    pub new_gain: f32,
}

impl Command for SetTrackGain {
    fn execute(&self, engine: &mut Engine) -> Result<()> {
        if let Some(track) = engine.tracks_mut().iter_mut().find(|t| t.id == self.track_id) {
            track.gain = self.new_gain;
        }
        Ok(())
    }

    fn undo(&self, engine: &mut Engine) -> Result<()> {
        if let Some(track) = engine.tracks_mut().iter_mut().find(|t| t.id == self.track_id) {
            track.gain = self.old_gain;
        }
        Ok(())
    }

    fn name(&self) -> &str { "Change Gain" }
}

pub struct SetTrackPan {
    pub track_id: TrackId,
    pub old_pan: f32,
    pub new_pan: f32,
}

impl Command for SetTrackPan {
    fn execute(&self, engine: &mut Engine) -> Result<()> {
        if let Some(track) = engine.tracks_mut().iter_mut().find(|t| t.id == self.track_id) {
            track.pan = self.new_pan;
        }
        Ok(())
    }

    fn undo(&self, engine: &mut Engine) -> Result<()> {
        if let Some(track) = engine.tracks_mut().iter_mut().find(|t| t.id == self.track_id) {
            track.pan = self.old_pan;
        }
        Ok(())
    }
    
    fn name(&self) -> &str { "Change Pan" }
}

pub struct SetTrackMute {
    pub track_id: TrackId,
    pub new_state: bool,
}

impl Command for SetTrackMute {
    fn execute(&self, engine: &mut Engine) -> Result<()> {
        if let Some(track) = engine.tracks_mut().iter_mut().find(|t| t.id == self.track_id) {
            track.muted = self.new_state;
        }
        Ok(())
    }

    fn undo(&self, engine: &mut Engine) -> Result<()> {
        // Toggle back
        if let Some(track) = engine.tracks_mut().iter_mut().find(|t| t.id == self.track_id) {
            track.muted = !self.new_state;
        }
        Ok(())
    }
    
    fn name(&self) -> &str { "Toggle Mute" }
}