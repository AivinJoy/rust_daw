// src/daw_controller.rs

use std::path::PathBuf;
use std::time::Duration;
use std::io::{stdout, Write};
use std::fmt::Write as FmtWrite; // Trait for writing to String without allocation

use crate::{AudioPlayer, Recorder};
use crossterm::event::KeyCode;
use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate},
};

pub enum DawMode {
    RecordOnly,
    KaraokeRecord,
}

pub struct DawController {
    pub mode: DawMode,
    pub player: Option<AudioPlayer>,
    pub recorder: Option<Recorder>,
    pub total_duration: Duration,

    // Precomputed waveform for uploaded track
    pub precomputed_waveform: Option<(Vec<f32>, Vec<f32>)>,

    // --- OPTIMIZATION STATE ---
    cached_play_secs: u64,
    cached_rec_secs: u64,
    
    // We cache the previous length to detect if we need to redraw
    cached_waveform_len: usize, 
    
    // Has the static waveform (from file) been drawn?
    waveform_drawn: bool,
    force_redraw: bool, 

    // The Grid Cache: 20 lines of text representing the visual waveform.
    // We update this in memory, then dump it to the terminal.
    ascii_grid: Vec<String>,

    // A single reusable memory buffer for constructing the final CLI output.
    draw_buffer: String,
}

impl DawController {
    pub fn new(mode: DawMode, track_path: Option<String>) -> Result<Self, anyhow::Error> {
        let player = AudioPlayer::try_new(track_path.as_ref().map(|s| s.as_str()))?;
        let total_duration = player
            .as_ref()
            .map_or(Duration::ZERO, |p| p.get_total_duration());

        // Precompute waveform if track is uploaded (compressed to ~120 bins)
        let precomputed_waveform = if let Some(path) = track_path.as_ref() {
            if let Ok(wf) = crate::Waveform::build_from_path(path, 512) {
                let spp = (wf.sample_rate as f64) / 60.0;
                let (mins, maxs, _lvl) = wf.bins_for(spp, 0, 0, 120);
                Some((mins.to_vec(), maxs.to_vec()))
            } else {
                None
            }
        } else {
            None
        };

        // Initialize the grid with 20 empty strings
        let ascii_grid = vec![String::with_capacity(120); 20];

        Ok(Self {
            mode,
            player,
            recorder: None,
            total_duration,
            precomputed_waveform,
            cached_play_secs: u64::MAX,
            cached_rec_secs: u64::MAX,
            cached_waveform_len: 0,
            waveform_drawn: false,
            force_redraw: true,
            ascii_grid,
            draw_buffer: String::with_capacity(4096),
        })
    }

    pub fn run_tick(&mut self) -> Result<(), anyhow::Error> {
        // 1. Logic Tick
        self.tick();

        // 2. Gather current state (Cheap atomic/primitive reads)
        let curr_time = self.player.as_ref().map_or(Duration::ZERO, |p| p.get_current_time());
        let curr_secs = curr_time.as_secs();
        
        let (is_recording, rec_secs, wf_len) = if let Some(rec) = &self.recorder {
            // Check length efficiently
            let len = if let Ok(guard) = rec.live_waveform().try_lock() {
                guard.len() 
            } else {
                self.cached_waveform_len 
            };
            (true, rec.get_record_time().as_secs(), len)
        } else {
            (false, 0, 0)
        };

        // 3. DIRTY CHECK: Has anything visually changed?
        let time_changed = curr_secs != self.cached_play_secs || rec_secs != self.cached_rec_secs;
        
        // If recording, we redraw if data length changed OR if we just started
        let wf_changed = is_recording && (wf_len != self.cached_waveform_len);
        let static_wf_needs_draw = self.precomputed_waveform.is_some() && !self.waveform_drawn;

        if !time_changed && !wf_changed && !static_wf_needs_draw && !self.force_redraw {
            return Ok(());
        }

        // 4. Update Cache Tracking
        self.cached_play_secs = curr_secs;
        self.cached_rec_secs = rec_secs;
        self.cached_waveform_len = wf_len;
        self.force_redraw = false;

        // 5. Build the Output Buffer
        self.draw_buffer.clear();

        // --- WAVEFORM UPDATE LOGIC ---
        // If waveform changed, we rebuild the ascii_grid strings
        if wf_changed || static_wf_needs_draw {
            self.update_ascii_grid();
            self.waveform_drawn = true;
        }

        // --- RENDER TO BUFFER ---
        
        // A. Move Cursor Top-Left
        let _ = write!(self.draw_buffer, "{}", MoveTo(0, 0));

        // B. Print Waveform from Grid Cache
        // We print all 20 lines from our cache.
        for line in &self.ascii_grid {
            // Print the cached line + Clear rest of line + Newline
            let _ = write!(self.draw_buffer, "{}\x1b[K\n", line);
        }
        
        // C. Status Line Rendering
        // Ensure we are below the waveform (row 20)
        let _ = write!(self.draw_buffer, "{}", MoveTo(0, 20));
        let _ = write!(self.draw_buffer, "{}", Clear(ClearType::UntilNewLine));
        
        let _ = write!(
            self.draw_buffer, 
            "🎵 Time: {:02}:{:02} / {:02}:{:02}", 
            curr_secs / 60, curr_secs % 60,
            self.total_duration.as_secs() / 60, self.total_duration.as_secs() % 60
        );

        if is_recording {
            let _ = write!(self.draw_buffer, " 🔴 REC {:02}:{:02}", rec_secs / 60, rec_secs % 60);
        }

        // 6. Final IO Flush
        let mut stdout = stdout();
        execute!(stdout, BeginSynchronizedUpdate)?;
        stdout.write_all(self.draw_buffer.as_bytes())?;
        execute!(stdout, EndSynchronizedUpdate)?;
        stdout.flush()?;

        Ok(())
    }

