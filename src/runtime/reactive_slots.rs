use super::state::ReactiveMediaTextureBindingUpdate;
use super::*;
use crate::ui::unit::Dp;
use crate::ui::widget::{
    BackdropBlurPrimitive, BackdropBlurPrimitiveSlot, BrushPrimitive, BrushPrimitiveSlot,
    HitInteraction, TextPrimitive, TexturePrimitive, TexturePrimitiveSlot,
};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct ReactiveSlotBinding {
    widget_id: WidgetId,
    local: LocalReactiveSlotBinding,
    root_offset: SceneCounts,
    root_hit_offset: usize,
    ancestor_offsets: Vec<(WidgetId, SceneCounts, usize)>,
    has_chunk_part: bool,
}

#[derive(Clone)]
struct ReactiveSlotPatch {
    writes: Vec<ReactiveSlotWrite>,
    hit_write: Option<ReactiveHitWrite>,
}

#[derive(Clone)]
struct LocalReactiveSlotBinding {
    kind: ReactiveSlotBindingKind,
}

#[derive(Clone)]
enum ReactiveSlotBindingKind {
    ShapeFillColor {
        slot: ShapePrimitiveSlot,
    },
    ShapeStrokeColor {
        slot: ShapePrimitiveSlot,
    },
    BackdropBlur {
        slot: BackdropBlurPrimitiveSlot,
    },
    Brush {
        slot: BrushPrimitiveSlot,
    },
    BorderRadius {
        background: Option<ShapePrimitiveSlot>,
        border: Option<ShapePrimitiveSlot>,
    },
    BorderWidth {
        background: Option<ShapePrimitiveSlot>,
        border: Option<ShapePrimitiveSlot>,
    },
    Opacity {
        background: Option<ShapePrimitiveSlot>,
        border: Option<ShapePrimitiveSlot>,
        text: Option<TextPrimitiveSlot>,
    },
    Offset {
        backdrop_blur: Option<BackdropBlurPrimitiveSlot>,
        background: Option<ShapePrimitiveSlot>,
        border: Option<ShapePrimitiveSlot>,
        texture: Option<TexturePrimitiveSlot>,
        brush: Option<BrushPrimitiveSlot>,
    },
    Scale {
        backdrop_blur: Option<BackdropBlurPrimitiveSlot>,
        background: Option<ShapePrimitiveSlot>,
        border: Option<ShapePrimitiveSlot>,
        texture: Option<TexturePrimitiveSlot>,
        brush: Option<BrushPrimitiveSlot>,
    },
    TextColor {
        slot: TextPrimitiveSlot,
    },
    TextContent {
        slot: TextPrimitiveSlot,
    },
    TextInputContent {
        slot: TextPrimitiveSlot,
    },
    TextureOpacity {
        slot: TexturePrimitiveSlot,
    },
    Texture {
        slot: TexturePrimitiveSlot,
    },
    ProgressFill {
        fill: ShapePrimitiveSlot,
        label: Option<TextPrimitiveSlot>,
    },
    SliderValue {
        active: ShapePrimitiveSlot,
        thumb: ShapePrimitiveSlot,
        thumb_border: Option<ShapePrimitiveSlot>,
        label: Option<TextPrimitiveSlot>,
        hit_index: usize,
        widget_id: WidgetId,
    },
}

#[derive(Clone)]
enum ReactiveSlotWrite {
    ShapeColor {
        slot: ShapePrimitiveSlot,
        color: Color,
    },
    ShapeRect {
        slot: ShapePrimitiveSlot,
        rect: Rect,
    },
    ShapeCornerRadius {
        slot: ShapePrimitiveSlot,
        corner_radius: f32,
    },
    ShapeStrokeWidth {
        slot: ShapePrimitiveSlot,
        stroke_width: f32,
    },
    BackdropBlur {
        slot: BackdropBlurPrimitiveSlot,
        primitive: BackdropBlurPrimitive,
    },
    Brush {
        slot: BrushPrimitiveSlot,
        primitive: BrushPrimitive,
    },
    TextColor {
        slot: TextPrimitiveSlot,
        color: Color,
    },
    TextContent {
        slot: TextPrimitiveSlot,
        content: Arc<str>,
        font_family: Option<Arc<str>>,
    },
    TextPrimitive {
        slot: TextPrimitiveSlot,
        primitive: TextPrimitive,
    },
    TextureOpacity {
        slot: TexturePrimitiveSlot,
        opacity: f32,
    },
    Texture {
        slot: TexturePrimitiveSlot,
        primitive: TexturePrimitive,
    },
}

#[derive(Clone, Copy)]
enum ReactiveHitWrite {
    Slider {
        hit_index: usize,
        widget_id: WidgetId,
        value: f32,
        track_rect: Rect,
        thumb_rect: Rect,
    },
}

impl ReactiveSlotWrite {
    fn write<VM>(&self, computed: &mut ComputedScene<VM>, offset: &SceneCounts) -> bool {
        match self {
            Self::ShapeColor { slot, color } => {
                computed.write_shape_color_slot(offset, *slot, *color)
            }
            Self::ShapeRect { slot, rect } => computed.write_shape_rect_slot(offset, *slot, *rect),
            Self::ShapeCornerRadius {
                slot,
                corner_radius,
            } => computed.write_shape_corner_radius_slot(offset, *slot, *corner_radius),
            Self::ShapeStrokeWidth { slot, stroke_width } => {
                computed.write_shape_stroke_width_slot(offset, *slot, *stroke_width)
            }
            Self::BackdropBlur { slot, primitive } => {
                computed.write_backdrop_blur_slot(offset, *slot, *primitive)
            }
            Self::Brush { slot, primitive } => {
                computed.write_brush_slot(offset, *slot, primitive.clone())
            }
            Self::TextColor { slot, color } => {
                computed.write_text_color_slot(offset, *slot, *color)
            }
            Self::TextContent {
                slot,
                content,
                font_family,
            } => computed.write_text_content_slot(
                offset,
                *slot,
                content.clone(),
                font_family.clone(),
            ),
            Self::TextPrimitive { slot, primitive } => {
                computed.write_text_slot(offset, *slot, primitive.clone())
            }
            Self::TextureOpacity { slot, opacity } => {
                computed.write_texture_opacity_slot(offset, *slot, *opacity)
            }
            Self::Texture { slot, primitive } => {
                computed.write_texture_slot(offset, *slot, primitive.clone())
            }
        }
    }
}

impl ReactiveHitWrite {
    fn can_write<VM>(self, computed: &ComputedScene<VM>, hit_offset: usize) -> bool {
        match self {
            Self::Slider {
                hit_index,
                widget_id,
                ..
            } => {
                let Some(index) = hit_offset.checked_add(hit_index) else {
                    return false;
                };
                matches!(
                    computed.hit_regions.get(index).map(|hit| &hit.interaction),
                    Some(HitInteraction::Slider { id, .. }) if *id == widget_id
                )
            }
        }
    }

    fn write<VM>(self, computed: &mut ComputedScene<VM>, hit_offset: usize) -> bool {
        match self {
            Self::Slider {
                hit_index,
                widget_id,
                value,
                track_rect,
                thumb_rect,
            } => {
                if !self.can_write(computed, hit_offset) {
                    return false;
                }
                let index = hit_offset + hit_index;
                let HitInteraction::Slider {
                    id,
                    value: target_value,
                    track_rect: target_track_rect,
                    thumb_rect: target_thumb_rect,
                    ..
                } = &mut computed.hit_regions[index].interaction
                else {
                    return false;
                };
                if *id != widget_id {
                    return false;
                }
                *target_value = value;
                *target_track_rect = track_rect;
                *target_thumb_rect = thumb_rect;
                true
            }
        }
    }
}

