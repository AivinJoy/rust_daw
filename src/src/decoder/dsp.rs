// src/decoder/dsp.rs

pub fn append_interleaved_to_planar(
    interleaved: &[f32],
    planar: &mut [Vec<f32>],
    channels: usize,
) {
    let frames = interleaved.len() / channels;
    for f in 0..frames {
        let row = &interleaved[f * channels..(f + 1) * channels];
        for ch in 0..channels {
            planar[ch].push(row[ch]);
        }
    }
}

pub fn planar_len(planar: &[Vec<f32>]) -> usize {
    planar.iter().map(|v| v.len()).min().unwrap_or(0)
}

pub fn take_from_planar(planar: &mut [Vec<f32>], frames: usize) -> Vec<Vec<f32>> {
    let channels = planar.len();
    let mut out = Vec::with_capacity(channels);
    for ch in 0..channels {
        let n = frames.min(planar[ch].len());
        let tail = planar[ch].split_off(n);
        let head = std::mem::replace(&mut planar[ch], tail);
        out.push(head);
    }
    out
}

pub fn interleave(planar: &mut [Vec<f32>]) -> Vec<f32> {
    let channels = planar.len();
    if channels == 0 {
        return Vec::new();
    }
    let frames = planar[0].len();
    let mut out = vec![0.0f32; frames * channels];
    for f in 0..frames {
        for ch in 0..channels {
            out[f * channels + ch] = planar[ch][f];
        }
    }
    out
}

pub fn updown_mix_interleaved(input: &[f32], in_ch: usize, out_ch: usize) -> Vec<f32> {
    if in_ch == out_ch {
        return input.to_vec();
    }
    let frames = input.len() / in_ch;
    let mut out = vec![0.0f32; frames * out_ch];

    match (in_ch, out_ch) {
        (1, 2) => {
            for f in 0..frames {
                let m = input[f];
                out[f * 2] = m;
                out[f * 2 + 1] = m;
            }
        }
        (2, 1) => {
            for f in 0..frames {
                let l = input[f * 2];
                let r = input[f * 2 + 1];
                out[f] = 0.5 * (l + r);
            }
        }
        _ if out_ch < in_ch => {
            let factor = in_ch as f32 / out_ch as f32;
            for f in 0..frames {
                for oc in 0..out_ch {
                    let start = (oc as f32 * factor).floor() as usize;
                    let end = (((oc + 1) as f32 * factor).ceil() as usize).min(in_ch);
                    let mut acc = 0.0f32;
                    let mut n = 0usize;
                    for ic in start..end {
                        acc += input[f * in_ch + ic];
                        n += 1;
                    }
                    out[f * out_ch + oc] = if n > 0 { acc / n as f32 } else { 0.0 };
                }
            }
        }
        _ => {
            for f in 0..frames {
                for oc in 0..out_ch {
                    let ic = oc % in_ch;
                    out[f * out_ch + oc] = input[f * in_ch + ic];
                }
            }
        }
    }

    out
}

#[inline]
pub fn fade_samples_ms(sample_rate: u32, ms: u32) -> usize {
    ((sample_rate as u64 * ms as u64) / 1000) as usize
}
