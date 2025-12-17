pub mod detector;
pub mod utils;
pub mod adapter;

pub use detector::{BpmDetector, BpmOptions, BpmResult};
pub use adapter::analyze_bpm_for_file;
