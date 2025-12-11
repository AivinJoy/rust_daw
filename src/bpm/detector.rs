// src/bpm/detector.rs
use rustfft::{FftPlanner, num_complex::Complex, num_traits::Zero};
use std::collections::HashMap;
use crate::bpm::utils::{hann_window, downmix_to_mono, moving_average_inplace};

#[derive(Debug, Clone)]
pub struct BpmResult {
    pub bpm: f32,
    pub confidence: f32,
    pub candidates: Vec<(f32, f32)>,
    pub beat_times: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct BpmOptions {
    pub window_size: usize,
    pub hop_size: usize,
    pub env_rate: f32,
    pub min_bpm: f32,
    pub max_bpm: f32,
    pub band_count: usize,
    pub compute_beats: bool,
    pub silence_threshold: f32,
}

impl Default for BpmOptions {
    fn default() -> Self {
        Self {
            window_size: 2048,
            hop_size: 512,
            env_rate: 0.0,
            min_bpm: 40.0,
            max_bpm: 240.0,
            band_count: 3,
            compute_beats: true,
            silence_threshold: 1e-5,
        }
    }
}

pub struct BpmDetector {
    planner: FftPlanner<f32>,
    window: Vec<f32>,
}

impl BpmDetector {
    pub fn new(window_size: usize) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let _ = planner.plan_fft_forward(window_size); // warm-up
        Self {
            planner,
            window: hann_window(window_size),
        }
    }

    pub fn detect(
        &mut self,
        audio: &[f32],
        channels: usize,
        sample_rate: u32,
        opts: BpmOptions,
    ) -> Option<BpmResult> {
        if channels == 0 || audio.is_empty() { return None; }
        // quick RMS check
        let rms = quick_rms(audio, channels);
        if rms < opts.silence_threshold { return None; }

        // derived params
        let window_size = opts.window_size.next_power_of_two();
        let hop = opts.hop_size.max(1);
        let env_rate = if opts.env_rate > 0.0 { opts.env_rate } else { sample_rate as f32 / hop as f32 };

        // downmix
        let mono = downmix_to_mono(audio, channels);

        // stft mags
        let mag_frames = compute_spectrogram(&mono, sample_rate as usize, window_size, hop, &mut self.planner, &self.window);
        if mag_frames.len() < 4 { return None; }

        // novelty: multi-band flux
        let mut novelty = multi_band_flux(&mag_frames, opts.band_count);
        if novelty.len() < 8 { return None; }

        // smooth & normalize
        moving_average_inplace(&mut novelty, 3);
        let norm = normalize_peak(&novelty);

        // autocorr by FFT
        let (lag_min, lag_max) = bpm_range_to_lag_range(opts.min_bpm, opts.max_bpm, env_rate);
        if lag_max <= lag_min + 2 { return None; }
        let lag_scores = autocorrelate_range_fft(&norm, lag_min, lag_max, &mut self.planner);

        // fold
        let folded = fold_bpm_candidates(&lag_scores, env_rate, 60.0, 200.0);
        if folded.is_empty() { return None; }

        // candidate vec
        let mut cand_vec: Vec<(f32, f32)> = folded.into_iter().map(|(k, v)| (k as f32 / 10.0, v)).collect();
        cand_vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // comb refine top N
        let top_n = cand_vec.len().min(6);
        let mut refined = Vec::with_capacity(top_n);
        for i in 0..top_n {
            let (bpm, base_score) = cand_vec[i];
            let score = comb_score(&norm, bpm, env_rate);
            refined.push((bpm, score + 0.05 * base_score));
        }
        refined.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let primary = refined[0];
        let confidence = confidence_from_candidates(&refined);

        // beats
        let beat_times = if opts.compute_beats {
            compute_beats_from_novelty(&norm, primary.0, env_rate, hop, sample_rate as usize)
        } else { Vec::new() };

        Some(BpmResult {
            bpm: (primary.0 * 100.0).round() / 100.0,
            confidence,
            candidates: refined,
            beat_times,
        })
    }
}

// ---------- Helper functions (same approach as earlier) ----------