impl LocalReactiveSlotBinding {
    fn can_write<VM>(&self, computed: &ComputedScene<VM>, offset: &SceneCounts) -> bool {
        match &self.kind {
            ReactiveSlotBindingKind::ShapeFillColor { slot }
            | ReactiveSlotBindingKind::ShapeStrokeColor { slot } => {
                computed.can_write_shape_color_slot(offset, *slot)
            }
            ReactiveSlotBindingKind::BackdropBlur { slot } => {
                computed.can_write_backdrop_blur_slot(offset, *slot)
            }
            ReactiveSlotBindingKind::Brush { slot } => computed.can_write_brush_slot(offset, *slot),
            ReactiveSlotBindingKind::BorderRadius { background, border } => {
                can_write_shape_option(computed, offset, *background)
                    && can_write_shape_option(computed, offset, *border)
            }
            ReactiveSlotBindingKind::BorderWidth { background, border } => {
                can_write_shape_option(computed, offset, *background)
                    && can_write_shape_option(computed, offset, *border)
            }
            ReactiveSlotBindingKind::Opacity {
                background,
                border,
                text,
            } => {
                can_write_shape_option(computed, offset, *background)
                    && can_write_shape_option(computed, offset, *border)
                    && text
                        .map(|slot| computed.can_write_text_color_slot(offset, slot))
                        .unwrap_or(true)
            }
            ReactiveSlotBindingKind::Offset {
                backdrop_blur,
                background,
                border,
                texture,
                brush,
            }
            | ReactiveSlotBindingKind::Scale {
                backdrop_blur,
                background,
                border,
                texture,
                brush,
            } => {
                backdrop_blur
                    .map(|slot| computed.can_write_backdrop_blur_slot(offset, slot))
                    .unwrap_or(true)
                    && can_write_shape_option(computed, offset, *background)
                    && can_write_shape_option(computed, offset, *border)
                    && texture
                        .map(|slot| computed.can_write_texture_opacity_slot(offset, slot))
                        .unwrap_or(true)
                    && brush
                        .map(|slot| computed.can_write_brush_slot(offset, slot))
                        .unwrap_or(true)
            }
            ReactiveSlotBindingKind::TextColor { slot }
            | ReactiveSlotBindingKind::TextContent { slot }
            | ReactiveSlotBindingKind::TextInputContent { slot } => {
                computed.can_write_text_color_slot(offset, *slot)
            }
            ReactiveSlotBindingKind::TextureOpacity { slot }
            | ReactiveSlotBindingKind::Texture { slot } => {
                computed.can_write_texture_opacity_slot(offset, *slot)
            }
            ReactiveSlotBindingKind::ProgressFill { fill, label } => {
                computed.can_write_shape_color_slot(offset, *fill)
                    && label
                        .map(|slot| computed.can_write_text_color_slot(offset, slot))
                        .unwrap_or(true)
            }
            ReactiveSlotBindingKind::SliderValue {
                active,
                thumb,
                thumb_border,
                label,
                ..
            } => {
                computed.can_write_shape_color_slot(offset, *active)
                    && computed.can_write_shape_color_slot(offset, *thumb)
                    && can_write_shape_option(computed, offset, *thumb_border)
                    && label
                        .map(|slot| computed.can_write_text_color_slot(offset, slot))
                        .unwrap_or(true)
            }
        }
    }

    fn can_write_hit<VM>(&self, computed: &ComputedScene<VM>, hit_offset: usize) -> bool {
        match &self.kind {
            ReactiveSlotBindingKind::SliderValue {
                hit_index,
                widget_id,
                ..
            } => ReactiveHitWrite::Slider {
                hit_index: *hit_index,
                widget_id: *widget_id,
                value: 0.0,
                track_rect: Rect::new(Dp::ZERO, Dp::ZERO, Dp::ZERO, Dp::ZERO),
                thumb_rect: Rect::new(Dp::ZERO, Dp::ZERO, Dp::ZERO, Dp::ZERO),
            }
            .can_write(computed, hit_offset),
            _ => true,
        }
    }