    /// Rebuilds the self.ascii_grid based on current data.
    /// This is where the visualization math happens.
    fn update_ascii_grid(&mut self) {
        // 1. Get the Data Slices
        // We declare local variables to hold the references/clones
        let (mins, maxs) = if let Some(rec) = &self.recorder {
            if let Ok(guard) = rec.live_waveform().lock() {
                // If you implemented the Ring Buffer logic, use guard.get_visible_bins()
                // If using the older Logic, snapshot() works but clones vectors.
                // Assuming snapshot() returns (Vec<f32>, Vec<f32>) for now:
                guard.snapshot()
            } else {
                return;
            }
        } else if let Some((m, x)) = &self.precomputed_waveform {
            (m.clone(), x.clone())
        } else {
            return;
        };

        if mins.is_empty() { return; }

        // 2. Clear current grid lines
        for line in &mut self.ascii_grid {
            line.clear();
        }

        // 3. Determine View Window (Last 120 columns)
        let cols = 120; 
        let len = mins.len();
        let start_index = len.saturating_sub(cols);
        let visible_mins = &mins[start_index..];
        let visible_maxs = &maxs[start_index..];
        let height = 20;

        // 4. Build the Grid Column by Column
        // This loop runs max 120 times.
        for i in 0..visible_mins.len() {
            let min = visible_mins[i];
            let max = visible_maxs[i];

            // Normalize audio (-1.0 to 1.0) to grid coordinates
            // 0.0 is center. -1 is bottom, +1 is top.
            // Map -1..1 to 0..height
            let n_min = (min + 1.0) / 2.0;
            let n_max = (max + 1.0) / 2.0;

            let start_row = (n_min * height as f32).floor() as usize;
            let end_row = (n_max * height as f32).ceil() as usize;

            // Fill the vertical slice
            // We iterate rows from Top (0) to Bottom (19) visually
            for row in 0..height {
                // visual_y 0 is bottom, 19 is top
                let visual_y = height - 1 - row;
                
                let char_to_add = if visual_y >= start_row && visual_y < end_row {
                    '│' // Waveform bar
                } else if visual_y == height / 2 {
                    '─' // Center line
                } else {
                    ' ' // Empty space
                };

                self.ascii_grid[row].push(char_to_add);
            }
        }
    }

    // -------------------------------------------------------------
    // Playback keys
    // -------------------------------------------------------------
    pub fn handle_playback_keys(&self, key: KeyCode) {
        if let Some(player) = &self.player {
            match key {
                KeyCode::Char(' ') => player.toggle_playback(),
                KeyCode::Up => player.set_volume((player.get_volume() + 0.1).min(1.0)),
                KeyCode::Down => player.set_volume((player.get_volume() - 0.1).max(0.0)),
                KeyCode::Right => { let _ = player.seek_by_secs(5); }
                KeyCode::Left => { let _ = player.seek_by_secs(-5); }
                _ => {}
            }
        }
    }

    // -------------------------------------------------------------
    // Record keys
    // -------------------------------------------------------------
    pub fn handle_record_keys(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if self.recorder.is_none() {
                    if let Ok(r) = Recorder::start(PathBuf::from("recording.wav")) {
                        self.recorder = Some(r);
                        self.force_redraw = true;
                        println!("\n🔴 Recording started: recording.wav");
                    }
                } else if let Some(r) = self.recorder.take() {
                    r.stop();
                    println!("\n⏹️  Recording stopped and saved.");
                }
            }
            _ => {}
        }
    }

    // -------------------------------------------------------------
    // Monitor keys
    // -------------------------------------------------------------
    pub fn handle_monitor_keys(&mut self, key: KeyCode) {
        if matches!(key, KeyCode::Char('l') | KeyCode::Char('L')) {
            if let Some(rec) = self.recorder.as_mut() {
                if let Err(e) = rec.toggle_monitor() {
                    println!("\n❌ Failed to toggle monitor: {}", e);
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        self.handle_playback_keys(key);
        self.handle_record_keys(key);
        self.handle_monitor_keys(key);
    }

    pub fn should_quit(&self, key: KeyCode) -> bool {
        matches!(key, KeyCode::Char('q') | KeyCode::Char('Q'))
    }

    pub fn tick(&mut self) {
        if let Some(player) = &self.player {
            if player.is_playing() && player.get_current_time() >= self.total_duration {
                self.force_redraw = true;
                println!("\n🎵 Track finished.");
                if let Some(r) = self.recorder.take() {
                    r.stop();
                    println!("⏹️ Recording stopped.");
                }
            }
        }
    }
}