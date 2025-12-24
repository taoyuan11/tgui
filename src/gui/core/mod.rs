mod canvas;

pub use canvas::Canvas;

pub fn blend_colors(back: u32, front: u32, alpha: f32) -> u32 {
    if alpha >= 0.999 { return front; }
    if alpha <= 0.001 { return back; }

    let rb = ((back >> 16) & 0xFF) as f32;
    let gb = ((back >> 8) & 0xFF) as f32;
    let bb = (back & 0xFF) as f32;

    let rf = ((front >> 16) & 0xFF) as f32;
    let gf = ((front >> 8) & 0xFF) as f32;
    let bf = (front & 0xFF) as f32;

    // 使用平方和混合以获得更好的视觉效果 (Gamma Corrected)
    let r = (rb * rb * (1.0 - alpha) + rf * rf * alpha).sqrt();
    let g = (gb * gb * (1.0 - alpha) + gf * gf * alpha).sqrt();
    let b = (bb * bb * (1.0 - alpha) + bf * bf * alpha).sqrt();

    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}