fn quick_rms(audio: &[f32], channels: usize) -> f32 {
    let mut acc = 0.0f64;
    let mut cnt = 0usize;
    for chunk in audio.chunks_exact(channels) {
        let mut s = 0.0f32;
        for &c in chunk { s += c; }
        let mono = s / channels as f32;
        acc += (mono as f64) * (mono as f64);
        cnt += 1;
    }
    if cnt == 0 { return 0.0; }
    ((acc / cnt as f64) as f32).sqrt()
}

fn compute_spectrogram(
    mono: &[f32],
    _sample_rate: usize,
    window_size: usize,
    hop: usize,
    planner: &mut FftPlanner<f32>,
    window: &[f32],
) -> Vec<Vec<f32>> {
    let n = window_size;
    let half = n / 2 + 1;
    let fft = planner.plan_fft_forward(n);
    let mut mag_frames: Vec<Vec<f32>> = Vec::new();

    let mut pos = 0usize;
    let mut inbuf: Vec<Complex<f32>> = vec![Complex::zero(); n];
    while pos + n <= mono.len() {
        for k in 0..n {
            inbuf[k].re = mono[pos + k] * window[k];
            inbuf[k].im = 0.0;
        }
        fft.process(&mut inbuf);
        let mut mag = vec![0.0f32; half];
        for b in 0..half {
            mag[b] = inbuf[b].norm();
        }
        mag_frames.push(mag);
        pos += hop;
    }
    mag_frames
}

fn multi_band_flux(mag_frames: &Vec<Vec<f32>>, band_count: usize) -> Vec<f32> {
    if mag_frames.len() < 2 { return vec![]; }
    let bins = mag_frames[0].len();
    let mut novelty = vec![0.0f32; mag_frames.len()];
    let mut band_edges = Vec::with_capacity(band_count + 1);
    for i in 0..=band_count {
        let edge = ((i as f32 / band_count as f32) * (bins as f32)).round() as usize;
        band_edges.push(edge.min(bins));
    }
    for t in 1..mag_frames.len() {
        let prev = &mag_frames[t - 1];
        let cur = &mag_frames[t];
        let mut sum_flux = 0.0f32;
        for bidx in 0..band_count {
            let start = band_edges[bidx];
            let end = band_edges[bidx + 1];
            let mut band_flux = 0.0f32;
            for k in start..end {
                let diff = cur[k] - prev[k];
                if diff > 0.0 { band_flux += diff; }
            }
            let weight = match bidx {
                0 => 1.0,
                1 => 1.0,
                _ => 0.8,
            };
            sum_flux += weight * band_flux;
        }
        novelty[t] = sum_flux;
    }
    let maxv = novelty.iter().cloned().fold(0./0., f32::max);
    if maxv > 0.0 {
        for v in &mut novelty { *v /= maxv; }
    }
    novelty
}

fn normalize_peak(x: &[f32]) -> Vec<f32> {
    if x.is_empty() { return vec![]; }
    let mean = x.iter().sum::<f32>() / x.len() as f32;
    let mut max_abs = 0.0f32;
    for &v in x {
        let a = (v - mean).abs();
        if a > max_abs { max_abs = a; }
    }
    if max_abs == 0.0 { return vec![0.0f32; x.len()]; }
    x.iter().map(|&v| (v - mean) / max_abs).collect()
}

// FFT autocorr
fn autocorrelate_range_fft(x: &[f32], lag_min: usize, lag_max: usize, planner: &mut FftPlanner<f32>) -> Vec<(usize, f32)> {
    let n = x.len();
    if n == 0 || lag_max < lag_min || lag_min >= n { return Vec::new(); }
    let mut conv = 1usize;
    while conv < (n * 2) { conv <<= 1; }
    let fft = planner.plan_fft_forward(conv);
    let ifft = planner.plan_fft_inverse(conv);
    let mut buf: Vec<Complex<f32>> = vec![Complex::zero(); conv];
    for i in 0..n { buf[i].re = x[i]; buf[i].im = 0.0; }
    fft.process(&mut buf);
    for v in buf.iter_mut() {
        let re = v.re; let im = v.im;
        *v = Complex { re: re * re + im * im, im: 0.0 };
    }
    ifft.process(&mut buf);
    let scale = 1.0 / conv as f32;
    let mut out = Vec::with_capacity(lag_max.saturating_sub(lag_min) + 1);
    for lag in lag_min..=lag_max {
        if lag >= n { out.push((lag, 0.0)); continue; }
        let ac = buf[lag].re * scale;
        let denom = (n - lag) as f32;
        let norm_score = if denom > 0.0 { ac / denom } else { 0.0 };
        out.push((lag, norm_score));
    }
    out
}

