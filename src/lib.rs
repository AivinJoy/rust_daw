// src/lib.rs

mod audio;
pub mod decoder;
mod player;
pub mod waveform;
pub mod recorder;
pub mod daw_controller;

pub use player::AudioPlayer;
pub use waveform::Waveform; // convenience
pub use recorder::Recorder;