use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use image::{DynamicImage, RgbaImage};
use resvg::tiny_skia;

use crate::foundation::error::TguiError;
use crate::media::TextureFrame;

use super::*;

#[cfg(feature = "bench-support")]
thread_local! {
    static FORCE_LEGACY_WIDGET_SHADOW_OPACITY: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// Benchmark-only A/B control for the former widget-shadow path, which baked the
/// current visual opacity into every cached RGBA texture.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_widget_shadow_opacity<R>(legacy: bool, f: impl FnOnce() -> R) -> R {
    FORCE_LEGACY_WIDGET_SHADOW_OPACITY.with(|flag| {
        let previous = flag.replace(legacy);
        struct Reset<'a> {
            flag: &'a std::cell::Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.flag.set(self.previous);
            }
        }
        let _reset = Reset { flag, previous };
        f()
    })
}

pub(crate) fn widget_shadow_opacity_legacy_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_WIDGET_SHADOW_OPACITY.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    false
}

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
    if widget_shadow_opacity_legacy_enabled() {
        hash_f32(opacity, &mut hasher);
    }
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
    let shadow_color = shadow
        .color
        .with_alpha_factor(if widget_shadow_opacity_legacy_enabled() {
            opacity
        } else {
            1.0
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::unit::dp;

    fn test_shadow() -> crate::theme::Shadow {
        crate::theme::Shadow {
            offset_x: dp(0.0),
            offset_y: dp(3.0),
            blur: dp(6.0),
            spread: dp(-1.0),
            color: Color::hexa(0x1122339B),
        }
    }

    #[test]
    fn canonical_widget_shadow_pixels_and_cache_key_ignore_visual_opacity() {
        let frame = Rect::new(0.0, 0.0, 48.0, 32.0);
        let shadow = test_shadow();
        let low_key = rounded_rect_shadow_cache_key(frame, 8.0, shadow.clone(), 0.25, 1.0);
        let high_key = rounded_rect_shadow_cache_key(frame, 8.0, shadow.clone(), 0.85, 1.0);
        assert_eq!(low_key, high_key);

        let low = rasterize_rounded_rect_shadow(
            frame,
            8.0,
            shadow.clone(),
            0.25,
            -16.0,
            -16.0,
            80,
            72,
            1.0,
        )
        .expect("canonical low-opacity shadow");
        let high =
            rasterize_rounded_rect_shadow(frame, 8.0, shadow, 0.85, -16.0, -16.0, 80, 72, 1.0)
                .expect("canonical high-opacity shadow");
        assert_eq!(low.pixels(), high.pixels());
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn canonical_primitive_opacity_matches_legacy_baked_alpha_within_two_lsb() {
        let frame = Rect::new(0.0, 0.0, 48.0, 32.0);
        let shadow = test_shadow();
        let opacity = 0.37;
        let canonical = rasterize_rounded_rect_shadow(
            frame,
            8.0,
            shadow.clone(),
            opacity,
            -16.0,
            -16.0,
            80,
            72,
            1.0,
        )
        .expect("canonical shadow");
        let legacy = with_legacy_widget_shadow_opacity(true, || {
            rasterize_rounded_rect_shadow(frame, 8.0, shadow, opacity, -16.0, -16.0, 80, 72, 1.0)
        })
        .expect("legacy shadow");
        for (canonical, legacy) in canonical
            .pixels()
            .chunks_exact(4)
            .zip(legacy.pixels().chunks_exact(4))
        {
            assert_eq!(&canonical[..3], &legacy[..3]);
            let canonical_draw_alpha = canonical[3] as f32 * opacity;
            assert!(
                (canonical_draw_alpha - legacy[3] as f32).abs() <= 2.0,
                "canonical={} legacy={}",
                canonical_draw_alpha,
                legacy[3]
            );
        }
    }
}
