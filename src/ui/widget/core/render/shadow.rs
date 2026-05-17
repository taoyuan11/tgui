use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use image::{DynamicImage, RgbaImage};
use resvg::tiny_skia;

use crate::foundation::error::TguiError;
use crate::media::TextureFrame;

use super::*;

pub(super) fn rounded_rect_shadow_cache_key(
    frame: Rect,
    corner_radius: f32,
    shadow: crate::theme::Shadow,
    opacity: f32,
    scale_factor: f32,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_f32(frame.width.get(), &mut hasher);
    hash_f32(frame.height.get(), &mut hasher);
    hash_f32(corner_radius, &mut hasher);
    hash_f32(shadow.offset_x.get(), &mut hasher);
    hash_f32(shadow.offset_y.get(), &mut hasher);
    hash_f32(shadow.blur.get(), &mut hasher);
    hash_f32(shadow.spread.get(), &mut hasher);
    shadow.color.hash(&mut hasher);
    hash_f32(opacity, &mut hasher);
    hash_f32(scale_factor, &mut hasher);
    hasher.finish()
}

pub(super) fn rasterize_rounded_rect_shadow(
    frame: Rect,
    corner_radius: f32,
    shadow: crate::theme::Shadow,
    opacity: f32,
    min_x: f32,
    min_y: f32,
    width: u32,
    height: u32,
    scale_factor: f32,
) -> Result<TextureFrame, TguiError> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| {
        TguiError::Media(format!(
            "failed to allocate widget shadow surface {}x{}",
            width, height
        ))
    })?;

    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    let path = build_rounded_rect_shadow_path(frame, corner_radius, min_x, min_y, scale_factor)
        .ok_or_else(|| {
            TguiError::Media("failed to build widget rounded shadow path".to_string())
        })?;
    pixmap.as_mut().fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::from_translate(
            shadow.offset_x.get() * scale_factor,
            shadow.offset_y.get() * scale_factor,
        ),
        None,
    );

    let blurred = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(width, height, pixmap.data().to_vec()).ok_or_else(|| {
            TguiError::Media("failed to create widget shadow image buffer".to_string())
        })?,
    )
    .fast_blur((shadow.blur.get() * scale_factor).max(0.0));

    let mut pixels = blurred.to_rgba8().into_raw();
    let shadow_color = shadow.color.with_alpha_factor(opacity);
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = pixel[3] as f32 / 255.0;
        pixel[0] = ((shadow_color.r as f32) * alpha).round().clamp(0.0, 255.0) as u8;
        pixel[1] = ((shadow_color.g as f32) * alpha).round().clamp(0.0, 255.0) as u8;
        pixel[2] = ((shadow_color.b as f32) * alpha).round().clamp(0.0, 255.0) as u8;
        pixel[3] = ((shadow_color.a as f32) * alpha).round().clamp(0.0, 255.0) as u8;
    }

    Ok(TextureFrame::new(width, height, pixels))
}

pub(super) fn shadow_padding(blur: f32) -> f32 {
    (blur * 2.0).ceil().max(1.0)
}

fn hash_f32(value: f32, hasher: &mut impl Hasher) {
    value.to_bits().hash(hasher);
}

pub(super) fn build_rounded_rect_shadow_path(
    frame: Rect,
    corner_radius: f32,
    min_x: f32,
    min_y: f32,
    scale_factor: f32,
) -> Option<tiny_skia::Path> {
    let x = (frame.x.get() - min_x) * scale_factor;
    let y = (frame.y.get() - min_y) * scale_factor;
    let width = frame.width.get() * scale_factor;
    let height = frame.height.get() * scale_factor;
    let radius = (corner_radius * scale_factor)
        .max(0.0)
        .min(width * 0.5)
        .min(height * 0.5);
    let mut builder = tiny_skia::PathBuilder::new();

    if radius <= 0.0 {
        let rect = tiny_skia::Rect::from_xywh(x, y, width, height)?;
        builder.push_rect(rect);
        return builder.finish();
    }

    let right = x + width;
    let bottom = y + height;
    builder.move_to(x + radius, y);
    append_tiny_skia_arc_segments(
        &mut builder,
        right - radius,
        y + radius,
        radius,
        -std::f32::consts::FRAC_PI_2,
        std::f32::consts::FRAC_PI_2,
        true,
    );
    append_tiny_skia_arc_segments(
        &mut builder,
        right - radius,
        bottom - radius,
        radius,
        0.0,
        std::f32::consts::FRAC_PI_2,
        true,
    );
    append_tiny_skia_arc_segments(
        &mut builder,
        x + radius,
        bottom - radius,
        radius,
        std::f32::consts::FRAC_PI_2,
        std::f32::consts::FRAC_PI_2,
        true,
    );
    append_tiny_skia_arc_segments(
        &mut builder,
        x + radius,
        y + radius,
        radius,
        std::f32::consts::PI,
        std::f32::consts::FRAC_PI_2,
        true,
    );
    builder.close();
    builder.finish()
}

fn append_tiny_skia_arc_segments(
    builder: &mut tiny_skia::PathBuilder,
    center_x: f32,
    center_y: f32,
    radius: f32,
    start_angle: f32,
    sweep_angle: f32,
    connect_with_line: bool,
) {
    if radius <= 0.0 || sweep_angle.abs() <= f32::EPSILON {
        return;
    }

    let steps = ((sweep_angle.abs() / std::f32::consts::FRAC_PI_8).ceil() as usize).max(1);
    for index in 0..=steps {
        let t = index as f32 / steps as f32;
        let angle = start_angle + sweep_angle * t;
        let px = center_x + angle.cos() * radius;
        let py = center_y + angle.sin() * radius;
        if index == 0 {
            if connect_with_line {
                builder.line_to(px, py);
            } else {
                builder.move_to(px, py);
            }
        } else {
            builder.line_to(px, py);
        }
    }
}
