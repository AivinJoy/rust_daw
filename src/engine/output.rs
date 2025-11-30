use crate::audio::{setup_output_device, OutputConfig};
use crate::engine::Engine;
use cpal::traits::StreamTrait;
use std::sync::{Arc, Mutex};

pub fn run_engine_example() -> anyhow::Result<()> {
    let OutputConfig { device, config, sample_format, output_channels, output_sample_rate } =
        setup_output_device()?;

    let engine = Arc::new(Mutex::new(Engine::new(output_sample_rate, output_channels)));
    // Example: add two tracks
    {
        let mut eng = engine.lock().unwrap();
        eng.add_track("track1.wav".to_string())?;
        eng.add_track("track2.wav".to_string())?;
        eng.play();
    }

    let engine_cb = engine.clone();

    let err_fn = |err| eprintln!("Engine output error: {err}");

    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _| {
            if let Ok(mut eng) = engine_cb.lock() {
                eng.render(data);
            } else {
                data.fill(0.0);
            }
        },
        err_fn,
        None,
    )?;

    stream.play()?;
    // Keep main thread alive (for now you can just loop or block on input)
    loop {}
}