fn fold_bpm_candidates(lag_scores: &[(usize, f32)], env_rate: f32, pref_min: f32, pref_max: f32) -> HashMap<i32, f32> {
    let mut map: HashMap<i32, f32> = HashMap::new();
    for &(lag, score) in lag_scores {
        if score <= 0.0 { continue; }
        let bpm_raw = 60.0 * env_rate / (lag as f32);
        if !bpm_raw.is_finite() || bpm_raw <= 0.0 { continue; }
        let mut bpm = bpm_raw;
        let lower = pref_min / 2.0; let upper = pref_max * 2.0;
        while bpm < lower { bpm *= 2.0; }
        while bpm > upper { bpm *= 0.5; }
        while bpm < pref_min { bpm *= 2.0; }
        while bpm > pref_max { bpm *= 0.5; }
        let key = (bpm * 10.0).round() as i32;
        *map.entry(key).or_insert(0.0) += score.max(0.0);
    }
    map
}

fn comb_score(novelty: &[f32], bpm: f32, env_rate: f32) -> f32 {
    if novelty.is_empty() || bpm <= 0.0 { return 0.0; }
    let period_sec = 60.0 / bpm;
    let frames_per_beat = period_sec * env_rate;
    if frames_per_beat < 1.0 { return 0.0; }
    let max_phase = frames_per_beat.max(1.0) as usize;
    let mut best = 0.0f32;
    for phase in 0..max_phase {
        let mut s = 0.0f32;
        let mut pos = phase as f32;
        while (pos as usize) < novelty.len() {
            s += novelty[pos as usize];
            pos += frames_per_beat;
        }
        if s > best { best = s; }
    }
    best
}

fn confidence_from_candidates(cands: &[(f32, f32)]) -> f32 {
    if cands.is_empty() { return 0.0; }
    let best = cands[0].1;
    let sum: f32 = cands.iter().map(|c| c.1).sum();
    let rel = if sum > 0.0 { best / sum } else { 0.0 };
    (rel * 1.2).min(1.0)
}

fn compute_beats_from_novelty(novelty: &[f32], bpm: f32, env_rate: f32, _hop: usize, _sample_rate: usize) -> Vec<f32> {
    let mut beats = Vec::new();
    if novelty.is_empty() || bpm <= 0.0 { return beats; }
    let period_sec = 60.0 / bpm;
    let frames_per_beat = period_sec * env_rate;
    if frames_per_beat < 1.0 { return beats; }
    let max_phase = frames_per_beat.max(1.0) as usize;
    let mut best_phase = 0usize;
    let mut best_s = -1.0f32;
    for phase in 0..max_phase {
        let mut s = 0.0f32;
        let mut pos = phase as f32;
        while (pos as usize) < novelty.len() {
            s += novelty[pos as usize];
            pos += frames_per_beat;
        }
        if s > best_s { best_s = s; best_phase = phase; }
    }
    let mut pos = best_phase as f32;
    let window_frames = ((frames_per_beat * 0.3).max(2.0)) as isize;
    while (pos as usize) < novelty.len() {
        let center = pos as isize;
        let start = (center - window_frames).max(0) as usize;
        let end = (center + window_frames).min(novelty.len() as isize - 1) as usize;
        let mut best_idx = center as usize;
        let mut best_v = novelty[best_idx];
        for i in start..=end {
            if novelty[i] > best_v { best_v = novelty[i]; best_idx = i; }
        }
        let seconds = (best_idx as f32) / env_rate;
        beats.push(seconds);
        pos += frames_per_beat;
    }
    beats
}

fn bpm_range_to_lag_range(min_bpm: f32, max_bpm: f32, env_rate: f32) -> (usize, usize) {
    let min_bpm = min_bpm.max(1.0); let max_bpm = max_bpm.max(min_bpm + 1.0);
    let lag_max = (env_rate * 60.0 / min_bpm).round() as usize;
    let lag_min = (env_rate * 60.0 / max_bpm).round() as usize;
    (lag_min.max(1), lag_max.max(lag_min + 1))
}
