// src/engine/mod.rs

pub mod track;
pub mod mixer;
pub mod time;

pub use track::{Track, TrackId, TrackState};
pub use mixer::Mixer;
pub use time::TempoMap;

use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Transport {
    pub position: Duration,
    pub playing: bool,
    pub tempo: TempoMap,
}

pub struct Engine {
    pub transport: Transport,
    pub sample_rate: u32,
    pub channels: usize,
    pub master_gain: f32, // <--- New Field
    tracks: Vec<Track>,
    mixer: Mixer,
}

impl Engine {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        Self {
            transport: Transport {
                position: Duration::from_secs(0),
                playing: false,
                tempo: TempoMap::default(),
            },
            sample_rate,
            channels,
            master_gain: 1.0, // <--- FIXED: Initialized here (Default 1.0 = 100%)
            tracks: Vec::new(),
            mixer: Mixer::new(channels),
        }
    }

    // --- NEW: Method to set track start time ---
    pub fn set_track_start_time(&mut self, track_index: usize, start_time_secs: f64) {
        if let Some(track) = self.tracks.get_mut(track_index) {
            track.start_time = Duration::from_secs_f64(start_time_secs.max(0.0));
            // Re-seek to ensure decoder is in sync with new position relative to global transport
            track.seek(self.transport.position);
        }
    }

    pub fn set_bpm(&mut self, bpm: f32) {
        self.transport.tempo.bpm = bpm as f64;
    }

    pub fn clear_tracks(&mut self) {
        self.tracks.clear();
    }

    pub fn add_track(&mut self, path: String) -> anyhow::Result<TrackId> {
        let id = TrackId(self.tracks.len() as u32);
        let track = Track::new(id, path, self.sample_rate, self.channels)?;
        self.tracks.push(track);
        Ok(id)
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn tracks_mut(&mut self) -> &mut [Track] {
        &mut self.tracks
    }

    pub fn play(&mut self) {
        self.transport.playing = true;
        for t in &mut self.tracks {
            t.set_state(TrackState::Playing);
        }
    }

    pub fn pause(&mut self) {
        self.transport.playing = false;
        for t in &mut self.tracks {
            t.set_state(TrackState::Paused);
        }
    }

    pub fn seek(&mut self, pos: Duration) {
        self.transport.position = pos;
        for t in &mut self.tracks {
            t.seek(pos);
        }
    }

    // src/engine/mod.rs

    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);

        if !self.transport.playing {
            return;
        }

        let channels = self.channels;
        let frames = out.len() / channels;

        self.mixer.begin_block(frames);

        let current_pos = self.transport.position;
        let sr = self.sample_rate;

        // --- NON-DESTRUCTIVE SOLO LOGIC ---
        // Check if ANY track has solo enabled
        let any_solo = self.tracks.iter().any(|t| t.solo);

        for track in &mut self.tracks {
            // Determine if this specific track should make sound
            let should_play = if any_solo {
                // Solo Mode: Ignore Mute, only play if THIS track is soloed
                track.solo
            } else {
                // Normal Mode: Play if NOT muted
                !track.muted
            };

            // Also check if track is technically playing/active
            if !should_play || track.gain <= 0.001 {
                continue;
            }

            // Note: We access track.state inside render_track or check it here
            // Assuming render_into handles the "is state == Playing" check, 
            // but we can check here to save a function call:
            if matches!(track.state(), TrackState::Playing) {
                 self.mixer.render_track(track, frames, channels, current_pos, sr);
            }
        }

        self.mixer.mix_into(out, channels);

        // Apply Master Gain
        if (self.master_gain - 1.0).abs() > 0.001 {
            for sample in out.iter_mut() {
                *sample *= self.master_gain;
            }
        }

        let secs = frames as f64 / self.sample_rate as f64;
        self.transport.position += Duration::from_secs_f64(secs);
    }
}