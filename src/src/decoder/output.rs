// src/decoder/output.rs

use ringbuf::traits::Producer as RbProducer;
use std::time::Duration;

pub fn push_with_fade<P: RbProducer<Item = f32>>(
    producer: &mut P,
    data: &[f32],
    post_seek_fade_samples: &mut usize,
) {
    let mut idx = 0usize;

    if *post_seek_fade_samples > 0 && idx < data.len() {
        let n = (*post_seek_fade_samples).min(data.len() - idx);
        for i in 0..n {
            let ramp = (i as f32) / (n as f32);
            let s = data[idx + i] * ramp;
            loop {
                match producer.try_push(s) {
                    Ok(()) => break,
                    Err(_) => std::thread::park_timeout(Duration::from_micros(200)),
                }
            }
        }
        *post_seek_fade_samples -= n;
        idx += n;
    }

    while idx < data.len() {
        match producer.try_push(data[idx]) {
            Ok(()) => idx += 1,
            Err(_) => std::thread::park_timeout(Duration::from_micros(200)),
        }
    }
}