    fn patch_for(&self, value: ReactiveScenePropertyValue) -> Option<ReactiveSlotPatch> {
        let mut writes = Vec::new();
        let mut hit_write = None;
        match (&self.kind, value) {
            (
                ReactiveSlotBindingKind::ShapeFillColor { slot },
                ReactiveScenePropertyValue::ShapeFillColor { color, .. },
            )
            | (
                ReactiveSlotBindingKind::ShapeStrokeColor { slot },
                ReactiveScenePropertyValue::ShapeStrokeColor { color, .. },
            ) => {
                writes.push(ReactiveSlotWrite::ShapeColor { slot: *slot, color });
            }
            (
                ReactiveSlotBindingKind::BackdropBlur { slot },
                ReactiveScenePropertyValue::BackdropBlur(primitive),
            ) => {
                writes.push(ReactiveSlotWrite::BackdropBlur {
                    slot: *slot,
                    primitive,
                });
            }
            (
                ReactiveSlotBindingKind::Brush { slot },
                ReactiveScenePropertyValue::Brush(primitive),
            ) => {
                writes.push(ReactiveSlotWrite::Brush {
                    slot: *slot,
                    primitive,
                });
            }
            (
                ReactiveSlotBindingKind::BorderRadius { background, border },
                ReactiveScenePropertyValue::BorderRadius {
                    background: value_background,
                    border: value_border,
                },
            ) => {
                if background.is_some() != value_background.is_some()
                    || border.is_some() != value_border.is_some()
                {
                    return None;
                }
                if let (Some(slot), Some((_, _, corner_radius))) = (*background, value_background) {
                    writes.push(ReactiveSlotWrite::ShapeCornerRadius {
                        slot,
                        corner_radius,
                    });
                }
                if let (Some(slot), Some((_, _, _, corner_radius))) = (*border, value_border) {
                    writes.push(ReactiveSlotWrite::ShapeCornerRadius {
                        slot,
                        corner_radius,
                    });
                }
            }
            (
                ReactiveSlotBindingKind::BorderWidth { background, border },
                ReactiveScenePropertyValue::BorderWidth {
                    background: value_background,
                    border: value_border,
                    ..
                },
            ) => {
                if background.is_some() != value_background.is_some()
                    || border.is_some() != value_border.is_some()
                {
                    return None;
                }
                if let (Some(slot), Some((rect, _, corner_radius))) =
                    (*background, value_background)
                {
                    writes.push(ReactiveSlotWrite::ShapeRect { slot, rect });
                    writes.push(ReactiveSlotWrite::ShapeCornerRadius {
                        slot,
                        corner_radius,
                    });
                }
                if let (Some(slot), Some((_, _, stroke_width))) = (*border, value_border) {
                    writes.push(ReactiveSlotWrite::ShapeStrokeWidth { slot, stroke_width });
                }
            }
            (
                ReactiveSlotBindingKind::Opacity {
                    background,
                    border,
                    text,
                },
                ReactiveScenePropertyValue::Opacity {
                    background: value_background,
                    border: value_border,
                    text: value_text,
                },
            ) => {
                if background.is_some() != value_background.is_some()
                    || border.is_some() != value_border.is_some()
                    || text.is_some() != value_text.is_some()
                {
                    return None;
                }
                if let (Some(slot), Some((_, color))) = (*background, value_background) {
                    writes.push(ReactiveSlotWrite::ShapeColor { slot, color });
                }
                if let (Some(slot), Some((_, _, color))) = (*border, value_border) {
                    writes.push(ReactiveSlotWrite::ShapeColor { slot, color });
                }
                if let (Some(slot), Some(color)) = (*text, value_text) {
                    writes.push(ReactiveSlotWrite::TextColor { slot, color });
                }
            }
            (
                ReactiveSlotBindingKind::Offset {
                    backdrop_blur,
                    background,
                    border,
                    texture,
                    brush,
                },
                ReactiveScenePropertyValue::Offset {
                    background: value_background,
                    border: value_border,
                    backdrop_blur: value_backdrop_blur,
                    brush: value_brush,
                    texture: value_texture,
                },
            ) => {
                if backdrop_blur.is_some() != value_backdrop_blur.is_some()
                    || background.is_some() != value_background.is_some()
                    || border.is_some() != value_border.is_some()
                    || texture.is_some() != value_texture.is_some()
                    || brush.is_some() != value_brush.is_some()
                {
                    return None;
                }
                if let (Some(slot), Some(primitive)) = (*backdrop_blur, value_backdrop_blur) {
                    writes.push(ReactiveSlotWrite::BackdropBlur { slot, primitive });
                }
                if let (Some(slot), Some((rect, _))) = (*background, value_background) {
                    writes.push(ReactiveSlotWrite::ShapeRect { slot, rect });
                }
                if let (Some(slot), Some((rect, _, _))) = (*border, value_border) {
                    writes.push(ReactiveSlotWrite::ShapeRect { slot, rect });
                }
                if let (
                    Some(slot),
                    Some((texture, media_key, frame, corner_radius, opacity, clip_rect, clip_mask)),
                ) = (*texture, value_texture)
                {
                    writes.push(ReactiveSlotWrite::Texture {
                        slot,
                        primitive: TexturePrimitive {
                            texture,
                            media_key,
                            frame,
                            quad: None,
                            uv_rect: None,
                            corner_radius,
                            opacity: opacity.clamp(0.0, 1.0),
                            clip_rect,
                            clip_mask,
                        },
                    });
                }
                if let (Some(slot), Some(primitive)) = (*brush, value_brush) {
                    writes.push(ReactiveSlotWrite::Brush { slot, primitive });
                }
            }
            (
                ReactiveSlotBindingKind::Scale {
                    backdrop_blur,
                    background,
                    border,
                    texture,
                    brush,
                },
                ReactiveScenePropertyValue::Scale {
                    background: value_background,
                    border: value_border,
                    backdrop_blur: value_backdrop_blur,
                    brush: value_brush,
                    texture: value_texture,
                },
            ) => {
                if backdrop_blur.is_some() != value_backdrop_blur.is_some()
                    || background.is_some() != value_background.is_some()
                    || border.is_some() != value_border.is_some()
                    || texture.is_some() != value_texture.is_some()
                    || brush.is_some() != value_brush.is_some()
                {
                    return None;
                }
                if let (Some(slot), Some(primitive)) = (*backdrop_blur, value_backdrop_blur) {
                    writes.push(ReactiveSlotWrite::BackdropBlur { slot, primitive });
                }
                if let (Some(slot), Some((rect, _, corner_radius))) =
                    (*background, value_background)
                {
                    writes.push(ReactiveSlotWrite::ShapeRect { slot, rect });
                    writes.push(ReactiveSlotWrite::ShapeCornerRadius {
                        slot,
                        corner_radius,
                    });
                }
                if let (Some(slot), Some((rect, stroke_width, _, corner_radius))) =
                    (*border, value_border)
                {
                    writes.push(ReactiveSlotWrite::ShapeRect { slot, rect });
                    writes.push(ReactiveSlotWrite::ShapeStrokeWidth { slot, stroke_width });
                    writes.push(ReactiveSlotWrite::ShapeCornerRadius {
                        slot,
                        corner_radius,
                    });
                }
                if let (
                    Some(slot),
                    Some((texture, media_key, frame, corner_radius, opacity, clip_rect, clip_mask)),
                ) = (*texture, value_texture)
                {
                    writes.push(ReactiveSlotWrite::Texture {
                        slot,
                        primitive: TexturePrimitive {
                            texture,
                            media_key,
                            frame,
                            quad: None,
                            uv_rect: None,
                            corner_radius,
                            opacity: opacity.clamp(0.0, 1.0),
                            clip_rect,
                            clip_mask,
                        },
                    });
                }
                if let (Some(slot), Some(primitive)) = (*brush, value_brush) {
                    writes.push(ReactiveSlotWrite::Brush { slot, primitive });
                }
            }
            (
                ReactiveSlotBindingKind::TextColor { slot },
                ReactiveScenePropertyValue::TextColor { color },
            ) => {
                writes.push(ReactiveSlotWrite::TextColor { slot: *slot, color });
            }
            (
                ReactiveSlotBindingKind::TextContent { slot },
                ReactiveScenePropertyValue::TextContent {
                    content,
                    font_family,
                },
            ) => {
                writes.push(ReactiveSlotWrite::TextContent {
                    slot: *slot,
                    content,
                    font_family,
                });
            }
            (
                ReactiveSlotBindingKind::TextInputContent { slot },
                ReactiveScenePropertyValue::TextInputContent(primitive),
            ) => {
                writes.push(ReactiveSlotWrite::TextPrimitive {
                    slot: *slot,
                    primitive,
                });
            }
            (
                ReactiveSlotBindingKind::TextureOpacity { slot },
                ReactiveScenePropertyValue::TextureOpacity { opacity, .. },
            ) => {
                writes.push(ReactiveSlotWrite::TextureOpacity {
                    slot: *slot,
                    opacity: opacity.clamp(0.0, 1.0),
                });
            }
            (
                ReactiveSlotBindingKind::Texture { slot },
                ReactiveScenePropertyValue::Texture {
                    texture,
                    media_key,
                    frame,
                    corner_radius,
                    opacity,
                    clip_rect,
                    clip_mask,
                },
            ) => {
                writes.push(ReactiveSlotWrite::Texture {
                    slot: *slot,
                    primitive: TexturePrimitive {
                        texture,
                        media_key,
                        frame,
                        quad: None,
                        uv_rect: None,
                        corner_radius,
                        opacity: opacity.clamp(0.0, 1.0),
                        clip_rect,
                        clip_mask,
                    },
                });
            }
            (
                ReactiveSlotBindingKind::ProgressFill { fill, label },
                ReactiveScenePropertyValue::ProgressFill {
                    fill_rect,
                    label: value_label,
                    ..
                },
            ) => {
                if label.is_some() != value_label.is_some() {
                    return None;
                }
                writes.push(ReactiveSlotWrite::ShapeRect {
                    slot: *fill,
                    rect: fill_rect,
                });
                if let (Some(slot), Some(label)) = (*label, value_label) {
                    writes.push(ReactiveSlotWrite::TextContent {
                        slot,
                        content: label.content,
                        font_family: label.font_family,
                    });
                }
            }
            (
                ReactiveSlotBindingKind::SliderValue {
                    active,
                    thumb,
                    thumb_border,
                    label,
                    hit_index,
                    widget_id: binding_widget_id,
                },
                ReactiveScenePropertyValue::SliderValue {
                    widget_id,
                    value,
                    active_rect,
                    thumb_rect,
                    thumb_border: value_thumb_border,
                    label: value_label,
                    track_rect,
                    ..
                },
            ) => {
                if *binding_widget_id != widget_id
                    || thumb_border.is_some() != value_thumb_border.is_some()
                    || label.is_some() != value_label.is_some()
                {
                    return None;
                }
                writes.push(ReactiveSlotWrite::ShapeRect {
                    slot: *active,
                    rect: active_rect,
                });
                writes.push(ReactiveSlotWrite::ShapeRect {
                    slot: *thumb,
                    rect: thumb_rect,
                });
                if let (Some(slot), Some(_)) = (*thumb_border, value_thumb_border) {
                    writes.push(ReactiveSlotWrite::ShapeRect {
                        slot,
                        rect: thumb_rect,
                    });
                }
                if let (Some(slot), Some(label)) = (*label, value_label) {
                    writes.push(ReactiveSlotWrite::TextContent {
                        slot,
                        content: label.content,
                        font_family: label.font_family,
                    });
                }
                hit_write = Some(ReactiveHitWrite::Slider {
                    hit_index: *hit_index,
                    widget_id,
                    value,
                    track_rect,
                    thumb_rect,
                });
            }
            _ => return None,
        }
        Some(ReactiveSlotPatch { writes, hit_write })
    }
}

