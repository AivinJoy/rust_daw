// src/lib.rs

pub mod audio;
pub mod decoder;
mod player;
pub mod waveform;
pub mod recorder;
pub mod daw_controller;
pub mod engine;
pub mod audio_runtime;
pub mod session;

pub use player::AudioPlayer;
pub use waveform::Waveform; // convenience
pub use recorder::Recorder;