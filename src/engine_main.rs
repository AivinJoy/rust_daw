// src/engine_main.rs

use std::sync::{Arc, Mutex};
use std::time::Duration;

use daw_modules::audio::setup_output_device;
use daw_modules::engine::{Engine, TrackId};
use cpal::traits::{DeviceTrait, StreamTrait};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{enable_raw_mode, disable_raw_mode},
};

fn main() -> Result<(), anyhow::Error> {
    // 1) Setup engine and stream
    let (engine, id1, id2) = init_engine_with_tracks()?;
    let _stream = start_engine_stream(engine.clone())?;

    // 2) Run keyboard loop
    enable_raw_mode()?;
    println!("Engine player:");
    println!("  SPACE = Play/Pause");
    println!("  Q     = Quit");
    println!("  1     = Mute/unmute Track 1");
    println!("  2     = Mute/unmute Track 2");
    println!("  S     = Solo Track 1");
    println!("  D     = Solo Track 2");

    run_input_loop(engine, id1, id2)?;

    disable_raw_mode()?;
    Ok(())
}

fn init_engine_with_tracks() -> Result<(Arc<Mutex<Engine>>, TrackId, TrackId), anyhow::Error> {
    let output = setup_output_device()?;
    let sample_rate = output.output_sample_rate;
    let channels = output.output_channels;

    let engine = Arc::new(Mutex::new(Engine::new(sample_rate, channels)));

    let id1;
    let id2;
    {
        let mut eng = engine.lock().unwrap();

        // Use real file names that exist in the run directory
        id1 = eng.add_track("song.wav".to_string())?;
        id2 = eng.add_track("song2.mp3".to_string())?;

        // Per‑track gain/pan
        if let Some(t1) = eng.tracks_mut().iter_mut().find(|t| t.id == id1) {
            t1.gain = 0.8;
            t1.pan = -0.5;
        }
        if let Some(t2) = eng.tracks_mut().iter_mut().find(|t| t.id == id2) {
            t2.gain = 0.8;
            t2.pan = 0.5;
        }

        eng.play();
    }

    Ok((engine, id1, id2))
}

fn start_engine_stream(engine: Arc<Mutex<Engine>>) -> Result<cpal::Stream, anyhow::Error> {
    let output = setup_output_device()?;
    let device = output.device;
    let config = output.config;
    let err_fn = |err| eprintln!("Engine output error: {err}");
    let engine_cb = engine.clone();

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
    Ok(stream)
}

fn run_input_loop(
    engine: Arc<Mutex<Engine>>,
    id1: TrackId,
    id2: TrackId,
) -> Result<(), anyhow::Error> {
    loop {
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(ev) = event::read()? {
                if ev.kind != KeyEventKind::Press {
                    continue;
                }

                match ev.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        break;
                    }
                    KeyCode::Char(' ') => {
                        let mut eng = engine.lock().unwrap();
                        if eng.transport.playing {
                            eng.pause();
                            println!("⏸️ Engine paused");
                        } else {
                            eng.play();
                            println!("▶️ Engine playing");
                        }
                    }
                    KeyCode::Char('1') => {
                        toggle_mute_for_track(&engine, id1);
                    }
                    KeyCode::Char('2') => {
                        toggle_mute_for_track(&engine, id2);
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        solo_track(&engine, id1);
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        solo_track(&engine, id2);
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn toggle_mute_for_track(engine: &Arc<Mutex<Engine>>, id: TrackId) {
    let mut eng = engine.lock().unwrap();
    if let Some(track) = eng.tracks_mut().iter_mut().find(|t| t.id == id) {
        track.muted = !track.muted;
        println!(
            "Track {:?} mute: {}",
            id.0,
            if track.muted { "ON" } else { "OFF" }
        );
    }
}

fn solo_track(engine: &Arc<Mutex<Engine>>, solo_id: TrackId) {
    let mut eng = engine.lock().unwrap();
    for track in eng.tracks_mut().iter_mut() {
        if track.id == solo_id {
            track.solo = true;
            track.muted = false;
        } else {
            // Simple solo model: mute others
            track.solo = false;
            track.muted = true;
        }
    }
    println!("Soloing track {:?}", solo_id.0);
}