fn can_write_shape_option<VM>(
    computed: &ComputedScene<VM>,
    offset: &SceneCounts,
    slot: Option<ShapePrimitiveSlot>,
) -> bool {
    slot.map(|slot| computed.can_write_shape_color_slot(offset, slot))
        .unwrap_or(true)
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn try_update_reactive_transform_records(
        &mut self,
        targets: &[(WidgetId, PropertySlot)],
        now: Instant,
    ) -> bool {
        if targets.is_empty()
            || targets
                .iter()
                .any(|(_, property)| *property != PropertySlot::Offset)
        {
            return false;
        }

        let mut unique_targets = Vec::with_capacity(targets.len());
        let mut seen = HashSet::new();
        for &(widget_id, _) in targets {
            if seen.insert(widget_id) {
                unique_targets.push(widget_id);
            }
        }

        let theme = self.animated_theme(now);
        let viewport = self.viewport_rect();
        let active_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        let widget_states = self.widget_state_map(active_scrollbar);
        let mut updates = Vec::with_capacity(unique_targets.len());
        let mut changed = false;
        for widget_id in unique_targets {
            let Some(cached) = self.cached_scene.as_ref() else {
                return false;
            };
            let Some(layout) = cached.layout.as_ref() else {
                return false;
            };
            let Some(record) = cached.computed.transform_records.get(&widget_id) else {
                return false;
            };
            let Some(visual_context) = cached.visual_contexts.get(&widget_id).copied() else {
                return false;
            };
            let Some(offset) = layout.resolve_reactive_transform_offset(
                widget_id,
                &self.font_manager,
                &theme,
                &self.media_manager,
                &mut self.animation_engine,
                self.reduced_motion,
                visual_context,
                self.hovered_scrollbar,
                active_scrollbar,
                &widget_states,
                &self.select_open_states,
                &self.scroll_states,
                &self.virtual_states,
                viewport,
                now,
                &self.config.style_sheet,
            ) else {
                return false;
            };
            changed |= record.current_offset != offset;
            updates.push((widget_id, offset));
        }
        if !changed {
            return false;
        }

        let Some(cached) = self.cached_scene.as_mut() else {
            return false;
        };
        for (widget_id, offset) in updates {
            let Some(record) = cached.computed.transform_records.get_mut(&widget_id) else {
                return false;
            };
            record.current_offset = offset;
            if let Some(chunk) = cached.scene_chunks.get_mut(&widget_id) {
                if let Some(record) = chunk.transform_records.get_mut(&widget_id) {
                    record.current_offset = offset;
                }
            }
            if let Some(parts) = cached.scene_chunk_parts.get_mut(&widget_id) {
                if let Some(record) = parts.before_children.transform_records.get_mut(&widget_id) {
                    record.current_offset = offset;
                }
                if let Some(record) = parts.after_children.transform_records.get_mut(&widget_id) {
                    record.current_offset = offset;
                }
            }
        }
        cached.computed_valid = true;
        true
    }

    pub(super) fn try_patch_reactive_property_slots(
        &mut self,
        targets: &[(WidgetId, PropertySlot)],
        now: Instant,
    ) -> bool {
        if targets.is_empty() {
            return false;
        }

        let mut unique_targets = Vec::with_capacity(targets.len());
        let mut seen = HashSet::new();
        for &(widget_id, property) in targets {
            if seen.insert((widget_id, property)) {
                unique_targets.push((widget_id, property));
            }
        }

        let mut plans = Vec::with_capacity(unique_targets.len());
        for (widget_id, property) in unique_targets {
            let Some(binding) = self.cached_scene.as_ref().and_then(|cached| {
                cached
                    .reactive_slot_bindings
                    .get(&(widget_id, property))
                    .cloned()
            }) else {
                return false;
            };
            let Some(value) = self.resolve_reactive_slot_value(widget_id, property, now) else {
                return false;
            };
            let Some(patch) = binding.local.patch_for(value) else {
                return false;
            };
            plans.push((binding, patch));
        }

        let mut media_updates = Vec::new();
        for (binding, patch) in &plans {
            for write in &patch.writes {
                if let Some(update) = reactive_media_texture_binding_update(binding, write) {
                    media_updates.push(update);
                }
            }
        }

        let Some(cached) = self.cached_scene.as_mut() else {
            return false;
        };
        for (binding, patch) in plans {
            if !write_reactive_slot_patch(cached, &binding, &patch) {
                return false;
            }
        }
        cached.computed_valid = true;
        let _ = cached;
        self.sync_reactive_media_texture_bindings(&media_updates)
    }

    pub(super) fn rebuild_reactive_slot_bindings(&mut self, now: Instant) {
        let bindings = self.build_reactive_slot_bindings(now);
        let Some(cached) = self.cached_scene.as_mut() else {
            return;
        };
        cached.reactive_slot_bindings = bindings;
    }

    fn build_reactive_slot_bindings(
        &mut self,
        now: Instant,
    ) -> HashMap<(WidgetId, PropertySlot), ReactiveSlotBinding> {
        let Some(cached) = self.cached_scene.as_ref() else {
            return HashMap::new();
        };
        let Some(layout) = cached.layout.as_ref() else {
            return HashMap::new();
        };
        let owners = cached.dependencies.property_owners();
        let mut targets = Vec::with_capacity(owners.len());
        for owner in owners {
            let Some(property) = owner.property else {
                continue;
            };
            let widget_id = WidgetId::from_raw(owner.widget_id);
            if layout.path_for(widget_id).is_none() {
                continue;
            }
            match owner.phase {
                DependencyPhase::Scene => targets.push((widget_id, property)),
                DependencyPhase::Layout
                    if property == PropertySlot::Offset
                        && layout.can_patch_layout_dependency_as_scene(widget_id) =>
                {
                    targets.push((widget_id, property));
                }
                DependencyPhase::Structure | DependencyPhase::Layout => {}
            }
        }
        targets.sort_by_key(|(widget_id, property)| (widget_id.raw(), *property as u8));
        targets.dedup();

        let mut bindings = HashMap::with_capacity(targets.len());
        for (widget_id, property) in targets {
            if let Some(binding) = self.build_reactive_slot_binding(widget_id, property, now) {
                bindings.insert((widget_id, property), binding);
            }
        }
        bindings
    }

    fn resolve_reactive_slot_value(
        &mut self,
        widget_id: WidgetId,
        property: PropertySlot,
        now: Instant,
    ) -> Option<ReactiveScenePropertyValue> {
        let theme = self.animated_theme(now);
        let viewport = self.viewport_rect();
        let active_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        let widget_states = self.widget_state_map(active_scrollbar);
        let cached = self.cached_scene.as_ref()?;
        let layout = cached.layout.as_ref()?;
        let visual_context = cached.visual_contexts.get(&widget_id).copied()?;
        layout.resolve_reactive_scene_property_value(
            widget_id,
            property,
            &self.font_manager,
            &theme,
            &self.media_manager,
            &mut self.animation_engine,
            self.reduced_motion,
            visual_context,
            self.hovered_scrollbar,
            active_scrollbar,
            &widget_states,
            &self.select_open_states,
            &self.scroll_states,
            &self.virtual_states,
            viewport,
            now,
            &self.config.style_sheet,
        )
    }

    fn build_reactive_slot_binding(
        &mut self,
        widget_id: WidgetId,
        property: PropertySlot,
        now: Instant,
    ) -> Option<ReactiveSlotBinding> {
        let value = self.resolve_reactive_slot_value(widget_id, property, now)?;
        let cached = self.cached_scene.as_ref()?;
        let layout = cached.layout.as_ref()?;
        let target_chunk = cached.scene_chunks.get(&widget_id)?;
        let (local_scene, has_chunk_part) =
            if let Some(parts) = cached.scene_chunk_parts.get(&widget_id) {
                (&parts.before_children, true)
            } else {
                (target_chunk, false)
            };
        let local = slot_binding_for_reactive_value(local_scene, value)?;
        let zero = SceneCounts::default();
        if !local.can_write(local_scene, &zero)
            || !local.can_write_hit(local_scene, 0)
            || !local.can_write(target_chunk, &zero)
            || !local.can_write_hit(target_chunk, 0)
        {
            return None;
        }

        let (root_offset, root_hit_offset, ancestor_offsets) = if widget_id == layout.root_id() {
            (SceneCounts::default(), 0, Vec::new())
        } else {
            let offsets = layout.scene_splice_ancestor_offsets(
                widget_id,
                &cached.scene_chunk_parts,
                &cached.scene_chunks,
            )?;
            let (root_id, root_offset, root_hit_offset, _) = offsets.first().copied()?;
            if root_id != layout.root_id() {
                return None;
            }
            let mut ancestor_offsets = Vec::with_capacity(offsets.len());
            for (ancestor_id, offset, hit_offset, _) in offsets {
                let ancestor_chunk = cached.scene_chunks.get(&ancestor_id)?;
                if !local.can_write(ancestor_chunk, &offset)
                    || !local.can_write_hit(ancestor_chunk, hit_offset)
                {
                    return None;
                }
                ancestor_offsets.push((ancestor_id, offset, hit_offset));
            }
            (root_offset, root_hit_offset, ancestor_offsets)
        };
        if !local.can_write(&cached.computed, &root_offset)
            || !local.can_write_hit(&cached.computed, root_hit_offset)
        {
            return None;
        }

        Some(ReactiveSlotBinding {
            widget_id,
            local,
            root_offset,
            root_hit_offset,
            ancestor_offsets,
            has_chunk_part,
        })
    }
}

