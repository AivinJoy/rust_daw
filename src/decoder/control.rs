// src/decoder/control.rs

use std::time::Duration;

/// Commands the decoder thread can handle (extend as needed).
pub enum DecoderCmd {
    Seek(Duration),
}
