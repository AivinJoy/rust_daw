// src/bpm/utils.rs
use std::f32::consts::PI;

pub fn hann_window(n: usize) -> Vec<f32> {
    (0..n).map(|i| {
        0.5 * (1.0 - (2.0 * PI * i as f32 / (n as f32)).cos())
    }).collect()
}

pub fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 { return interleaved.to_vec(); }
    let frames = interleaved.len() / channels;
    let mut out = Vec::with_capacity(frames);
    for chunk in interleaved.chunks_exact(channels) {
        let mut s = 0.0f32;
        for &c in chunk { s += c; }
        out.push(s / channels as f32);
    }
    out
}

pub fn moving_average_inplace(x: &mut [f32], radius: usize) {
    if radius == 0 { return; }
    let n = x.len();
    let mut out = vec![0.0f32; n];
    let _window = radius * 2 + 1;
    let mut sum = 0.0f32;
    for i in 0..n {
        if i == 0 {
            for j in 0..=radius {
                if j < n { sum += x[j]; }
            }
        } else {
            let add = i + radius;
            if add < n { sum += x[add]; }
            let sub = if i > radius { i - radius - 1 } else { usize::MAX };
            if sub != usize::MAX { sum -= x[sub]; }
        }
        let left = if i >= radius { i - radius } else { 0 };
        let right = (i + radius).min(n - 1);
        let count = (right - left + 1) as f32;
        out[i] = sum / count;
    }
    x.copy_from_slice(&out);
}