fn write_reactive_slot_patch<VM: 'static>(
    cached: &mut CachedScene<VM>,
    binding: &ReactiveSlotBinding,
    patch: &ReactiveSlotPatch,
) -> bool {
    let zero = SceneCounts::default();
    if binding.has_chunk_part {
        let Some(parts) = cached.scene_chunk_parts.get_mut(&binding.widget_id) else {
            return false;
        };
        for write in &patch.writes {
            if !write.write(&mut parts.before_children, &zero) {
                return false;
            }
        }
        if let Some(hit_write) = patch.hit_write {
            if !hit_write.write(&mut parts.before_children, 0) {
                return false;
            }
        }
    }
    let Some(target_chunk) = cached.scene_chunks.get_mut(&binding.widget_id) else {
        return false;
    };
    for write in &patch.writes {
        if !write.write(target_chunk, &zero) {
            return false;
        }
    }
    if let Some(hit_write) = patch.hit_write {
        if !hit_write.write(target_chunk, 0) {
            return false;
        }
    }
    for (ancestor_id, offset, hit_offset) in &binding.ancestor_offsets {
        let Some(ancestor_chunk) = cached.scene_chunks.get_mut(ancestor_id) else {
            return false;
        };
        for write in &patch.writes {
            if !write.write(ancestor_chunk, offset) {
                return false;
            }
        }
        if let Some(hit_write) = patch.hit_write {
            if !hit_write.write(ancestor_chunk, *hit_offset) {
                return false;
            }
        }
    }
    for write in &patch.writes {
        if !write.write(&mut cached.computed, &binding.root_offset) {
            return false;
        }
    }
    if let Some(hit_write) = patch.hit_write {
        if !hit_write.write(&mut cached.computed, binding.root_hit_offset) {
            return false;
        }
    }
    if let Some(snapshot) = cached
        .layout
        .as_ref()
        .and_then(|layout| layout.lifecycle_snapshot(binding.widget_id))
    {
        if let Some(state) = cached.lifecycle_states.get_mut(&binding.widget_id) {
            state.snapshot = snapshot;
        }
    }
    true
}

fn reactive_media_texture_binding_update(
    binding: &ReactiveSlotBinding,
    write: &ReactiveSlotWrite,
) -> Option<ReactiveMediaTextureBindingUpdate> {
    let ReactiveSlotWrite::Texture { slot, primitive } = write else {
        return None;
    };
    Some(ReactiveMediaTextureBindingUpdate {
        widget_id: binding.widget_id,
        slot: *slot,
        media_key: primitive.media_key.clone(),
        frame: primitive.frame,
        root_offset: binding.root_offset,
        ancestor_offsets: binding
            .ancestor_offsets
            .iter()
            .map(|(widget_id, offset, _)| (*widget_id, *offset))
            .collect(),
        has_chunk_part: binding.has_chunk_part,
    })
}

struct LocalReactiveSlotPlan {
    writes: Vec<ReactiveSlotWrite>,
    hit_write: Option<ReactiveHitWrite>,
}

fn slot_binding_for_reactive_value<VM>(
    computed: &ComputedScene<VM>,
    value: ReactiveScenePropertyValue,
) -> Option<LocalReactiveSlotBinding> {
    let plan = slot_write_for_reactive_value(computed, value.clone())?;
    let kind = slot_binding_kind_from_plan(value, &plan)?;
    Some(LocalReactiveSlotBinding { kind })
}

fn slot_binding_kind_from_plan(
    value: ReactiveScenePropertyValue,
    plan: &LocalReactiveSlotPlan,
) -> Option<ReactiveSlotBindingKind> {
    match value {
        ReactiveScenePropertyValue::ShapeFillColor { .. } => {
            let [ReactiveSlotWrite::ShapeColor { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::ShapeFillColor { slot: *slot })
        }
        ReactiveScenePropertyValue::ShapeStrokeColor { .. } => {
            let [ReactiveSlotWrite::ShapeColor { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::ShapeStrokeColor { slot: *slot })
        }
        ReactiveScenePropertyValue::BackdropBlur(_) => {
            let [ReactiveSlotWrite::BackdropBlur { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::BackdropBlur { slot: *slot })
        }
        ReactiveScenePropertyValue::Brush(_) => {
            let [ReactiveSlotWrite::Brush { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::Brush { slot: *slot })
        }
        ReactiveScenePropertyValue::BorderRadius { background, border } => {
            no_hit(plan)?;
            let mut writes = plan.writes.iter();
            let background = if background.is_some() {
                Some(next_shape_corner_radius_slot(&mut writes)?)
            } else {
                None
            };
            let border = if border.is_some() {
                Some(next_shape_corner_radius_slot(&mut writes)?)
            } else {
                None
            };
            writes.next().is_none().then_some(())?;
            Some(ReactiveSlotBindingKind::BorderRadius { background, border })
        }
        ReactiveScenePropertyValue::BorderWidth {
            background, border, ..
        } => {
            no_hit(plan)?;
            let mut writes = plan.writes.iter();
            let background = if background.is_some() {
                let rect_slot = next_shape_rect_slot(&mut writes)?;
                let radius_slot = next_shape_corner_radius_slot(&mut writes)?;
                Some((rect_slot == radius_slot).then_some(rect_slot)?)
            } else {
                None
            };
            let border = if border.is_some() {
                Some(next_shape_stroke_width_slot(&mut writes)?)
            } else {
                None
            };
            writes.next().is_none().then_some(())?;
            Some(ReactiveSlotBindingKind::BorderWidth { background, border })
        }
        ReactiveScenePropertyValue::Opacity {
            background,
            border,
            text,
        } => {
            no_hit(plan)?;
            let mut writes = plan.writes.iter();
            let background = if background.is_some() {
                Some(next_shape_color_slot(&mut writes)?)
            } else {
                None
            };
            let border = if border.is_some() {
                Some(next_shape_color_slot(&mut writes)?)
            } else {
                None
            };
            let text = if text.is_some() {
                Some(next_text_color_slot(&mut writes)?)
            } else {
                None
            };
            writes.next().is_none().then_some(())?;
            Some(ReactiveSlotBindingKind::Opacity {
                background,
                border,
                text,
            })
        }
        ReactiveScenePropertyValue::Offset {
            background,
            border,
            backdrop_blur,
            brush,
            texture,
        } => {
            no_hit(plan)?;
            let mut writes = plan.writes.iter();
            let backdrop_blur = if backdrop_blur.is_some() {
                Some(next_backdrop_blur_slot(&mut writes)?)
            } else {
                None
            };
            let background = if background.is_some() {
                Some(next_shape_rect_slot(&mut writes)?)
            } else {
                None
            };
            let border = if border.is_some() {
                Some(next_shape_rect_slot(&mut writes)?)
            } else {
                None
            };
            let texture = if texture.is_some() {
                Some(next_texture_slot(&mut writes)?)
            } else {
                None
            };
            let brush = if brush.is_some() {
                Some(next_brush_slot(&mut writes)?)
            } else {
                None
            };
            writes.next().is_none().then_some(())?;
            Some(ReactiveSlotBindingKind::Offset {
                backdrop_blur,
                background,
                border,
                texture,
                brush,
            })
        }
        ReactiveScenePropertyValue::Scale {
            background,
            border,
            backdrop_blur,
            brush,
            texture,
        } => {
            no_hit(plan)?;
            let mut writes = plan.writes.iter();
            let backdrop_blur = if backdrop_blur.is_some() {
                Some(next_backdrop_blur_slot(&mut writes)?)
            } else {
                None
            };
            let background = if background.is_some() {
                let rect_slot = next_shape_rect_slot(&mut writes)?;
                let radius_slot = next_shape_corner_radius_slot(&mut writes)?;
                Some((rect_slot == radius_slot).then_some(rect_slot)?)
            } else {
                None
            };
            let border = if border.is_some() {
                let rect_slot = next_shape_rect_slot(&mut writes)?;
                let width_slot = next_shape_stroke_width_slot(&mut writes)?;
                let radius_slot = next_shape_corner_radius_slot(&mut writes)?;
                Some((rect_slot == width_slot && rect_slot == radius_slot).then_some(rect_slot)?)
            } else {
                None
            };
            let texture = if texture.is_some() {
                Some(next_texture_slot(&mut writes)?)
            } else {
                None
            };
            let brush = if brush.is_some() {
                Some(next_brush_slot(&mut writes)?)
            } else {
                None
            };
            writes.next().is_none().then_some(())?;
            Some(ReactiveSlotBindingKind::Scale {
                backdrop_blur,
                background,
                border,
                texture,
                brush,
            })
        }
        ReactiveScenePropertyValue::TextColor { .. } => {
            let [ReactiveSlotWrite::TextColor { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::TextColor { slot: *slot })
        }
        ReactiveScenePropertyValue::TextContent { .. } => {
            let [ReactiveSlotWrite::TextContent { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::TextContent { slot: *slot })
        }
        ReactiveScenePropertyValue::TextInputContent(_) => {
            let [ReactiveSlotWrite::TextPrimitive { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::TextInputContent { slot: *slot })
        }
        ReactiveScenePropertyValue::TextureOpacity { .. } => {
            let [ReactiveSlotWrite::TextureOpacity { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::TextureOpacity { slot: *slot })
        }
        ReactiveScenePropertyValue::Texture { .. } => {
            let [ReactiveSlotWrite::Texture { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::Texture { slot: *slot })
        }
        ReactiveScenePropertyValue::ProgressFill { label, .. } => {
            no_hit(plan)?;
            let mut writes = plan.writes.iter();
            let fill = next_shape_rect_slot(&mut writes)?;
            let label = if label.is_some() {
                Some(next_text_content_slot(&mut writes)?)
            } else {
                None
            };
            writes.next().is_none().then_some(())?;
            Some(ReactiveSlotBindingKind::ProgressFill { fill, label })
        }
        ReactiveScenePropertyValue::SliderValue {
            widget_id,
            thumb_border,
            label,
            ..
        } => {
            let mut writes = plan.writes.iter();
            let active = next_shape_rect_slot(&mut writes)?;
            let thumb = next_shape_rect_slot(&mut writes)?;
            let thumb_border = if thumb_border.is_some() {
                Some(next_shape_rect_slot(&mut writes)?)
            } else {
                None
            };
            let label = if label.is_some() {
                Some(next_text_content_slot(&mut writes)?)
            } else {
                None
            };
            writes.next().is_none().then_some(())?;
            let Some(ReactiveHitWrite::Slider {
                hit_index,
                widget_id: hit_widget_id,
                ..
            }) = plan.hit_write
            else {
                return None;
            };
            (hit_widget_id == widget_id).then_some(())?;
            Some(ReactiveSlotBindingKind::SliderValue {
                active,
                thumb,
                thumb_border,
                label,
                hit_index,
                widget_id,
            })
        }
    }
}

fn no_hit(plan: &LocalReactiveSlotPlan) -> Option<()> {
    plan.hit_write.is_none().then_some(())
}

fn next_shape_color_slot<'a>(
    writes: &mut impl Iterator<Item = &'a ReactiveSlotWrite>,
) -> Option<ShapePrimitiveSlot> {
    match writes.next()? {
        ReactiveSlotWrite::ShapeColor { slot, .. } => Some(*slot),
        _ => None,
    }
}

fn next_shape_rect_slot<'a>(
    writes: &mut impl Iterator<Item = &'a ReactiveSlotWrite>,
) -> Option<ShapePrimitiveSlot> {
    match writes.next()? {
        ReactiveSlotWrite::ShapeRect { slot, .. } => Some(*slot),
        _ => None,
    }
}

fn next_shape_corner_radius_slot<'a>(
    writes: &mut impl Iterator<Item = &'a ReactiveSlotWrite>,
) -> Option<ShapePrimitiveSlot> {
    match writes.next()? {
        ReactiveSlotWrite::ShapeCornerRadius { slot, .. } => Some(*slot),
        _ => None,
    }
}

fn next_shape_stroke_width_slot<'a>(
    writes: &mut impl Iterator<Item = &'a ReactiveSlotWrite>,
) -> Option<ShapePrimitiveSlot> {
    match writes.next()? {
        ReactiveSlotWrite::ShapeStrokeWidth { slot, .. } => Some(*slot),
        _ => None,
    }
}

fn next_backdrop_blur_slot<'a>(
    writes: &mut impl Iterator<Item = &'a ReactiveSlotWrite>,
) -> Option<BackdropBlurPrimitiveSlot> {
    match writes.next()? {
        ReactiveSlotWrite::BackdropBlur { slot, .. } => Some(*slot),
        _ => None,
    }
}

fn next_brush_slot<'a>(
    writes: &mut impl Iterator<Item = &'a ReactiveSlotWrite>,
) -> Option<BrushPrimitiveSlot> {
    match writes.next()? {
        ReactiveSlotWrite::Brush { slot, .. } => Some(*slot),
        _ => None,
    }
}

fn next_text_color_slot<'a>(
    writes: &mut impl Iterator<Item = &'a ReactiveSlotWrite>,
) -> Option<TextPrimitiveSlot> {
    match writes.next()? {
        ReactiveSlotWrite::TextColor { slot, .. } => Some(*slot),
        _ => None,
    }
}

fn next_text_content_slot<'a>(
    writes: &mut impl Iterator<Item = &'a ReactiveSlotWrite>,
) -> Option<TextPrimitiveSlot> {
    match writes.next()? {
        ReactiveSlotWrite::TextContent { slot, .. } => Some(*slot),
        _ => None,
    }
}

fn next_texture_slot<'a>(
    writes: &mut impl Iterator<Item = &'a ReactiveSlotWrite>,
) -> Option<TexturePrimitiveSlot> {
    match writes.next()? {
        ReactiveSlotWrite::Texture { slot, .. } => Some(*slot),
        _ => None,
    }
}

