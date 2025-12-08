// src/main.rs

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::time::{Duration, Instant};

use daw_modules::daw_controller::{DawController, DawMode};

fn main() -> Result<(), anyhow::Error> {
    let args: Vec<String> = std::env::args().collect();
    let (mode, track_path1, track_path2) = if args.len() > 2 {
        // two paths: backing + second track
        (DawMode::KaraokeRecord, Some(args[1].clone()), Some(args[2].clone()))
    } else if args.len() > 1 {
        // only one path: backing track
        (DawMode::KaraokeRecord, Some(args[1].clone()), None)
    } else {
        (DawMode::RecordOnly, None, None)
    };

    let mut daw = DawController::new(mode, track_path1, track_path2)?;
    // let mut daw = DawController::new_with_engine(mode, track_path)?;


    println!("Press [R] Record | [SPACE] Play/Pause | [L] Monitor toggle | [Q] Quit");

    enable_raw_mode()?;

    // Target 20 FPS (50ms per frame)
    let target_frame_duration = Duration::from_millis(50);
    
    // Initial draw
    daw.run_tick()?;

    loop {
        let start_time = Instant::now();

        // 1. Process Input
        // Calculate remaining time for this frame to keep consistent FPS
        // We poll for the *entire* duration of the frame if necessary.
        if event::poll(target_frame_duration)? {
            if let Event::Key(ev) = event::read()? {
                if ev.kind == KeyEventKind::Press {
                    if ev.code == KeyCode::Char('c')
                        && ev.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }

                    if daw.should_quit(ev.code) {
                        break;
                    }

                    daw.handle_key(ev.code, ev.modifiers);
                    // Force an immediate tick update on input for responsiveness
                    daw.run_tick()?; 
                    continue; 
                }
            }
        }

        // 2. Update Game Loop / UI
        // We run this if the poll timed out (meaning frame time elapsed)
        // or if we processed an input (and continue looped above).
        // Since we are not using 'continue' for timeout, we run this now.
        
        let elapsed = start_time.elapsed();
        if elapsed < target_frame_duration {
             // If processing took less time than the frame duration, 
             // poll took care of the waiting. We don't need extra sleep.
        }

        daw.run_tick()?;
    }

    disable_raw_mode()?;
    println!("\n🛑 Exiting DAW.");
    Ok(())
}