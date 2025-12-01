// src/daw_controller.rs

use std::fmt::Write as FmtWrite;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::KeyCode;
use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate},
};


use crate::audio_runtime::AudioRuntime;
use crate::Recorder;
use crate::AudioPlayer; // used only to probe duration
use crate::Waveform;
use crate::session::export::export_project_to_wav;
use crate::session::serialization::ProjectManifest; // If needed, or we just let save handle it.

pub enum DawMode {
    RecordOnly,
    KaraokeRecord,
}

pub struct DawController {
    pub mode: DawMode,

    // Central audio backend: Engine + CPAL stream
    audio: Option<AudioRuntime>,

    second_track_path: Option<String>,
    pub recorder: Option<Recorder>,
    pub total_duration: Duration,

    // Precomputed waveform for uploaded track
    pub precomputed_waveform: Option<(Vec<f32>, Vec<f32>)>,

    // --- OPTIMIZATION STATE ---
    cached_play_secs: u64,
    cached_rec_secs: u64,
    cached_waveform_len: usize,
    waveform_drawn: bool,
    force_redraw: bool,

    // The Grid Cache: 20 lines of text representing the visual waveform.
    ascii_grid: Vec<String>,

    // Reusable buffer for CLI output.
    draw_buffer: String,
}

impl DawController {
    pub fn new(
        mode: DawMode, 
        track_path1: Option<String>, 
        track_path2: Option<String>,) -> Result<Self, anyhow::Error> {
        // 1) Create AudioRuntime (Engine + CPAL stream), optionally with one track
        let audio = AudioRuntime::new(track_path1.clone())?;

        // 2) Probe total duration using AudioPlayer once (then drop it)
        let total_duration = if let Some(path) = track_path1.as_ref() {
            if let Ok(p) = AudioPlayer::new(path) {
                p.get_total_duration()
            } else {
                Duration::ZERO
            }
        } else {
            Duration::ZERO
        };

        // 3) Precompute waveform if track is provided
        let precomputed_waveform = if let Some(path) = track_path1.as_ref() {
            if let Ok(wf) = Waveform::build_from_path(path, 512) {
                let spp = (wf.sample_rate as f64) / 60.0;
                let (mins, maxs, _lvl) = wf.bins_for(spp, 0, 0, 120);
                Some((mins.to_vec(), maxs.to_vec()))
            } else {
                None
            }
        } else {
            None
        };

        let ascii_grid = vec![String::with_capacity(120); 20];

        Ok(Self {
            mode,
            audio: Some(audio),
            second_track_path: track_path2,
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

        // 2. Gather current playback time
        let curr_time = self.current_time();
        let curr_secs = curr_time.as_secs();

        let _ = write!(self.draw_buffer,"\n");
        self.render_track_status();

        let (is_recording, rec_secs, wf_len) = if let Some(rec) = &self.recorder {
            let len = if let Ok(guard) = rec.live_waveform().try_lock() {
                guard.len()
            } else {
                self.cached_waveform_len
            };
            (true, rec.get_record_time().as_secs(), len)
        } else {
            (false, 0, 0)
        };

        // 3. Dirty check
        let time_changed = curr_secs != self.cached_play_secs || rec_secs != self.cached_rec_secs;
        let wf_changed = is_recording && (wf_len != self.cached_waveform_len);
        let static_wf_needs_draw = self.precomputed_waveform.is_some() && !self.waveform_drawn;

        if !time_changed && !wf_changed && !static_wf_needs_draw && !self.force_redraw {
            return Ok(());
        }

        // 4. Update cache
        self.cached_play_secs = curr_secs;
        self.cached_rec_secs = rec_secs;
        self.cached_waveform_len = wf_len;
        self.force_redraw = false;

        // 5. Build output buffer
        self.draw_buffer.clear();

        if wf_changed || static_wf_needs_draw {
            self.update_ascii_grid();
            self.waveform_drawn = true;
        }

        // Move cursor top-left
        let _ = write!(self.draw_buffer, "{}", MoveTo(0, 0));

        // Print waveform lines
        for line in &self.ascii_grid {
            let _ = write!(self.draw_buffer, "{}\x1b[K\n", line);
        }

        // Status line
        let _ = write!(self.draw_buffer, "{}", MoveTo(0, 20));
        let _ = write!(self.draw_buffer, "{}", Clear(ClearType::UntilNewLine));

        let total = self.total_duration;
        let _ = write!(
            self.draw_buffer,
            "🎵 Time: {:02}:{:02} / {:02}:{:02}",
            curr_secs / 60,
            curr_secs % 60,
            total.as_secs() / 60,
            total.as_secs() % 60
        );

        if is_recording {
            let _ = write!(
                self.draw_buffer,
                " 🔴 REC {:02}:{:02}",
                rec_secs / 60,
                rec_secs % 60
            );
        }

        // 6. Flush to terminal
        let mut stdout = stdout();
        execute!(stdout, BeginSynchronizedUpdate)?;
        stdout.write_all(self.draw_buffer.as_bytes())?;
        execute!(stdout, EndSynchronizedUpdate)?;
        stdout.flush()?;

        Ok(())
    }

    fn current_time(&self) -> Duration {
        if let Some(audio) = &self.audio {
            audio.position()
        } else {
            Duration::ZERO
        }
    }

    fn is_playing(&self) -> bool {
        if let Some(audio) = &self.audio {
            audio.is_playing()
        } else {
            false
        }
    }

    fn toggle_play_pause(&mut self) {
        if let Some(audio) = &self.audio {
            audio.toggle_play();
        }
    }

    fn adjust_volume(&mut self, delta: f32) {
    if let Some(audio) = &self.audio {
        let current = audio.master_gain();
        let new = (current + delta).clamp(0.0, 2.0);
        audio.set_master_gain(new);
        println!("Volume: {:.0}%", new * 100.0);
    }
}


    fn seek_by_secs(&mut self, delta: i64) {
        if let Some(audio) = &self.audio {
            let cur = audio.position().as_secs_f64();
            let tgt = (cur + delta as f64).max(0.0);
            audio.seek(Duration::from_secs_f64(tgt));
        }
    }

    fn total_duration_backend(&self) -> Duration {
        self.total_duration
    }

    fn update_ascii_grid(&mut self) {
        let (mins, maxs) = if let Some(rec) = &self.recorder {
            if let Ok(guard) = rec.live_waveform().lock() {
                guard.snapshot()
            } else {
                return;
            }
        } else if let Some((m, x)) = &self.precomputed_waveform {
            (m.clone(), x.clone())
        } else {
            return;
        };

        if mins.is_empty() {
            return;
        }

        for line in &mut self.ascii_grid {
            line.clear();
        }

        let cols = 120;
        let len = mins.len();
        let start_index = len.saturating_sub(cols);
        let visible_mins = &mins[start_index..];
        let visible_maxs = &maxs[start_index..];
        let height = 20;

        for i in 0..visible_mins.len() {
            let min = visible_mins[i];
            let max = visible_maxs[i];

            let n_min = (min + 1.0) / 2.0;
            let n_max = (max + 1.0) / 2.0;

            let start_row = (n_min * height as f32).floor() as usize;
            let end_row = (n_max * height as f32).ceil() as usize;

            for row in 0..height {
                let visual_y = height - 1 - row;

                let ch = if visual_y >= start_row && visual_y < end_row {
                    '│'
                } else if visual_y == height / 2 {
                    '─'
                } else {
                    ' '
                };

                self.ascii_grid[row].push(ch);
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

    pub fn handle_key(&mut self, key: KeyCode, modifiers: crossterm::event::KeyModifiers) {
        // 1. Priority: Handle Global Shortcuts (Undo/Redo, Save, Load)
        if self.handle_global_shortcuts(key, modifiers) {
            return; // Shortcut consumed the event, stop processing.
        }

        // 2. Normal handlers
        self.handle_playback_keys(key);
        self.handle_record_keys(key);
        self.handle_monitor_keys(key);
    }

    /// Returns true if a global shortcut was executed.
    /// Returns true if a global shortcut was executed.
    fn handle_global_shortcuts(&mut self, key: KeyCode, modifiers: crossterm::event::KeyModifiers) -> bool {
        // All global shortcuts require CONTROL
        if !modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
            return false;
        }

        match key {
            // [CTRL + Z] => UNDO
            KeyCode::Char('z') | KeyCode::Char('Z') => {
                self.undo();
                true
            }
            // [CTRL + Y] => REDO
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.redo();
                true
            }

            // [CTRL + S] => SAVE
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if let Some(audio) = &self.audio {
                    if let Err(e) = audio.save_session("project.json") {
                        println!("Error saving: {}", e);
                    }
                }
                true
            }

            // [CTRL + B] => BOUNCE (EXPORT)
            KeyCode::Char('b') | KeyCode::Char('B') => {
                 if let Some(audio) = &self.audio {
                    // 1. Auto-save to ensure we export current state
                    let _ = audio.save_session("project.json");
                    
                    // 2. Load manifest from disk
                    if let Ok(manifest) = ProjectManifest::load_from_disk("project.json") {
                        // 3. Run Export
                        if let Err(e) = export_project_to_wav(&manifest, "mixdown.wav") {
                            println!("Export failed: {}", e);
                        }
                    }
                 }
                 true
            }

            // [CTRL + O] => OPEN / LOAD
            KeyCode::Char('o') | KeyCode::Char('O') => {
                if let Some(audio) = &self.audio {
                    if let Err(e) = audio.load_session("project.json") {
                        println!("Error loading: {}", e);
                    } else {
                        self.force_redraw = true; 
                    }
                }
                true
            }

            _ => false,
        }
    }

    fn undo(&mut self) {
        if let Some(audio) = &self.audio {
            audio.undo();
            // Force redraw to show the slider jumping back
            self.force_redraw = true; 
        }
    }

    fn redo(&mut self) {
        if let Some(audio) = &self.audio {
            audio.redo();
            self.force_redraw = true;
        }
    }

    pub fn should_quit(&self, key: KeyCode) -> bool {
        matches!(key, KeyCode::Char('q') | KeyCode::Char('Q'))
    }

    pub fn tick(&mut self) {
        if self.is_playing() && self.current_time() >= self.total_duration_backend() {
            self.force_redraw = true;
            println!("\n🎵 Track finished.");
            if let Some(r) = self.recorder.take() {
                r.stop();
                println!("⏹️ Recording stopped.");
            }
        }
    }

    fn add_second_track(&mut self) {
        if let Some(audio) = &self.audio {
            if let Some(path) = &self.second_track_path {
                if let Err(e) = audio.add_track(path.clone()) {
                    println!("\n❌ Failed to add second track: {e}");
                } else {
                    println!("\n➕ Added second track: {}", path);
                }
            } else {
                println!("\nℹ️ No second track path provided on the command line.");
            }
        }
    }

    fn mute_track(&mut self, idx: usize) {
        if let Some(audio) = &self.audio {
            audio.toggle_mute(idx);
        }
    }

    fn solo_track(&mut self, idx: usize) {
        if let Some(audio) = &self.audio {
            audio.solo_track(idx);
        }
    }

    fn clear_solo(&mut self) {
        if let Some(audio) = &self.audio {
            audio.clear_solo();
        }
    }

    fn adjust_track1_gain(&mut self, delta: f32) {
        if let Some(audio) = &self.audio {
            audio.adjust_track_gain(0, delta);
        }
    }

    fn adjust_track2_gain(&mut self, delta: f32) {
        if let Some(audio) = &self.audio {
            audio.adjust_track_gain(1, delta);
        }
    }

    fn adjust_track1_pan(&mut self, delta: f32) {
        if let Some(audio) = &self.audio {
            audio.adjust_track_pan(0, delta);
        }
    }

    fn adjust_track2_pan(&mut self, delta: f32) {
        if let Some(audio) = &self.audio {
            audio.adjust_track_pan(1, delta);
        }
    }

        fn reset_track1_gain(&mut self) {
        if let Some(audio) = &self.audio {
            audio.reset_track_gain(0);
        }
    }

    fn reset_track2_gain(&mut self) {
        if let Some(audio) = &self.audio {
            audio.reset_track_gain(1);
        }
    }

    fn reset_track1_pan(&mut self) {
        if let Some(audio) = &self.audio {
            audio.reset_track_pan(0);
        }
    }

    fn reset_track2_pan(&mut self) {
        if let Some(audio) = &self.audio {
            audio.reset_track_pan(1);
        }
    }

    fn render_track_status(&mut self) {
        if let Some(audio) = &self.audio {
            if let Some(engine) = audio.debug_snapshot() {
                // engine.tracks is a Vec<TrackSnapshot>
                for (i, t) in engine.tracks.iter().enumerate() {
                    let _ = write!(
                        self.draw_buffer,
                        "\nTr{} [{}{}] gain:{:>3}% pan:{:>4}",
                        i + 1,
                        if t.muted { "M" } else { "-" },
                        if t.solo  { "S" } else { "-" },
                        (t.gain * 100.0).round() as i32,
                        format!("{:.2}", t.pan),
                    );
                }
            }
        }
    }



    // -------------------------------------------------------------
    // Playback keys
    // -------------------------------------------------------------
    pub fn handle_playback_keys(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(' ') => self.toggle_play_pause(),
            KeyCode::Up => self.adjust_volume(0.1),
            KeyCode::Down => self.adjust_volume(-0.1),
            KeyCode::Right => self.seek_by_secs(5),
            KeyCode::Left => self.seek_by_secs(-5),
            KeyCode::Char('t') | KeyCode::Char('T') => self.add_second_track(),
            // Add these:
            KeyCode::Char('1') => self.mute_track(0),      // mute/unmute track 1
            KeyCode::Char('2') => self.mute_track(1),      // mute/unmute track 2
            KeyCode::Char('s') | KeyCode::Char('S') => self.solo_track(0), // solo track 1
            KeyCode::Char('d') | KeyCode::Char('D') => self.solo_track(1), // solo track 2
            KeyCode::Char('c') | KeyCode::Char('C') => self.clear_solo(),  // clear solo

            // Track 1 gain: Z/X, reset: Q
            KeyCode::Char('z') | KeyCode::Char('Z') => self.adjust_track1_gain(-0.1),
            KeyCode::Char('x') | KeyCode::Char('X') => self.adjust_track1_gain(0.1),
            KeyCode::Char('q') | KeyCode::Char('Q') => self.reset_track1_gain(),

            // Track 2 gain: B/N, reset: W
            KeyCode::Char('b') | KeyCode::Char('B') => self.adjust_track2_gain(-0.1),
            KeyCode::Char('n') | KeyCode::Char('N') => self.adjust_track2_gain(0.1),
            KeyCode::Char('w') | KeyCode::Char('W') => self.reset_track2_gain(),

            // Track 1 pan: A/F, reset: E
            KeyCode::Char('a') | KeyCode::Char('A') => self.adjust_track1_pan(-0.1),
            KeyCode::Char('f') | KeyCode::Char('F') => self.adjust_track1_pan(0.1),
            KeyCode::Char('e') | KeyCode::Char('E') => self.reset_track1_pan(),

            // Track 2 pan: G/H, reset: R
            KeyCode::Char('g') | KeyCode::Char('G') => self.adjust_track2_pan(-0.1),
            KeyCode::Char('h') | KeyCode::Char('H') => self.adjust_track2_pan(0.1),
            KeyCode::Char('j') | KeyCode::Char('J') => self.reset_track2_pan(),
            _ => {}
        }
    }  

}