fn slot_write_for_reactive_value<VM>(
    computed: &ComputedScene<VM>,
    value: ReactiveScenePropertyValue,
) -> Option<LocalReactiveSlotPlan> {
    match value {
        ReactiveScenePropertyValue::ShapeFillColor { rect, color } => {
            let slots = computed
                .scene
                .matching_shape_slots(|shape| shape.stroke_width == 0.0 && shape.rect == rect);
            if slots.len() == 1 {
                Some(LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::ShapeColor {
                        slot: slots[0],
                        color,
                    }],
                    hit_write: None,
                })
            } else {
                None
            }
        }
        ReactiveScenePropertyValue::ShapeStrokeColor {
            rect,
            stroke_width,
            color,
        } => {
            if stroke_width <= 0.0 {
                return None;
            }
            let slots = computed.scene.matching_shape_slots(|shape| {
                shape.rect == rect && (shape.stroke_width - stroke_width).abs() <= f32::EPSILON
            });
            if slots.len() == 1 {
                Some(LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::ShapeColor {
                        slot: slots[0],
                        color,
                    }],
                    hit_write: None,
                })
            } else {
                None
            }
        }
        ReactiveScenePropertyValue::BackdropBlur(primitive) => {
            if primitive.rect.is_empty() {
                return None;
            }
            let slots = computed.scene.matching_backdrop_blur_slots(|blur| {
                blur.rect == primitive.rect
                    && (blur.corner_radius - primitive.corner_radius).abs() <= f32::EPSILON
            });
            if slots.len() == 1 {
                Some(LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::BackdropBlur {
                        slot: slots[0],
                        primitive,
                    }],
                    hit_write: None,
                })
            } else {
                None
            }
        }
        ReactiveScenePropertyValue::Brush(primitive) => {
            let slots = computed.scene.matching_brush_slots(|brush| {
                brush.rect == primitive.rect
                    && (brush.corner_radius - primitive.corner_radius).abs() <= f32::EPSILON
            });
            if slots.len() == 1 {
                Some(LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::Brush {
                        slot: slots[0],
                        primitive,
                    }],
                    hit_write: None,
                })
            } else {
                None
            }
        }
        ReactiveScenePropertyValue::BorderRadius { background, border } => {
            let mut writes = Vec::new();
            let mut used_slots = Vec::new();
            if let Some((rect, color, corner_radius)) = background {
                let slots = computed.scene.matching_shape_slots(|shape| {
                    shape.stroke_width == 0.0 && shape.rect == rect && shape.color == color
                });
                if slots.len() != 1 {
                    return None;
                }
                used_slots.push(slots[0]);
                writes.push(ReactiveSlotWrite::ShapeCornerRadius {
                    slot: slots[0],
                    corner_radius,
                });
            }
            if let Some((rect, stroke_width, color, corner_radius)) = border {
                let slots = computed.scene.matching_shape_slots(|shape| {
                    shape.rect == rect
                        && shape.color == color
                        && (shape.stroke_width - stroke_width).abs() <= f32::EPSILON
                });
                let slots = slots
                    .into_iter()
                    .filter(|slot| !used_slots.contains(slot))
                    .collect::<Vec<_>>();
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::ShapeCornerRadius {
                    slot: slots[0],
                    corner_radius,
                });
            }
            if writes.is_empty() {
                None
            } else {
                Some(LocalReactiveSlotPlan {
                    writes,
                    hit_write: None,
                })
            }
        }
        ReactiveScenePropertyValue::BorderWidth {
            frame,
            background,
            border,
        } => {
            let mut writes = Vec::new();
            let mut used_slots = Vec::new();
            if let Some((rect, color, corner_radius)) = background {
                let slots = computed.scene.matching_shape_slots(|shape| {
                    shape.stroke_width == 0.0
                        && shape.color == color
                        && rect_within(shape.rect, frame)
                });
                if slots.len() != 1 {
                    return None;
                }
                used_slots.push(slots[0]);
                writes.push(ReactiveSlotWrite::ShapeRect {
                    slot: slots[0],
                    rect,
                });
                writes.push(ReactiveSlotWrite::ShapeCornerRadius {
                    slot: slots[0],
                    corner_radius,
                });
            }
            if let Some((rect, color, stroke_width)) = border {
                let slots = computed.scene.matching_shape_slots(|shape| {
                    shape.rect == rect && shape.color == color && shape.stroke_width > 0.0
                });
                let slots = slots
                    .into_iter()
                    .filter(|slot| !used_slots.contains(slot))
                    .collect::<Vec<_>>();
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::ShapeStrokeWidth {
                    slot: slots[0],
                    stroke_width,
                });
            }
            if writes.is_empty() {
                None
            } else {
                Some(LocalReactiveSlotPlan {
                    writes,
                    hit_write: None,
                })
            }
        }
        ReactiveScenePropertyValue::Opacity {
            background,
            border,
            text,
        } => {
            let mut writes = Vec::new();
            let mut used_shape_slots = Vec::new();
            if let Some((rect, color)) = background {
                let slots = computed
                    .scene
                    .matching_shape_slots(|shape| shape.stroke_width == 0.0 && shape.rect == rect);
                if slots.len() != 1 {
                    return None;
                }
                used_shape_slots.push(slots[0]);
                writes.push(ReactiveSlotWrite::ShapeColor {
                    slot: slots[0],
                    color,
                });
            }
            if let Some((rect, stroke_width, color)) = border {
                if stroke_width <= 0.0 {
                    return None;
                }
                let slots = computed.scene.matching_shape_slots(|shape| {
                    shape.rect == rect && (shape.stroke_width - stroke_width).abs() <= f32::EPSILON
                });
                let slots = slots
                    .into_iter()
                    .filter(|slot| !used_shape_slots.contains(slot))
                    .collect::<Vec<_>>();
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::ShapeColor {
                    slot: slots[0],
                    color,
                });
            }
            if let Some(color) = text {
                let slots = computed
                    .scene
                    .matching_text_slots(|text| text.rich_spans.is_none() && !text.force_color);
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::TextColor {
                    slot: slots[0],
                    color,
                });
            }
            if writes.is_empty() {
                None
            } else {
                Some(LocalReactiveSlotPlan {
                    writes,
                    hit_write: None,
                })
            }
        }
        ReactiveScenePropertyValue::Offset {
            background,
            border,
            backdrop_blur,
            brush,
            texture,
        } => {
            let mut writes = Vec::new();
            let mut used_shape_slots = Vec::new();
            if let Some(primitive) = backdrop_blur {
                let slots = computed.scene.matching_backdrop_blur_slots(|_| true);
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::BackdropBlur {
                    slot: slots[0],
                    primitive,
                });
            }
            if let Some((rect, color)) = background {
                let slots = computed.scene.matching_shape_slots(|shape| {
                    shape.stroke_width == 0.0
                        && shape.color == color
                        && same_rect_size(shape.rect, rect)
                });
                if slots.len() != 1 {
                    return None;
                }
                used_shape_slots.push(slots[0]);
                writes.push(ReactiveSlotWrite::ShapeRect {
                    slot: slots[0],
                    rect,
                });
            }
            if let Some((rect, stroke_width, color)) = border {
                let slots = computed.scene.matching_shape_slots(|shape| {
                    shape.color == color
                        && (shape.stroke_width - stroke_width).abs() <= f32::EPSILON
                        && same_rect_size(shape.rect, rect)
                });
                let slots = slots
                    .into_iter()
                    .filter(|slot| !used_shape_slots.contains(slot))
                    .collect::<Vec<_>>();
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::ShapeRect {
                    slot: slots[0],
                    rect,
                });
            }
            if let Some((texture, media_key, frame, corner_radius, opacity, clip_rect, clip_mask)) =
                texture
            {
                let slots = computed.scene.matching_texture_slots(|texture| {
                    texture.quad.is_none() && texture.uv_rect.is_none()
                });
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::Texture {
                    slot: slots[0],
                    primitive: TexturePrimitive {
                        texture,
                        media_key,
                        frame,
                        quad: None,
                        uv_rect: None,
                        corner_radius,
                        opacity: opacity.clamp(0.0, 1.0),
                        clip_rect,
                        clip_mask,
                    },
                });
            }
            if let Some(primitive) = brush {
                let slots = computed.scene.matching_brush_slots(|_| true);
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::Brush {
                    slot: slots[0],
                    primitive,
                });
            }
            if writes.is_empty() {
                None
            } else {
                Some(LocalReactiveSlotPlan {
                    writes,
                    hit_write: None,
                })
            }
        }
        ReactiveScenePropertyValue::Scale {
            background,
            border,
            backdrop_blur,
            brush,
            texture,
        } => {
            let mut writes = Vec::new();
            let mut used_shape_slots = Vec::new();
            if let Some(primitive) = backdrop_blur {
                let slots = computed.scene.matching_backdrop_blur_slots(|_| true);
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::BackdropBlur {
                    slot: slots[0],
                    primitive,
                });
            }
            if let Some((rect, color, corner_radius)) = background {
                let slots = computed.scene.matching_shape_slots(|shape| {
                    shape.stroke_width == 0.0 && shape.color == color
                });
                if slots.len() != 1 {
                    return None;
                }
                used_shape_slots.push(slots[0]);
                writes.push(ReactiveSlotWrite::ShapeRect {
                    slot: slots[0],
                    rect,
                });
                writes.push(ReactiveSlotWrite::ShapeCornerRadius {
                    slot: slots[0],
                    corner_radius,
                });
            }
            if let Some((rect, stroke_width, color, corner_radius)) = border {
                let slots = computed
                    .scene
                    .matching_shape_slots(|shape| shape.color == color && shape.stroke_width > 0.0);
                let slots = slots
                    .into_iter()
                    .filter(|slot| !used_shape_slots.contains(slot))
                    .collect::<Vec<_>>();
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::ShapeRect {
                    slot: slots[0],
                    rect,
                });
                writes.push(ReactiveSlotWrite::ShapeStrokeWidth {
                    slot: slots[0],
                    stroke_width,
                });
                writes.push(ReactiveSlotWrite::ShapeCornerRadius {
                    slot: slots[0],
                    corner_radius,
                });
            }
            if let Some((texture, media_key, frame, corner_radius, opacity, clip_rect, clip_mask)) =
                texture
            {
                let slots = computed.scene.matching_texture_slots(|texture| {
                    texture.quad.is_none() && texture.uv_rect.is_none()
                });
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::Texture {
                    slot: slots[0],
                    primitive: TexturePrimitive {
                        texture,
                        media_key,
                        frame,
                        quad: None,
                        uv_rect: None,
                        corner_radius,
                        opacity: opacity.clamp(0.0, 1.0),
                        clip_rect,
                        clip_mask,
                    },
                });
            }
            if let Some(primitive) = brush {
                let slots = computed.scene.matching_brush_slots(|_| true);
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::Brush {
                    slot: slots[0],
                    primitive,
                });
            }
            if writes.is_empty() {
                None
            } else {
                Some(LocalReactiveSlotPlan {
                    writes,
                    hit_write: None,
                })
            }
        }
        ReactiveScenePropertyValue::TextColor { color } => {
            let slots = computed
                .scene
                .matching_text_slots(|text| text.rich_spans.is_none() && !text.force_color);
            if slots.len() == 1 {
                Some(LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::TextColor {
                        slot: slots[0],
                        color,
                    }],
                    hit_write: None,
                })
            } else {
                None
            }
        }
        ReactiveScenePropertyValue::TextContent {
            content,
            font_family,
        } => {
            let slots = computed
                .scene
                .matching_text_slots(|text| text.rich_spans.is_none() && !text.force_color);
            if slots.len() == 1 {
                Some(LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::TextContent {
                        slot: slots[0],
                        content,
                        font_family,
                    }],
                    hit_write: None,
                })
            } else {
                None
            }
        }
        ReactiveScenePropertyValue::TextInputContent(primitive) => {
            let slots = computed
                .scene
                .matching_text_slots(|text| text.rich_spans.is_none() && !text.force_color);
            if slots.len() == 1 {
                Some(LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::TextPrimitive {
                        slot: slots[0],
                        primitive,
                    }],
                    hit_write: None,
                })
            } else {
                None
            }
        }
        ReactiveScenePropertyValue::TextureOpacity {
            frame,
            corner_radius,
            opacity,
        } => {
            let opacity = opacity.clamp(0.0, 1.0);
            let slots = computed.scene.matching_texture_slots(|texture| {
                texture.frame == frame
                    && (texture.corner_radius - corner_radius).abs() <= f32::EPSILON
            });
            if slots.len() == 1 {
                Some(LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::TextureOpacity {
                        slot: slots[0],
                        opacity,
                    }],
                    hit_write: None,
                })
            } else {
                None
            }
        }
        ReactiveScenePropertyValue::Texture {
            texture,
            media_key,
            frame,
            corner_radius,
            opacity,
            clip_rect,
            clip_mask,
        } => {
            let slots = computed.scene.matching_texture_slots(|texture| {
                texture.quad.is_none() && texture.uv_rect.is_none()
            });
            if slots.len() == 1 {
                Some(LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::Texture {
                        slot: slots[0],
                        primitive: TexturePrimitive {
                            texture,
                            media_key,
                            frame,
                            quad: None,
                            uv_rect: None,
                            corner_radius,
                            opacity: opacity.clamp(0.0, 1.0),
                            clip_rect,
                            clip_mask,
                        },
                    }],
                    hit_write: None,
                })
            } else {
                None
            }
        }
        ReactiveScenePropertyValue::ProgressFill {
            track_rect,
            fill_rect,
            track_color,
            fill_color,
            label,
        } => {
            let track_slots = computed.scene.matching_shape_slots(|shape| {
                shape.stroke_width == 0.0 && shape.rect == track_rect && shape.color == track_color
            });
            if track_slots.len() != 1 {
                return None;
            }
            let track_slot = track_slots[0];
            let fill_slots = computed.scene.matching_shape_slots(|shape| {
                shape.stroke_width == 0.0
                    && shape.color == fill_color
                    && shape.rect.x == track_rect.x
                    && shape.rect.y == track_rect.y
                    && shape.rect.height == track_rect.height
                    && shape.rect.width <= track_rect.width
            });
            let fill_slots = fill_slots
                .into_iter()
                .filter(|slot| *slot != track_slot)
                .collect::<Vec<_>>();
            if fill_slots.len() == 1 {
                let mut writes = vec![ReactiveSlotWrite::ShapeRect {
                    slot: fill_slots[0],
                    rect: fill_rect,
                }];
                if let Some(label) = label {
                    let label_slots = computed.scene.matching_text_slots(|text| {
                        text.rich_spans.is_none() && !text.force_color && text.frame == label.frame
                    });
                    if label_slots.len() != 1 {
                        return None;
                    }
                    writes.push(ReactiveSlotWrite::TextContent {
                        slot: label_slots[0],
                        content: label.content,
                        font_family: label.font_family,
                    });
                }
                Some(LocalReactiveSlotPlan {
                    writes,
                    hit_write: None,
                })
            } else {
                None
            }
        }
        ReactiveScenePropertyValue::SliderValue {
            widget_id,
            value,
            track_rect,
            active_rect,
            thumb_rect,
            track_color,
            active_track_color,
            thumb_color,
            thumb_border,
            label,
        } => {
            let track_slots = computed.scene.matching_shape_slots(|shape| {
                shape.stroke_width == 0.0 && shape.rect == track_rect && shape.color == track_color
            });
            if track_slots.len() != 1 {
                return None;
            }
            let track_slot = track_slots[0];

            let active_slots = computed.scene.matching_shape_slots(|shape| {
                shape.stroke_width == 0.0
                    && shape.color == active_track_color
                    && slider_active_rect_matches(shape.rect, track_rect)
            });
            let active_slots = active_slots
                .into_iter()
                .filter(|slot| *slot != track_slot)
                .collect::<Vec<_>>();
            if active_slots.len() != 1 {
                return None;
            }

            let thumb_slots = computed.scene.matching_shape_slots(|shape| {
                shape.stroke_width == 0.0
                    && shape.color == thumb_color
                    && slider_thumb_rect_matches(shape.rect, thumb_rect, track_rect)
            });
            let thumb_slots = thumb_slots
                .into_iter()
                .filter(|slot| *slot != track_slot && *slot != active_slots[0])
                .collect::<Vec<_>>();
            if thumb_slots.len() != 1 {
                return None;
            }

            let mut writes = vec![
                ReactiveSlotWrite::ShapeRect {
                    slot: active_slots[0],
                    rect: active_rect,
                },
                ReactiveSlotWrite::ShapeRect {
                    slot: thumb_slots[0],
                    rect: thumb_rect,
                },
            ];

            if let Some((border_color, border_width)) = thumb_border {
                let border_slots = computed.scene.matching_shape_slots(|shape| {
                    shape.color == border_color
                        && (shape.stroke_width - border_width).abs() <= f32::EPSILON
                        && slider_thumb_rect_matches(shape.rect, thumb_rect, track_rect)
                });
                let border_slots = border_slots
                    .into_iter()
                    .filter(|slot| {
                        *slot != track_slot && *slot != active_slots[0] && *slot != thumb_slots[0]
                    })
                    .collect::<Vec<_>>();
                if border_slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::ShapeRect {
                    slot: border_slots[0],
                    rect: thumb_rect,
                });
            }
            if let Some(label) = label {
                let label_slots = computed.scene.matching_text_slots(|text| {
                    text.rich_spans.is_none() && !text.force_color && text.frame == label.frame
                });
                if label_slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::TextContent {
                    slot: label_slots[0],
                    content: label.content,
                    font_family: label.font_family,
                });
            }

            let hit_slots = computed
                .hit_regions
                .iter()
                .enumerate()
                .filter_map(|(index, hit)| match &hit.interaction {
                    HitInteraction::Slider { id, .. } if *id == widget_id => Some(index),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if hit_slots.len() != 1 {
                return None;
            }

            Some(LocalReactiveSlotPlan {
                writes,
                hit_write: Some(ReactiveHitWrite::Slider {
                    hit_index: hit_slots[0],
                    widget_id,
                    value,
                    track_rect,
                    thumb_rect,
                }),
            })
        }
    }
}

