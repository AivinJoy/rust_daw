// src/waveform/terminal.rs

pub fn render_ascii(mins: &[f32], maxs: &[f32], height: usize) -> Vec<String> {
    let h = height.max(4);
    let mut lines = vec![vec![' '; mins.len()]; h];
    let to_row = |v: f32| -> usize {
        let clamped = v.clamp(-1.0, 1.0);
        let y = (0.5 - 0.5 * clamped) * (h as f32 - 1.0);
        y.round() as usize
    };
    for x in 0..mins.len() {
        let y1 = to_row(maxs[x]);
        let y0 = to_row(mins[x]);
        let (a, b) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        for y in a..=b {
            lines[y][x] = '█';
        }
    }
    lines.into_iter().map(|row| row.into_iter().collect()).collect()
}
