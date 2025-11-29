// src/decoder/resample.rs

use anyhow::Result;
use rubato::{
    calculate_cutoff, Resampler, SincFixedIn, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};
use crate::decoder::dsp;

pub fn build_resampler(
    src_rate: u32,
    dst_rate: u32,
    channels: usize,
) -> Result<Option<SincFixedIn<f32>>> {
    if src_rate == dst_rate {
        return Ok(None);
    }
    let ratio = dst_rate as f64 / src_rate as f64;
    let sinc_len = 256usize;
    let window = WindowFunction::BlackmanHarris2;
    let f_cutoff = calculate_cutoff(sinc_len, window);
    let params = SincInterpolationParameters {
        sinc_len,
        f_cutoff,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 128,
        window,
    };
    let chunk_size = 1024;
    let r = SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, channels)?;
    Ok(Some(r))
}

pub fn try_process_exact(
    resampler: &mut SincFixedIn<f32>,
    stage_planar: &mut [Vec<f32>],
) -> Option<Vec<Vec<f32>>> {
    let need = resampler.input_frames_next();
    let have = dsp::planar_len(stage_planar);
    if have < need {
        return None;
    }
    let in_block = dsp::take_from_planar(stage_planar, need);
    let out = resampler.process(&in_block, None).ok()?;
    Some(out)
}

pub fn drain_remaining_planar(stage_planar: &mut [Vec<f32>]) -> Option<Vec<Vec<f32>>> {
    let have = dsp::planar_len(stage_planar);
    if have > 0 {
        Some(dsp::take_from_planar(stage_planar, have))
    } else {
        None
    }
}

pub fn process_partial_some(
    resampler: &mut SincFixedIn<f32>,
    in_block: &mut [Vec<f32>],
) -> Result<Option<Vec<Vec<f32>>>> {
    let out = resampler.process_partial(Some(&*in_block), None)?;
    Ok(Some(out))
}

pub fn process_partial_none<T>(
    resampler: &mut SincFixedIn<T>,
) -> Result<Option<Vec<Vec<T>>>>
where
    T: rubato::Sample + Send + Sync,
{
    let out = resampler.process_partial::<Vec<T>>(None, None)?;
    Ok(Some(out))
}
