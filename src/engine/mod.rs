// src/engine/mod.rs

pub mod track;
pub mod mixer;
pub mod time;

pub use track::{Track, TrackId, TrackState};
pub use mixer::Mixer;
pub use time::TempoMap;

use std::time::Duration;

/// Global transport state for the engine.
#[derive(Clone, Debug)]
pub struct Transport {
    pub position: Duration,
    pub playing: bool,
    pub tempo: TempoMap,
}

/// Simple multitrack engine: owns tracks + mixer and advances them in lockstep.
pub struct Engine {
    pub transport: Transport,
    pub sample_rate: u32,
    pub channels: usize,
    tracks: Vec<Track>,
    mixer: Mixer,
}

impl Engine {
    /// Create a new engine for a given device format.
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        Self {
            transport: Transport {
                position: Duration::from_secs(0),
                playing: false,
                tempo: TempoMap::default(), //initailze 120BPM 4/4
            },
            sample_rate,
            channels,
            tracks: Vec::new(),
            mixer: Mixer::new(channels),
        }
    }

    pub fn set_bpm(&mut self, bpm: f32) {
        self.transport.tempo.bpm = bpm as f64;
    }

    pub fn clear_tracks(&mut self) {
        // Drop all tracks (stops their decoders automatically)
        self.tracks.clear();
    }

    /// Add a new track from a file path.
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

    /// Render `frames` into `out` (interleaved f32), mixing all tracks.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);

        if !self.transport.playing {
            return;
        }

        let channels = self.channels;
        let frames = out.len() / channels;

        self.mixer.begin_block(frames);

        for track in &mut self.tracks {
            if !track.is_audible() {
                continue;
            }
            self.mixer.render_track(track, frames, channels);
        }

        self.mixer.mix_into(out, channels);

        let secs = frames as f64 / self.sample_rate as f64;
        self.transport.position += Duration::from_secs_f64(secs);
    }

}