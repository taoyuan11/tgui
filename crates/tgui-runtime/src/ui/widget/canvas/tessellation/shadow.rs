use std::hash::{Hash, Hasher};

use image::{DynamicImage, RgbaImage};
use lyon::path::{Path, PathEvent};
use resvg::tiny_skia;

use crate::foundation::error::TguiError;
use crate::media::{MediaManager, TextureFrame};
use crate::ui::unit::{Dp, Sp, UnitContext};
use crate::ui::widget::common;

use super::super::*;
use super::mesh::{dashed_path, normalize_dash_pattern};

#[cfg(feature = "bench-support")]
thread_local! {
    static FORCE_LEGACY_CANVAS_SHADOW_OPACITY: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// Benchmark-only A/B control for the former canvas-shadow path, which baked the
/// current visual opacity into every cached RGBA texture.
#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_canvas_shadow_opacity<R>(legacy: bool, f: impl FnOnce() -> R) -> R {
    FORCE_LEGACY_CANVAS_SHADOW_OPACITY.with(|flag| {
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

fn canvas_shadow_opacity_legacy_enabled() -> bool {
    #[cfg(feature = "bench-support")]
    {
        return FORCE_LEGACY_CANVAS_SHADOW_OPACITY.with(std::cell::Cell::get);
    }
    #[cfg(not(feature = "bench-support"))]
    false
}

pub(super) fn shadow_texture_for_path(
    path: &CanvasPath,
    lyon_path: &Path,
    fill: Option<&CanvasBrush>,
    stroke: Option<&CanvasStroke>,
    shadow: CanvasShadow,
    opacity: f32,
    origin: Point,
    clip: CanvasClipContext,
    media: &MediaManager,
    units: UnitContext,
) -> Option<TexturePrimitive> {
    let base_bounds = super::path_base_bounds(path)?;
    let padding = shadow_padding(shadow.blur);
    let min_x = base_bounds.min_x + shadow.offset.x.get().min(0.0) - padding;
    let min_y = base_bounds.min_y + shadow.offset.y.get().min(0.0) - padding;
    let max_x = base_bounds.max_x + shadow.offset.x.get().max(0.0) + padding;
    let max_y = base_bounds.max_y + shadow.offset.y.get().max(0.0) + padding;
    let frame = common::Rect::new(
        origin.x + min_x,
        origin.y + min_y,
        (max_x - min_x).max(1.0),
        (max_y - min_y).max(1.0),
    );
    let width = units.logical_to_physical(frame.width.get()).ceil().max(1.0) as u32;
    let height = units
        .logical_to_physical(frame.height.get())
        .ceil()
        .max(1.0) as u32;

    let legacy_opacity = canvas_shadow_opacity_legacy_enabled();
    let opacity = opacity.clamp(0.0, 1.0);
    let cache_key = canvas_shadow_cache_key(
        path,
        shadow,
        if legacy_opacity { opacity } else { 1.0 },
        units.scale_factor(),
    );
    let texture = media
        .canvas_shadow_texture(cache_key, width, height, || {
            rasterize_canvas_shadow(
                lyon_path,
                fill.is_some(),
                stroke,
                path.fill_rule,
                shadow,
                if legacy_opacity { opacity } else { 1.0 },
                min_x,
                min_y,
                units.scale_factor(),
            )
        })
        .ok()??;

    Some(TexturePrimitive {
        texture,
        media_key: None,
        media_layout: None,
        mask_tint: None,
        frame,
        quad: None,
        uv_rect: None,
        corner_radius: 0.0,
        opacity: if legacy_opacity { 1.0 } else { opacity },
        clip_rect: clip.clip_rect,
        clip_mask: clip.clip_mask,
    })
}

fn rasterize_canvas_shadow(
    path: &Path,
    has_fill: bool,
    stroke: Option<&CanvasStroke>,
    fill_rule: CanvasFillRule,
    shadow: CanvasShadow,
    opacity: f32,
    min_x: f32,
    min_y: f32,
    scale_factor: f32,
) -> Result<TextureFrame, TguiError> {
    let dashed = stroke.and_then(|stroke| dashed_path(path, stroke));
    let source_path = dashed.as_ref().unwrap_or(path);
    let tiny_path = to_tiny_skia_path(source_path, min_x, min_y, scale_factor)?;

    let (width, height) = super::transformed_path_size(source_path, min_x, min_y, scale_factor);
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| {
        TguiError::Media(format!(
            "failed to allocate canvas shadow surface {}x{}",
            width, height
        ))
    })?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);

    if has_fill {
        pixmap.as_mut().fill_path(
            &tiny_path,
            &paint,
            fill_rule.to_tiny_skia(),
            tiny_skia::Transform::identity(),
            None,
        );
    }

    if let Some(stroke) = stroke {
        let mut stroke_style = tiny_skia::Stroke {
            width: stroke.width.get().max(0.0) * scale_factor,
            line_cap: match stroke.line_cap {
                CanvasStrokeCap::Butt => tiny_skia::LineCap::Butt,
                CanvasStrokeCap::Square => tiny_skia::LineCap::Square,
                CanvasStrokeCap::Round => tiny_skia::LineCap::Round,
            },
            line_join: match stroke.line_join {
                CanvasStrokeJoin::Miter => tiny_skia::LineJoin::Miter,
                CanvasStrokeJoin::Bevel => tiny_skia::LineJoin::Bevel,
                CanvasStrokeJoin::Round => tiny_skia::LineJoin::Round,
            },
            miter_limit: stroke.miter_limit.max(0.0),
            ..Default::default()
        };
        if let Some(pattern) = stroke
            .dash_pattern
            .as_ref()
            .and_then(|pattern| normalize_dash_pattern(pattern))
        {
            stroke_style.dash = tiny_skia::StrokeDash::new(
                pattern
                    .into_iter()
                    .map(|value| value * scale_factor)
                    .collect(),
                stroke.dash_offset.get() * scale_factor,
            );
        }
        pixmap.as_mut().stroke_path(
            &tiny_path,
            &paint,
            &stroke_style,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    let blurred = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(width, height, pixmap.data().to_vec()).ok_or_else(|| {
            TguiError::Media("failed to create canvas shadow image buffer".to_string())
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

fn to_tiny_skia_path(
    path: &Path,
    min_x: f32,
    min_y: f32,
    scale_factor: f32,
) -> Result<tiny_skia::Path, TguiError> {
    let mut builder = tiny_skia::PathBuilder::new();
    for event in path.iter() {
        match event {
            PathEvent::Begin { at } => {
                builder.move_to((at.x - min_x) * scale_factor, (at.y - min_y) * scale_factor)
            }
            PathEvent::Line { to, .. } => {
                builder.line_to((to.x - min_x) * scale_factor, (to.y - min_y) * scale_factor)
            }
            PathEvent::Quadratic { ctrl, to, .. } => builder.quad_to(
                (ctrl.x - min_x) * scale_factor,
                (ctrl.y - min_y) * scale_factor,
                (to.x - min_x) * scale_factor,
                (to.y - min_y) * scale_factor,
            ),
            PathEvent::Cubic {
                ctrl1, ctrl2, to, ..
            } => builder.cubic_to(
                (ctrl1.x - min_x) * scale_factor,
                (ctrl1.y - min_y) * scale_factor,
                (ctrl2.x - min_x) * scale_factor,
                (ctrl2.y - min_y) * scale_factor,
                (to.x - min_x) * scale_factor,
                (to.y - min_y) * scale_factor,
            ),
            PathEvent::End { close, .. } => {
                if close {
                    builder.close();
                }
            }
        }
    }

    builder.finish().ok_or_else(|| {
        TguiError::Media("failed to finish canvas shadow path rasterization".to_string())
    })
}

fn canvas_shadow_cache_key(
    path: &CanvasPath,
    shadow: CanvasShadow,
    opacity: f32,
    scale_factor: f32,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.fill_rule.hash(&mut hasher);
    for command in path.path.commands_internal() {
        match *command {
            PathCommand::MoveTo(point_value) => {
                0u8.hash(&mut hasher);
                hash_point(point_value, &mut hasher);
            }
            PathCommand::LineTo(point_value) => {
                1u8.hash(&mut hasher);
                hash_point(point_value, &mut hasher);
            }
            PathCommand::QuadTo { ctrl, to } => {
                2u8.hash(&mut hasher);
                hash_point(ctrl, &mut hasher);
                hash_point(to, &mut hasher);
            }
            PathCommand::CubicTo { ctrl1, ctrl2, to } => {
                3u8.hash(&mut hasher);
                hash_point(ctrl1, &mut hasher);
                hash_point(ctrl2, &mut hasher);
                hash_point(to, &mut hasher);
            }
            PathCommand::Close => {
                4u8.hash(&mut hasher);
            }
        }
    }
    path.fill.is_some().hash(&mut hasher);
    if let Some(stroke) = path.stroke.as_ref() {
        hash_f32(stroke.width.get(), &mut hasher);
        if let Some(pattern) = stroke.dash_pattern.as_ref() {
            pattern.len().hash(&mut hasher);
            for value in pattern {
                hash_f32(value.get(), &mut hasher);
            }
        } else {
            0usize.hash(&mut hasher);
        }
        hash_f32(stroke.dash_offset.get(), &mut hasher);
    } else {
        0u8.hash(&mut hasher);
    }
    shadow.color.hash(&mut hasher);
    hash_point(shadow.offset, &mut hasher);
    hash_f32(shadow.blur.get(), &mut hasher);
    hash_f32(opacity, &mut hasher);
    hash_f32(scale_factor, &mut hasher);
    hasher.finish()
}

fn hash_point(point_value: Point, hasher: &mut impl Hasher) {
    hash_f32(point_value.x.get(), hasher);
    hash_f32(point_value.y.get(), hasher);
}

fn hash_rect(rect: Rect, hasher: &mut impl Hasher) {
    hash_f32(rect.x.get(), hasher);
    hash_f32(rect.y.get(), hasher);
    hash_f32(rect.width.get(), hasher);
    hash_f32(rect.height.get(), hasher);
}

fn hash_f32(value: f32, hasher: &mut impl Hasher) {
    value.to_bits().hash(hasher);
}

pub(in super::super) fn canvas_text_hit_cache_key(item: &CanvasItem, units: UnitContext) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    item.id().hash(&mut hasher);
    hash_f32(units.scale_factor(), &mut hasher);
    hash_f32(units.resolve_sp(Sp::new(1.0)), &mut hasher);

    if let CanvasItem::Text(text) = item {
        text.plain_text().hash(&mut hasher);
        text.name().hash(&mut hasher);
        hash_rect(text.frame, &mut hasher);
        for value in text.style.transform.matrix {
            hash_f32(value, &mut hasher);
        }
        text.text_style.font_family.hash(&mut hasher);
        text.text_style.color.hash(&mut hasher);
        hash_f32(text.text_style.font_size.get(), &mut hasher);
        text.text_style.font_weight.hash(&mut hasher);
        if let Some(line_height) = text.text_style.line_height {
            1u8.hash(&mut hasher);
            hash_f32(line_height.get(), &mut hasher);
        } else {
            0u8.hash(&mut hasher);
        }
        hash_f32(text.text_style.letter_spacing.get(), &mut hasher);
        canvas_text_wrap_name(text.paragraph_style.wrap).hash(&mut hasher);
        canvas_text_horizontal_align_name(text.paragraph_style.horizontal_align).hash(&mut hasher);
        canvas_text_vertical_align_name(text.paragraph_style.vertical_align).hash(&mut hasher);
        canvas_text_overflow_name(text.paragraph_style.overflow).hash(&mut hasher);
    }

    hasher.finish()
}

#[cfg(test)]
mod tests {
    use lyon::math::point;

    use super::*;
    use crate::foundation::color::Color;
    use crate::ui::unit::dp;

    #[test]
    fn canonical_canvas_shadow_pixels_match_legacy_opacity_scaling() {
        let mut builder = Path::builder();
        builder.begin(point(4.0, 4.0));
        builder.line_to(point(44.0, 4.0));
        builder.line_to(point(40.0, 28.0));
        builder.line_to(point(8.0, 32.0));
        builder.end(true);
        let path = builder.build();
        let shadow = CanvasShadow::new(
            Color::hexa(0x112233B8),
            Point::new(dp(2.0), dp(4.0)),
            dp(6.0),
        );
        let canonical = rasterize_canvas_shadow(
            &path,
            true,
            None,
            CanvasFillRule::NonZero,
            shadow,
            1.0,
            0.0,
            0.0,
            1.0,
        )
        .expect("canonical canvas shadow should rasterize");
        let legacy = rasterize_canvas_shadow(
            &path,
            true,
            None,
            CanvasFillRule::NonZero,
            shadow,
            0.35,
            0.0,
            0.0,
            1.0,
        )
        .expect("legacy canvas shadow should rasterize");
        assert_eq!(canonical.size(), legacy.size());
        let mut max_alpha_error = 0_u8;
        for (canonical, legacy) in canonical
            .pixels()
            .chunks_exact(4)
            .zip(legacy.pixels().chunks_exact(4))
        {
            assert_eq!(&canonical[..3], &legacy[..3]);
            let expected_alpha = ((canonical[3] as f32) * 0.35).round() as u8;
            max_alpha_error = max_alpha_error.max(legacy[3].abs_diff(expected_alpha));
        }
        assert!(max_alpha_error <= 2, "alpha error was {max_alpha_error}");
    }
}

pub(in super::super) fn shadow_padding(blur: Dp) -> f32 {
    blur.get().max(0.0) * SHADOW_BLUR_PADDING_MULTIPLIER
}
