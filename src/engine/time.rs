// src/engine/time.rs

use std::time::Duration;
use serde::{Serialize, Deserialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TimeSignature {
    pub numerator: u32,   // e.g., 4
    pub denominator: u32, // e.g., 4
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self { numerator: 4, denominator: 4 }
    }
}

/// NEW: Holds data for a single grid line on the timeline.
#[derive(Debug, Clone, Serialize)]
pub struct GridLine {
    /// The exact time in seconds corresponding to this line.
    pub time: f64,
    /// Is this line the very start of a bar? (e.g., 1.1.0, 2.1.0)
    pub is_bar_start: bool,
    /// The human-readable bar number (1-indexed).
    pub bar_number: u32,
}


/// The "Brain" that relates Real Time (Seconds) to Musical Time (Bars/Beats).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TempoMap {
    pub bpm: f64,
    pub signature: TimeSignature,
}

impl Default for TempoMap {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            signature: TimeSignature::default(),
        }
    }
}

impl TempoMap {
    pub fn new(bpm: f64, numerator: u32, denominator: u32) -> Self {
        Self {
            bpm,
            signature: TimeSignature { numerator, denominator },
        }
    }

    /// Seconds per beat (e.g., 120 BPM -> 0.5s)
    pub fn seconds_per_beat(&self) -> f64 {
        let quarter_note_spb = 60.0 / self.bpm;
        quarter_note_spb * (4.0 / self.signature.denominator as f64)
    }


    /// Seconds per bar (e.g., 4/4 @ 120 BPM -> 2.0s)
    pub fn seconds_per_bar(&self) -> f64 {
        self.seconds_per_beat() * self.signature.numerator as f64
    }

    /// Convert exact Duration to a Bar/Beat representation.
    /// Returns (bar, beat, percentage_of_beat)
    pub fn timestamp_to_musical(&self, position: Duration) -> (u32, u32, f64) {
        let total_seconds = position.as_secs_f64();
        let spb = self.seconds_per_beat();
        
        let total_beats = total_seconds / spb;
        let beats_per_bar = self.signature.numerator as f64;

        let bar_index = (total_beats / beats_per_bar).floor();
        let beat_in_bar = total_beats % beats_per_bar;
        
        // Bars are usually 1-indexed for humans, but 0-indexed for math.
        // We return 1-indexed Bars (1, 2, 3...) and 1-indexed Beats.
        (
            bar_index as u32 + 1, 
            beat_in_bar.floor() as u32 + 1, 
            beat_in_bar.fract()
        )
    }

    /// Generates grid lines (in Seconds) for a specific time range.
    /// This is what the Frontend will ask for to draw the grid.
    /// `resolution`: 4 = quarter notes, 8 = eighth notes, 16 = sixteenths
    /// UPDATED: Generates grid line data for a specific time range.
    pub fn get_grid_lines(&self, start: Duration, end: Duration, resolution: u32) -> Vec<GridLine> {
        let spb = self.seconds_per_beat();
        let beats_per_bar = self.signature.numerator as f64;
        
        // How many beats are in one grid step?
        // resolution 1 = 1 line per bar
        // resolution 4 = 1 line per quarter note (1 beat)
        let beats_per_step = if resolution == 1 {
            beats_per_bar
        } else {
            4.0 / resolution as f64
        };

        let seconds_per_step = spb * beats_per_step;
        
        let start_sec = start.as_secs_f64();
        let end_sec = end.as_secs_f64();

        // 1. Calculate the starting STEP INDEX (Integer)
        // This aligns us perfectly to the grid, regardless of scroll position
        let mut step_index = (start_sec / seconds_per_step).ceil() as u64;
        
        let mut lines = Vec::new();

        // 2. Loop by Integer Steps (No float accumulation drift)
        loop {
            let time = step_index as f64 * seconds_per_step;
            if time > end_sec + 0.001 {
                break;
            }

            // 3. Calculate Bar/Beat Logic using Integers (if possible) or precise Math
            // How many steps fit in one bar?
            // e.g. 4/4 time, Res 4 (quarter notes) -> 4 steps per bar
            let steps_per_bar = (beats_per_bar / beats_per_step).round() as u64;

            // Is this step the start of a bar?
            // If resolution is 1 (bars), every step is a bar start.
            // If resolution is 4, every 4th step is a bar start.
            let is_bar_start = if steps_per_bar == 0 {
                true 
            } else {
                step_index % steps_per_bar == 0
            };

            // Calculate Bar Number (1-indexed)
            let bar_number = (step_index / steps_per_bar) as u32 + 1;

            lines.push(GridLine {
                time,
                is_bar_start,
                bar_number,
            });

            step_index += 1;
        }
        
        lines
    }
}