fn slider_active_rect_matches(rect: Rect, track_rect: Rect) -> bool {
    if rect.width < Dp::ZERO || rect.height < Dp::ZERO {
        return false;
    }
    let horizontal = track_rect.width >= track_rect.height;
    if horizontal {
        rect.x == track_rect.x
            && rect.y == track_rect.y
            && rect.height == track_rect.height
            && rect.width <= track_rect.width
    } else {
        rect.x == track_rect.x
            && rect.width == track_rect.width
            && rect.bottom() == track_rect.bottom()
            && rect.height <= track_rect.height
    }
}

fn rect_within(inner: Rect, outer: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
        && inner.width > Dp::ZERO
        && inner.height > Dp::ZERO
}

fn same_rect_size(a: Rect, b: Rect) -> bool {
    a.width == b.width && a.height == b.height
}

fn slider_thumb_rect_matches(rect: Rect, target_thumb_rect: Rect, track_rect: Rect) -> bool {
    if rect.width != target_thumb_rect.width || rect.height != target_thumb_rect.height {
        return false;
    }
    let horizontal = track_rect.width >= track_rect.height;
    if horizontal {
        rect.y == target_thumb_rect.y
            && rect.x >= track_rect.x - (target_thumb_rect.width * 0.5)
            && rect.x <= track_rect.right() - (target_thumb_rect.width * 0.5)
    } else {
        rect.x == target_thumb_rect.x
            && rect.y >= track_rect.y - (target_thumb_rect.height * 0.5)
            && rect.y <= track_rect.bottom() - (target_thumb_rect.height * 0.5)
    }
}
