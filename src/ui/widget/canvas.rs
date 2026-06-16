use std::cell::RefCell;
use std::sync::Arc;

mod geometry;
mod image_utils;
mod items;
mod path;
mod path_utils;
mod recorder;
mod scene;
mod scene_debug;
mod scene_query;
mod scene_render;
mod scene_text;
mod tessellation;
mod types;
mod widget;

use lyon::math::vector;
use resvg::tiny_skia;

use crate::foundation::binding::Signal;
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::{
    resolve_media_rect, ContentFit, IntrinsicSize, MediaManager, MediaSource, RasterRequest,
};
use crate::text::font::{FontCatalog, FontManager, FontWeight, TextFontRequest};
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};
use crate::ui::unit::{Dp, Sp, UnitContext};
use unicode_segmentation::UnicodeSegmentation;

use super::background::{
    BackgroundBrush, BackgroundGradientStop, BackgroundLinearGradient, BackgroundRadialGradient,
};
use super::common::{
    CanvasCompositePrimitive, CanvasItemInteractionHandlers, CanvasTextSpanPrimitive, ClipMask,
    CursorStyle, InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, MeshPrimitive,
    MeshVertex, Point, Rect, RenderCommand, TextPrimitive, TexturePrimitive, VisualStyle, WidgetId,
    WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::style::CanvasStyle;
pub(crate) use geometry::*;
pub(crate) use image_utils::*;
pub use items::*;
pub use path::*;
pub use recorder::*;
pub use scene::*;
pub use scene_debug::*;
pub(crate) use scene_debug::{
    canvas_text_horizontal_align_name, canvas_text_overflow_name, canvas_text_vertical_align_name,
    canvas_text_wrap_name,
};
pub use scene_query::*;
pub(crate) use scene_render::*;
pub(crate) use scene_text::*;
use tessellation::*;
pub use types::*;
pub use widget::*;

const MAX_CANVAS_GRADIENT_STOPS: usize = 8;
const CANVAS_FLATTEN_TOLERANCE: f32 = 0.1;
const SHADOW_BLUR_PADDING_MULTIPLIER: f32 = 3.0;

fn tessellate_items(
    items: &[CanvasItem],
    origin: Point,
    opacity: f32,
    clip: CanvasClipContext,
    media: &MediaManager,
    units: UnitContext,
) -> CanvasRenderOutput {
    let mut output = CanvasRenderOutput::default();
    for item in items {
        let rendered = item.tessellate(origin, opacity, clip, media, units);
        output.commands.extend(rendered.commands);
        output.meshes.extend(rendered.meshes);
        output.textures.extend(rendered.textures);
        output.texts.extend(rendered.texts);
    }
    output
}

fn output_to_commands(output: CanvasRenderOutput) -> Vec<RenderCommand> {
    let mut commands = output.commands;
    commands.extend(output.meshes.into_iter().map(RenderCommand::Mesh));
    commands.extend(output.textures.into_iter().map(RenderCommand::Texture));
    commands.extend(
        output
            .texts
            .into_iter()
            .map(|t| RenderCommand::Text(Box::new(t))),
    );
    commands
}

fn tessellate_composite_item(
    item: &CanvasItem,
    origin: Point,
    opacity: f32,
    clip: CanvasClipContext,
    media: &MediaManager,
    units: UnitContext,
) -> CanvasRenderOutput {
    let mut output = CanvasRenderOutput::default();
    let Some(bounds) = item.layout_bounds() else {
        return output;
    };
    let bounds_rect = offset_rect(bounds_rect(bounds), origin);
    let style = item.style();
    let resolved_effects = resolve_canvas_effects(&style.effects);

    let (content_output, mask_commands, blend_mode) = match item {
        CanvasItem::Path(path) => (
            tessellate_path(path, origin, opacity * style.opacity, clip, media, units),
            None,
            style.blend_mode,
        ),
        CanvasItem::Text(text) => (
            tessellate_text(text, origin, opacity * style.opacity, clip),
            None,
            style.blend_mode,
        ),
        CanvasItem::Image(image) => (
            tessellate_image(image, origin, opacity * style.opacity, clip, media, units),
            None,
            style.blend_mode,
        ),
        CanvasItem::Group(group_item) => {
            let nested_output =
                tessellate_items(&group_item.items, origin, opacity, clip, media, units);
            let CanvasGroupShape::Path { path, fill_rule } = &group_item.shape;
            let mask = tessellate_path(
                &CanvasPath::new(group_item.style.id, path.clone())
                    .fill_rule(*fill_rule)
                    .fill(Color::WHITE),
                origin,
                1.0,
                CanvasClipContext::default(),
                media,
                units,
            );
            let mask_commands = match group_item.mode {
                CanvasGroupMode::Clip | CanvasGroupMode::Mask => {
                    Some(output_to_commands(mask).into())
                }
            };
            (nested_output, mask_commands, style.blend_mode)
        }
    };

    let mut content_output = content_output;
    if style.transform != CanvasTransform2D::IDENTITY {
        apply_transform_to_output(&mut content_output, style.transform, origin);
    }

    let content_commands: Arc<[RenderCommand]> = output_to_commands(content_output).into();
    output
        .commands
        .push(RenderCommand::CanvasComposite(Box::new(
            CanvasCompositePrimitive {
                bounds: bounds_rect,
                opacity: 1.0,
                blend_mode,
                blur_radius: resolved_effects.blur_radius,
                color_filter: resolved_effects.color_filter,
                inner_shadow_color: resolved_effects.inner_shadow.map(|shadow| shadow.color),
                inner_shadow_offset: resolved_effects
                    .inner_shadow
                    .map(|shadow| shadow.offset)
                    .unwrap_or(Point::ZERO),
                inner_shadow_blur_radius: resolved_effects
                    .inner_shadow
                    .map(|shadow| shadow.blur.get().max(0.0))
                    .unwrap_or(0.0),
                clip_rect: clip.clip_rect,
                clip_mask: clip.clip_mask,
                content_commands,
                mask_commands,
            },
        )));
    output
}

fn tessellate_text(
    text: &CanvasText,
    origin: Point,
    opacity: f32,
    clip: CanvasClipContext,
) -> CanvasRenderOutput {
    let frame = offset_rect(text.frame, origin);
    let line_height = text
        .text_style
        .line_height
        .unwrap_or(Sp::new(text.text_style.font_size.get() * 1.2));
    let content = text.content.plain_text();
    let rich_spans = match &text.content {
        CanvasTextContent::Plain(_) => None,
        CanvasTextContent::Rich(spans) => Some(Arc::from(
            spans
                .iter()
                .cloned()
                .map(|span| CanvasTextSpanPrimitive {
                    content: Arc::from(span.content),
                    font_family: span.style.font_family.map(Arc::from),
                    color: span
                        .style
                        .color
                        .with_alpha_factor(opacity * text.style.opacity),
                    font_size: span.style.font_size.get(),
                    font_weight: span.style.font_weight,
                    line_height: span.style.line_height.map(|height| height.get()),
                    letter_spacing: span.style.letter_spacing.get(),
                })
                .collect::<Vec<_>>(),
        )),
    };
    CanvasRenderOutput {
        texts: vec![TextPrimitive {
            content: Arc::from(content),
            rich_spans,
            frame,
            quad: None,
            color: text
                .text_style
                .color
                .with_alpha_factor(opacity * text.style.opacity),
            force_color: true,
            font_family: text.text_style.font_family.clone().map(Arc::from),
            font_size: text.text_style.font_size.get(),
            font_weight: text.text_style.font_weight,
            line_height: line_height.get(),
            letter_spacing: text.text_style.letter_spacing.get(),
            wrap: text.paragraph_style.wrap,
            overflow: text.paragraph_style.overflow,
            horizontal_align: text.paragraph_style.horizontal_align,
            vertical_align: text.paragraph_style.vertical_align,
            clip_rect: clip.clip_rect,
            clip_mask: clip.clip_mask,
        }],
        ..Default::default()
    }
}

fn tessellate_image(
    image: &CanvasImage,
    origin: Point,
    opacity: f32,
    clip: CanvasClipContext,
    media: &MediaManager,
    units: UnitContext,
) -> CanvasRenderOutput {
    let frame = offset_rect(image.frame, origin);
    let metadata = media.image_snapshot(&image.source, None);
    let intrinsic_size = metadata.intrinsic_size;
    let source_rect = normalized_source_rect(image.source_rect, intrinsic_size);
    let source_size = source_rect
        .map(|rect| intrinsic_size_from_rect(rect))
        .unwrap_or(intrinsic_size);
    let target_frame = resolve_media_rect(frame, source_size, image.fit);
    let snapshot = if let Some(raster_request) =
        raster_request_for_image(metadata.intrinsic_size, source_rect, target_frame, units)
    {
        media.image_snapshot(&image.source, Some(raster_request))
    } else {
        metadata
    };
    let Some(texture) = snapshot.texture else {
        return CanvasRenderOutput::default();
    };

    CanvasRenderOutput {
        textures: vec![TexturePrimitive {
            texture,
            media_key: None,
            frame: target_frame,
            quad: None,
            uv_rect: source_rect.and_then(|rect| source_rect_to_uv_rect(rect, intrinsic_size)),
            corner_radius: image.corner_radius.get(),
            opacity: opacity * image.style.opacity,
            clip_rect: clip.clip_rect,
            clip_mask: clip.clip_mask,
        }],
        ..Default::default()
    }
}

fn apply_transform_to_output(
    output: &mut CanvasRenderOutput,
    transform: CanvasTransform2D,
    origin: Point,
) {
    if transform == CanvasTransform2D::IDENTITY {
        return;
    }

    for mesh in &mut output.meshes {
        let mut vertices = mesh.vertices.to_vec();
        for vertex in &mut vertices {
            let point_value = transform.apply(Point::new(
                vertex.position[0] - origin.x.get(),
                vertex.position[1] - origin.y.get(),
            ));
            vertex.position = [
                origin.x.get() + point_value.x.get(),
                origin.y.get() + point_value.y.get(),
            ];
        }
        mesh.vertices = Arc::from(vertices);

        let mut triangles = mesh.triangles.to_vec();
        for triangle in &mut triangles {
            for point_value in triangle.iter_mut() {
                let transformed = transform.apply(Point::new(
                    point_value.x - origin.x,
                    point_value.y - origin.y,
                ));
                *point_value = Point::new(origin.x + transformed.x, origin.y + transformed.y);
            }
        }
        mesh.triangles = Arc::from(triangles);
    }

    for texture in &mut output.textures {
        let quad = transform_rect_quad(texture.frame, transform, origin);
        texture.quad = Some(quad);
        if let Some(rect) = quad_bounds_rect(quad) {
            texture.frame = rect;
        }
    }

    for text in &mut output.texts {
        let quad = transform_rect_quad(text.frame, transform, origin);
        text.quad = Some(quad);
        if let Some(rect) = quad_bounds_rect(quad) {
            text.frame = rect;
        }
    }
}

#[cfg(test)]
mod tests;
