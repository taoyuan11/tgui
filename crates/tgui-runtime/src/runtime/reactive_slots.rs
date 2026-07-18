use super::state::ReactiveMediaTextureBindingUpdate;
use super::*;
use crate::ui::unit::Dp;
use crate::ui::widget::{
    BackdropBlurPrimitive, BackdropBlurPrimitiveSlot, BrushPrimitive, BrushPrimitiveSlot,
    HitGeometry, HitInteraction, OverlayShapePrimitiveSlot, OverlayTextPrimitiveSlot,
    OverlayTexturePrimitiveSlot, SceneChunkParts, TextPrimitive, TexturePrimitive,
    TexturePrimitiveSlot,
};
use smallvec::SmallVec;
use std::sync::Arc;

fn reactive_property_targets_are_canonical(targets: &[(WidgetId, PropertySlot)]) -> bool {
    targets
        .windows(2)
        .all(|pair| (pair[0].0.raw(), pair[0].1 as u8) < (pair[1].0.raw(), pair[1].1 as u8))
}

#[derive(Clone)]
pub(crate) struct ReactiveSlotBinding {
    widget_id: WidgetId,
    local: LocalReactiveSlotBinding,
    root_offset: SceneCounts,
    root_hit_offset: usize,
    ancestor_offsets: Vec<(WidgetId, SceneCounts, usize)>,
    has_chunk_part: bool,
}

#[derive(Clone)]
pub(crate) struct ReactiveSlotPatch {
    writes: SmallVec<[ReactiveSlotWrite; 4]>,
    hit_write: Option<ReactiveHitWrite>,
}

impl ReactiveSlotBinding {
    #[cfg(feature = "bench-support")]
    pub(crate) fn patch_for(&self, value: ReactiveScenePropertyValue) -> Option<ReactiveSlotPatch> {
        self.local.patch_for(value)
    }
}

#[derive(Clone)]
struct LocalReactiveSlotBinding {
    kind: ReactiveSlotBindingKind,
}

#[derive(Clone)]
enum ReactiveSlotBindingKind {
    ShapeFillColor {
        slot: ShapePrimitiveSlot,
        container_occluder: Option<bool>,
    },
    OverlayShapeFillColor {
        slot: OverlayShapePrimitiveSlot,
        container_occluder: Option<bool>,
    },
    ShapeStrokeColor {
        slot: ShapePrimitiveSlot,
    },
    OverlayShapeStrokeColor {
        slot: OverlayShapePrimitiveSlot,
    },
    BackdropBlur {
        slot: BackdropBlurPrimitiveSlot,
        container_occluder: Option<bool>,
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
        shadow: Option<TexturePrimitiveSlot>,
        background: Option<ShapePrimitiveSlot>,
        border: Option<ShapePrimitiveSlot>,
        text: Option<TextPrimitiveSlot>,
        container_occluder: Option<bool>,
    },
    Offset {
        backdrop_blur: Option<BackdropBlurPrimitiveSlot>,
        background: Option<ShapePrimitiveSlot>,
        border: Option<ShapePrimitiveSlot>,
        texture: Option<TexturePrimitiveSlot>,
        brush: Option<BrushPrimitiveSlot>,
        container_occluder: Option<(usize, WidgetId, Option<Rect>)>,
    },
    Scale {
        backdrop_blur: Option<BackdropBlurPrimitiveSlot>,
        background: Option<ShapePrimitiveSlot>,
        border: Option<ShapePrimitiveSlot>,
        texture: Option<TexturePrimitiveSlot>,
        brush: Option<BrushPrimitiveSlot>,
        container_occluder: Option<(usize, WidgetId, Option<Rect>)>,
    },
    TextColor {
        slot: TextPrimitiveSlot,
    },
    OverlayTextColor {
        slot: OverlayTextPrimitiveSlot,
    },
    TextContent {
        slot: TextPrimitiveSlot,
    },
    OverlayTextContent {
        slot: OverlayTextPrimitiveSlot,
    },
    TextInputContent {
        slot: TextPrimitiveSlot,
    },
    TextureOpacity {
        slot: TexturePrimitiveSlot,
    },
    TextureMaskTint {
        slot: TexturePrimitiveSlot,
    },
    OverlayTextureOpacity {
        slot: OverlayTexturePrimitiveSlot,
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
    OverlayShapeColor {
        slot: OverlayShapePrimitiveSlot,
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
    OverlayTextColor {
        slot: OverlayTextPrimitiveSlot,
        color: Color,
    },
    TextContent {
        slot: TextPrimitiveSlot,
        content: Arc<str>,
        font_family: Option<Arc<str>>,
    },
    OverlayTextContent {
        slot: OverlayTextPrimitiveSlot,
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
    TextureMaskTint {
        slot: TexturePrimitiveSlot,
        color: Color,
    },
    OverlayTextureOpacity {
        slot: OverlayTexturePrimitiveSlot,
        opacity: f32,
    },
    Texture {
        slot: TexturePrimitiveSlot,
        primitive: TexturePrimitive,
    },
}

#[derive(Clone, Copy)]
enum ReactiveHitWrite {
    ContainerOccluder {
        hit_index: usize,
        widget_id: WidgetId,
        rect: Rect,
        clip_rect: Option<Rect>,
    },
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
            Self::OverlayShapeColor { slot, color } => computed
                .scene
                .write_overlay_shape_color_slot(offset, *slot, *color),
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
            Self::OverlayTextColor { slot, color } => computed
                .scene
                .write_overlay_text_color_slot(offset, *slot, *color),
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
            Self::OverlayTextContent {
                slot,
                content,
                font_family,
            } => computed.scene.write_overlay_text_content_slot(
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
            Self::TextureMaskTint { slot, color } => {
                computed.write_texture_mask_tint_slot(offset, *slot, *color)
            }
            Self::OverlayTextureOpacity { slot, opacity } => computed
                .scene
                .write_overlay_texture_opacity_slot(offset, *slot, *opacity),
            Self::Texture { slot, primitive } => {
                computed.write_texture_slot(offset, *slot, primitive.clone())
            }
        }
    }
}

impl ReactiveHitWrite {
    fn can_write<VM>(self, computed: &ComputedScene<VM>, hit_offset: usize) -> bool {
        match self {
            Self::ContainerOccluder {
                hit_index,
                widget_id,
                clip_rect,
                ..
            } => {
                let Some(index) = hit_offset.checked_add(hit_index) else {
                    return false;
                };
                matches!(
                    computed.hit_regions.get(index),
                    Some(hit)
                        if matches!(hit.geometry, HitGeometry::Rect)
                            && hit.clip_rect == clip_rect
                            && matches!(hit.interaction, HitInteraction::Occluder { id } if id == widget_id)
                )
            }
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
            Self::ContainerOccluder {
                hit_index,
                widget_id,
                rect,
                clip_rect,
            } => {
                if !self.can_write(computed, hit_offset) {
                    return false;
                }
                let hit = &mut computed.hit_regions[hit_offset + hit_index];
                if !matches!(hit.interaction, HitInteraction::Occluder { id } if id == widget_id) {
                    return false;
                }
                hit.rect = rect;
                hit.clip_rect = clip_rect;
                true
            }
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
            ReactiveSlotBindingKind::ShapeFillColor { slot, .. }
            | ReactiveSlotBindingKind::ShapeStrokeColor { slot } => {
                computed.can_write_shape_color_slot(offset, *slot)
            }
            ReactiveSlotBindingKind::OverlayShapeFillColor { slot, .. }
            | ReactiveSlotBindingKind::OverlayShapeStrokeColor { slot } => {
                computed.scene.can_write_overlay_shape_slot(offset, *slot)
            }
            ReactiveSlotBindingKind::BackdropBlur { slot, .. } => {
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
                shadow,
                background,
                border,
                text,
                ..
            } => {
                shadow
                    .map(|slot| computed.can_write_texture_opacity_slot(offset, slot))
                    .unwrap_or(true)
                    && can_write_shape_option(computed, offset, *background)
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
                ..
            }
            | ReactiveSlotBindingKind::Scale {
                backdrop_blur,
                background,
                border,
                texture,
                brush,
                ..
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
            ReactiveSlotBindingKind::OverlayTextColor { slot }
            | ReactiveSlotBindingKind::OverlayTextContent { slot } => computed
                .scene
                .can_write_overlay_text_color_slot(offset, *slot),
            ReactiveSlotBindingKind::TextureOpacity { slot }
            | ReactiveSlotBindingKind::TextureMaskTint { slot }
            | ReactiveSlotBindingKind::Texture { slot } => {
                computed.can_write_texture_opacity_slot(offset, *slot)
            }
            ReactiveSlotBindingKind::OverlayTextureOpacity { slot } => computed
                .scene
                .can_write_overlay_texture_opacity_slot(offset, *slot),
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
            ReactiveSlotBindingKind::Offset {
                container_occluder: Some((hit_index, widget_id, clip_rect)),
                ..
            }
            | ReactiveSlotBindingKind::Scale {
                container_occluder: Some((hit_index, widget_id, clip_rect)),
                ..
            } => ReactiveHitWrite::ContainerOccluder {
                hit_index: *hit_index,
                widget_id: *widget_id,
                rect: Rect::new(Dp::ZERO, Dp::ZERO, Dp::ZERO, Dp::ZERO),
                clip_rect: *clip_rect,
            }
            .can_write(computed, hit_offset),
            _ => true,
        }
    }

    fn patch_for(&self, value: ReactiveScenePropertyValue) -> Option<ReactiveSlotPatch> {
        let mut writes = SmallVec::new();
        let mut hit_write = None;
        match (&self.kind, value) {
            (
                ReactiveSlotBindingKind::ShapeFillColor {
                    slot,
                    container_occluder,
                },
                ReactiveScenePropertyValue::ShapeFillColor {
                    color,
                    container_occluder: value_container_occluder,
                    ..
                },
            ) if *container_occluder == value_container_occluder => {
                writes.push(ReactiveSlotWrite::ShapeColor { slot: *slot, color });
            }
            (
                ReactiveSlotBindingKind::ShapeStrokeColor { slot },
                ReactiveScenePropertyValue::ShapeStrokeColor { color, .. },
            ) => {
                writes.push(ReactiveSlotWrite::ShapeColor { slot: *slot, color });
            }
            (
                ReactiveSlotBindingKind::OverlayShapeFillColor {
                    slot,
                    container_occluder,
                },
                ReactiveScenePropertyValue::ShapeFillColor {
                    color,
                    container_occluder: value_container_occluder,
                    ..
                },
            ) if *container_occluder == value_container_occluder => {
                writes.push(ReactiveSlotWrite::OverlayShapeColor { slot: *slot, color });
            }
            (
                ReactiveSlotBindingKind::OverlayShapeStrokeColor { slot },
                ReactiveScenePropertyValue::ShapeStrokeColor { color, .. },
            ) => {
                writes.push(ReactiveSlotWrite::OverlayShapeColor { slot: *slot, color });
            }
            (
                ReactiveSlotBindingKind::BackdropBlur {
                    slot,
                    container_occluder,
                },
                ReactiveScenePropertyValue::BackdropBlur {
                    primitive,
                    container_occluder: value_container_occluder,
                },
            ) if *container_occluder == value_container_occluder => {
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
                    shadow,
                    background,
                    border,
                    text,
                    container_occluder,
                },
                ReactiveScenePropertyValue::Opacity {
                    shadow: value_shadow,
                    background: value_background,
                    border: value_border,
                    text: value_text,
                    container_occluder: value_container_occluder,
                },
            ) if *container_occluder == value_container_occluder => {
                if shadow.is_some() != value_shadow.is_some()
                    || background.is_some() != value_background.is_some()
                    || border.is_some() != value_border.is_some()
                    || text.is_some() != value_text.is_some()
                {
                    return None;
                }
                if let (Some(slot), Some((_, _, opacity))) = (*shadow, value_shadow) {
                    writes.push(ReactiveSlotWrite::TextureOpacity {
                        slot,
                        opacity: opacity.clamp(0.0, 1.0),
                    });
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
                    container_occluder,
                },
                ReactiveScenePropertyValue::Offset {
                    background: value_background,
                    border: value_border,
                    backdrop_blur: value_backdrop_blur,
                    brush: value_brush,
                    texture: value_texture,
                    container_occluder: value_container_occluder,
                },
            ) => {
                if backdrop_blur.is_some() != value_backdrop_blur.is_some()
                    || background.is_some() != value_background.is_some()
                    || border.is_some() != value_border.is_some()
                    || texture.is_some() != value_texture.is_some()
                    || brush.is_some() != value_brush.is_some()
                    || container_occluder.is_some() != value_container_occluder.is_some()
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
                    Some((
                        texture,
                        media_key,
                        media_layout,
                        frame,
                        corner_radius,
                        opacity,
                        clip_rect,
                        clip_mask,
                    )),
                ) = (*texture, value_texture)
                {
                    writes.push(ReactiveSlotWrite::Texture {
                        slot,
                        primitive: TexturePrimitive {
                            texture,
                            media_key,
                            media_layout,
                            mask_tint: None,
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
                if let (
                    Some((hit_index, widget_id, binding_clip_rect)),
                    Some((value_widget_id, rect, clip_rect)),
                ) = (*container_occluder, value_container_occluder)
                {
                    if widget_id != value_widget_id || binding_clip_rect != clip_rect {
                        return None;
                    }
                    hit_write = Some(ReactiveHitWrite::ContainerOccluder {
                        hit_index,
                        widget_id,
                        rect,
                        clip_rect,
                    });
                }
            }
            (
                ReactiveSlotBindingKind::Scale {
                    backdrop_blur,
                    background,
                    border,
                    texture,
                    brush,
                    container_occluder,
                },
                ReactiveScenePropertyValue::Scale {
                    background: value_background,
                    border: value_border,
                    backdrop_blur: value_backdrop_blur,
                    brush: value_brush,
                    texture: value_texture,
                    container_occluder: value_container_occluder,
                },
            ) => {
                if backdrop_blur.is_some() != value_backdrop_blur.is_some()
                    || background.is_some() != value_background.is_some()
                    || border.is_some() != value_border.is_some()
                    || texture.is_some() != value_texture.is_some()
                    || brush.is_some() != value_brush.is_some()
                    || container_occluder.is_some() != value_container_occluder.is_some()
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
                    Some((
                        texture,
                        media_key,
                        media_layout,
                        frame,
                        corner_radius,
                        opacity,
                        clip_rect,
                        clip_mask,
                    )),
                ) = (*texture, value_texture)
                {
                    writes.push(ReactiveSlotWrite::Texture {
                        slot,
                        primitive: TexturePrimitive {
                            texture,
                            media_key,
                            media_layout,
                            mask_tint: None,
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
                if let (
                    Some((hit_index, widget_id, binding_clip_rect)),
                    Some((value_widget_id, rect, clip_rect)),
                ) = (*container_occluder, value_container_occluder)
                {
                    if widget_id != value_widget_id || binding_clip_rect != clip_rect {
                        return None;
                    }
                    hit_write = Some(ReactiveHitWrite::ContainerOccluder {
                        hit_index,
                        widget_id,
                        rect,
                        clip_rect,
                    });
                }
            }
            (
                ReactiveSlotBindingKind::TextColor { slot },
                ReactiveScenePropertyValue::TextColor { color },
            ) => {
                writes.push(ReactiveSlotWrite::TextColor { slot: *slot, color });
            }
            (
                ReactiveSlotBindingKind::OverlayTextColor { slot },
                ReactiveScenePropertyValue::TextColor { color },
            ) => {
                writes.push(ReactiveSlotWrite::OverlayTextColor { slot: *slot, color });
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
                ReactiveSlotBindingKind::OverlayTextContent { slot },
                ReactiveScenePropertyValue::TextContent {
                    content,
                    font_family,
                },
            ) => {
                writes.push(ReactiveSlotWrite::OverlayTextContent {
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
                ReactiveSlotBindingKind::TextureMaskTint { slot },
                ReactiveScenePropertyValue::TextureMaskTint { color },
            ) => {
                writes.push(ReactiveSlotWrite::TextureMaskTint { slot: *slot, color });
            }
            (
                ReactiveSlotBindingKind::OverlayTextureOpacity { slot },
                ReactiveScenePropertyValue::TextureOpacity { opacity, .. },
            ) => {
                writes.push(ReactiveSlotWrite::OverlayTextureOpacity {
                    slot: *slot,
                    opacity: opacity.clamp(0.0, 1.0),
                });
            }
            (
                ReactiveSlotBindingKind::Texture { slot },
                ReactiveScenePropertyValue::Texture {
                    texture,
                    media_key,
                    media_layout,
                    mask_tint,
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
                        media_layout,
                        mask_tint,
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

pub(crate) fn build_reactive_slot_binding_for_scene<VM: 'static>(
    widget_id: WidgetId,
    value: ReactiveScenePropertyValue,
    layout: &ResolvedSceneLayout<VM>,
    computed: &ComputedScene<VM>,
    scene_chunks: &HashMap<WidgetId, ComputedScene<VM>>,
    scene_chunk_parts: &HashMap<WidgetId, SceneChunkParts<VM>>,
) -> Option<ReactiveSlotBinding> {
    let target_chunk = scene_chunks.get(&widget_id)?;
    let (local_scene, has_chunk_part) = if let Some(parts) = scene_chunk_parts.get(&widget_id) {
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
        let offsets =
            layout.scene_splice_ancestor_offsets(widget_id, scene_chunk_parts, scene_chunks)?;
        let (root_id, root_offset, root_hit_offset, _) = offsets.first().copied()?;
        if root_id != layout.root_id() {
            return None;
        }
        let mut ancestor_offsets = Vec::with_capacity(offsets.len());
        for (ancestor_id, offset, hit_offset, _) in offsets {
            let ancestor_chunk = scene_chunks.get(&ancestor_id)?;
            if !local.can_write(ancestor_chunk, &offset)
                || !local.can_write_hit(ancestor_chunk, hit_offset)
            {
                return None;
            }
            ancestor_offsets.push((ancestor_id, offset, hit_offset));
        }
        (root_offset, root_hit_offset, ancestor_offsets)
    };
    if !local.can_write(computed, &root_offset) || !local.can_write_hit(computed, root_hit_offset) {
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

pub(crate) fn write_reactive_slot_patch_to_scene<VM: 'static>(
    computed: &mut ComputedScene<VM>,
    scene_chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
    scene_chunk_parts: &mut HashMap<WidgetId, SceneChunkParts<VM>>,
    binding: &ReactiveSlotBinding,
    patch: &ReactiveSlotPatch,
) -> bool {
    let zero = SceneCounts::default();
    if binding.has_chunk_part {
        let Some(parts) = scene_chunk_parts.get_mut(&binding.widget_id) else {
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
    let Some(target_chunk) = scene_chunks.get_mut(&binding.widget_id) else {
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
        let Some(ancestor_chunk) = scene_chunks.get_mut(ancestor_id) else {
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
        if !write.write(computed, &binding.root_offset) {
            return false;
        }
    }
    if let Some(hit_write) = patch.hit_write {
        if !hit_write.write(computed, binding.root_hit_offset) {
            return false;
        }
    }
    true
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

        let mut unique_targets = targets
            .iter()
            .map(|(widget_id, _)| *widget_id)
            .collect::<Vec<_>>();
        unique_targets.sort_unstable_by_key(|widget_id| widget_id.raw());
        unique_targets.dedup();

        self.try_update_canonical_transform_widget_ids(&unique_targets, now)
    }

    pub(super) fn try_update_canonical_reactive_transform_records(
        &mut self,
        targets: &[(WidgetId, PropertySlot)],
        now: Instant,
    ) -> bool {
        debug_assert!(reactive_property_targets_are_canonical(targets));
        if targets.is_empty()
            || targets
                .iter()
                .any(|(_, property)| *property != PropertySlot::Offset)
        {
            return false;
        }
        let widget_ids = targets
            .iter()
            .map(|(widget_id, _)| *widget_id)
            .collect::<Vec<_>>();
        self.try_update_canonical_transform_widget_ids(&widget_ids, now)
    }

    fn try_update_canonical_transform_widget_ids(
        &mut self,
        unique_targets: &[WidgetId],
        now: Instant,
    ) -> bool {
        debug_assert!(unique_targets
            .windows(2)
            .all(|pair| pair[0].raw() < pair[1].raw()));

        let theme = self.animated_theme(now);
        let viewport = self.viewport_rect();
        let active_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        let widget_states = self.widget_state_map(active_scrollbar);
        let resolved_offsets = {
            let Some(cached) = self.cached_scene.as_ref() else {
                return false;
            };
            let Some(layout) = cached.layout.as_ref() else {
                return false;
            };
            layout.resolve_reactive_transform_offsets(
                &unique_targets,
                &cached.visual_contexts,
                &self.font_manager,
                &theme,
                &self.media_manager,
                &mut self.animation_engine,
                self.reduced_motion,
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
        };
        let mut updates = Vec::with_capacity(unique_targets.len());
        let mut changed = false;
        for (widget_id, offset) in unique_targets.iter().copied().zip(resolved_offsets) {
            let Some(cached) = self.cached_scene.as_ref() else {
                return false;
            };
            let Some(record) = cached.computed.transform_records.get(&widget_id) else {
                return false;
            };
            let Some(offset) = offset else {
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

        let mut unique_targets = targets.to_vec();
        unique_targets
            .sort_unstable_by_key(|(widget_id, property)| (widget_id.raw(), *property as u8));
        unique_targets.dedup();

        self.try_patch_canonical_reactive_property_slots(&unique_targets, now)
    }

    pub(super) fn try_patch_canonical_reactive_property_slots(
        &mut self,
        targets: &[(WidgetId, PropertySlot)],
        now: Instant,
    ) -> bool {
        debug_assert!(reactive_property_targets_are_canonical(targets));
        if targets.is_empty() {
            return false;
        }

        let Some(values) = self.resolve_reactive_slot_values(targets, now) else {
            return false;
        };

        let mut plans = Vec::with_capacity(targets.len());
        for ((widget_id, property), value) in targets.iter().copied().zip(values) {
            let Some(binding) = self.cached_scene.as_ref().and_then(|cached| {
                cached
                    .reactive_slot_bindings
                    .get(&(widget_id, property))
                    .cloned()
            }) else {
                return false;
            };
            let Some(value) = value else {
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

        let Some(values) = self.resolve_reactive_slot_values(&targets, now) else {
            return HashMap::new();
        };
        let mut bindings = HashMap::with_capacity(targets.len());
        for ((widget_id, property), value) in targets.into_iter().zip(values) {
            let Some(value) = value else {
                continue;
            };
            let Some(cached) = self.cached_scene.as_ref() else {
                break;
            };
            let Some(layout) = cached.layout.as_ref() else {
                break;
            };
            if let Some(binding) = build_reactive_slot_binding_for_scene(
                widget_id,
                value,
                layout,
                &cached.computed,
                &cached.scene_chunks,
                &cached.scene_chunk_parts,
            ) {
                bindings.insert((widget_id, property), binding);
            }
        }
        bindings
    }

    pub(super) fn resolve_reactive_slot_values(
        &mut self,
        targets: &[(WidgetId, PropertySlot)],
        now: Instant,
    ) -> Option<Vec<Option<ReactiveScenePropertyValue>>> {
        let theme = self.animated_theme(now);
        let viewport = self.viewport_rect();
        let active_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        let widget_states = self.widget_state_map(active_scrollbar);
        let cached = self.cached_scene.as_ref()?;
        let layout = cached.layout.as_ref()?;
        Some(layout.resolve_reactive_scene_property_values(
            targets,
            &cached.visual_contexts,
            &self.font_manager,
            &theme,
            &self.media_manager,
            &mut self.animation_engine,
            self.reduced_motion,
            self.hovered_scrollbar,
            active_scrollbar,
            &widget_states,
            &self.select_open_states,
            &self.scroll_states,
            &self.virtual_states,
            viewport,
            now,
            &self.config.style_sheet,
        ))
    }

    #[cfg(all(test, feature = "bench-support"))]
    pub(super) fn resolve_reactive_slot_values_individually(
        &mut self,
        targets: &[(WidgetId, PropertySlot)],
        now: Instant,
    ) -> Option<Vec<Option<ReactiveScenePropertyValue>>> {
        let mut values = Vec::with_capacity(targets.len());
        for target in targets {
            let mut resolved =
                self.resolve_reactive_slot_values(std::slice::from_ref(target), now)?;
            values.push(resolved.pop().unwrap_or(None));
        }
        Some(values)
    }
}

fn write_reactive_slot_patch<VM: 'static>(
    cached: &mut CachedScene<VM>,
    binding: &ReactiveSlotBinding,
    patch: &ReactiveSlotPatch,
) -> bool {
    if !write_reactive_slot_patch_to_scene(
        &mut cached.computed,
        &mut cached.scene_chunks,
        &mut cached.scene_chunk_parts,
        binding,
        patch,
    ) {
        return false;
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
        media_layout: primitive.media_layout,
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
        ReactiveScenePropertyValue::ShapeFillColor {
            container_occluder, ..
        } => {
            let [write] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            match write {
                ReactiveSlotWrite::ShapeColor { slot, .. } => {
                    Some(ReactiveSlotBindingKind::ShapeFillColor {
                        slot: *slot,
                        container_occluder,
                    })
                }
                ReactiveSlotWrite::OverlayShapeColor { slot, .. } => {
                    Some(ReactiveSlotBindingKind::OverlayShapeFillColor {
                        slot: *slot,
                        container_occluder,
                    })
                }
                _ => None,
            }
        }
        ReactiveScenePropertyValue::ShapeStrokeColor { .. } => {
            let [write] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            match write {
                ReactiveSlotWrite::ShapeColor { slot, .. } => {
                    Some(ReactiveSlotBindingKind::ShapeStrokeColor { slot: *slot })
                }
                ReactiveSlotWrite::OverlayShapeColor { slot, .. } => {
                    Some(ReactiveSlotBindingKind::OverlayShapeStrokeColor { slot: *slot })
                }
                _ => None,
            }
        }
        ReactiveScenePropertyValue::BackdropBlur {
            container_occluder, ..
        } => {
            let [ReactiveSlotWrite::BackdropBlur { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::BackdropBlur {
                slot: *slot,
                container_occluder,
            })
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
            shadow,
            background,
            border,
            text,
            container_occluder,
        } => {
            no_hit(plan)?;
            let mut writes = plan.writes.iter();
            let shadow = if shadow.is_some() {
                Some(next_texture_opacity_slot(&mut writes)?)
            } else {
                None
            };
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
                shadow,
                background,
                border,
                text,
                container_occluder,
            })
        }
        ReactiveScenePropertyValue::Offset {
            background,
            border,
            backdrop_blur,
            brush,
            texture,
            container_occluder,
        } => {
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
            let container_occluder = match container_occluder {
                Some((widget_id, _, clip_rect)) => {
                    let Some(ReactiveHitWrite::ContainerOccluder {
                        hit_index,
                        widget_id: hit_widget_id,
                        clip_rect: hit_clip_rect,
                        ..
                    }) = plan.hit_write
                    else {
                        return None;
                    };
                    Some(
                        (hit_widget_id == widget_id && hit_clip_rect == clip_rect)
                            .then_some((hit_index, widget_id, clip_rect))?,
                    )
                }
                None => {
                    no_hit(plan)?;
                    None
                }
            };
            Some(ReactiveSlotBindingKind::Offset {
                backdrop_blur,
                background,
                border,
                texture,
                brush,
                container_occluder,
            })
        }
        ReactiveScenePropertyValue::Scale {
            background,
            border,
            backdrop_blur,
            brush,
            texture,
            container_occluder,
        } => {
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
            let container_occluder = match container_occluder {
                Some((widget_id, _, clip_rect)) => {
                    let Some(ReactiveHitWrite::ContainerOccluder {
                        hit_index,
                        widget_id: hit_widget_id,
                        clip_rect: hit_clip_rect,
                        ..
                    }) = plan.hit_write
                    else {
                        return None;
                    };
                    Some(
                        (hit_widget_id == widget_id && hit_clip_rect == clip_rect)
                            .then_some((hit_index, widget_id, clip_rect))?,
                    )
                }
                None => {
                    no_hit(plan)?;
                    None
                }
            };
            Some(ReactiveSlotBindingKind::Scale {
                backdrop_blur,
                background,
                border,
                texture,
                brush,
                container_occluder,
            })
        }
        ReactiveScenePropertyValue::TextColor { .. } => {
            let [write] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            match write {
                ReactiveSlotWrite::TextColor { slot, .. } => {
                    Some(ReactiveSlotBindingKind::TextColor { slot: *slot })
                }
                ReactiveSlotWrite::OverlayTextColor { slot, .. } => {
                    Some(ReactiveSlotBindingKind::OverlayTextColor { slot: *slot })
                }
                _ => None,
            }
        }
        ReactiveScenePropertyValue::TextContent { .. } => {
            let [write] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            match write {
                ReactiveSlotWrite::TextContent { slot, .. } => {
                    Some(ReactiveSlotBindingKind::TextContent { slot: *slot })
                }
                ReactiveSlotWrite::OverlayTextContent { slot, .. } => {
                    Some(ReactiveSlotBindingKind::OverlayTextContent { slot: *slot })
                }
                _ => None,
            }
        }
        ReactiveScenePropertyValue::TextInputContent(_) => {
            let [ReactiveSlotWrite::TextPrimitive { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::TextInputContent { slot: *slot })
        }
        ReactiveScenePropertyValue::TextureOpacity { .. } => {
            let [write] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            match write {
                ReactiveSlotWrite::TextureOpacity { slot, .. } => {
                    Some(ReactiveSlotBindingKind::TextureOpacity { slot: *slot })
                }
                ReactiveSlotWrite::OverlayTextureOpacity { slot, .. } => {
                    Some(ReactiveSlotBindingKind::OverlayTextureOpacity { slot: *slot })
                }
                _ => None,
            }
        }
        ReactiveScenePropertyValue::TextureMaskTint { .. } => {
            let [ReactiveSlotWrite::TextureMaskTint { slot, .. }] = plan.writes.as_slice() else {
                return None;
            };
            no_hit(plan)?;
            Some(ReactiveSlotBindingKind::TextureMaskTint { slot: *slot })
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

fn next_texture_opacity_slot<'a>(
    writes: &mut impl Iterator<Item = &'a ReactiveSlotWrite>,
) -> Option<TexturePrimitiveSlot> {
    match writes.next()? {
        ReactiveSlotWrite::TextureOpacity { slot, .. } => Some(*slot),
        _ => None,
    }
}

fn slot_write_for_reactive_value<VM>(
    computed: &ComputedScene<VM>,
    value: ReactiveScenePropertyValue,
) -> Option<LocalReactiveSlotPlan> {
    match value {
        ReactiveScenePropertyValue::ShapeFillColor { rect, color, .. } => {
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
                let slots = computed.scene.matching_overlay_shape_slots(|shape| {
                    shape.stroke_width == 0.0 && shape.rect == rect
                });
                (slots.len() == 1).then(|| LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::OverlayShapeColor {
                        slot: slots[0],
                        color,
                    }],
                    hit_write: None,
                })
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
                let slots = computed.scene.matching_overlay_shape_slots(|shape| {
                    shape.rect == rect && (shape.stroke_width - stroke_width).abs() <= f32::EPSILON
                });
                (slots.len() == 1).then(|| LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::OverlayShapeColor {
                        slot: slots[0],
                        color,
                    }],
                    hit_write: None,
                })
            }
        }
        ReactiveScenePropertyValue::BackdropBlur { primitive, .. } => {
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
            shadow,
            background,
            border,
            text,
            ..
        } => {
            let mut writes = Vec::new();
            let mut used_shape_slots = Vec::new();
            if let Some((texture_id, frame, opacity)) = shadow {
                let slots = computed.scene.matching_texture_slots(|texture| {
                    texture.texture.id() == texture_id
                        && texture.frame == frame
                        && texture.media_key.is_none()
                        && texture.media_layout.is_none()
                        && texture.mask_tint.is_none()
                        && texture.quad.is_none()
                        && texture.uv_rect.is_none()
                });
                if slots.len() != 1 {
                    return None;
                }
                writes.push(ReactiveSlotWrite::TextureOpacity {
                    slot: slots[0],
                    opacity: opacity.clamp(0.0, 1.0),
                });
            }
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
            container_occluder,
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
            if let Some((
                texture,
                media_key,
                media_layout,
                frame,
                corner_radius,
                opacity,
                clip_rect,
                clip_mask,
            )) = texture
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
                        media_layout,
                        mask_tint: None,
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
                let hit_write = if let Some((widget_id, rect, clip_rect)) = container_occluder {
                    let matches = computed
                        .hit_regions
                        .iter()
                        .enumerate()
                        .filter(|(_, hit)| {
                            hit.rect == rect
                                && hit.clip_rect == clip_rect
                                && matches!(hit.geometry, HitGeometry::Rect)
                                && matches!(
                                    hit.interaction,
                                    HitInteraction::Occluder { id } if id == widget_id
                                )
                        })
                        .map(|(index, _)| index)
                        .collect::<SmallVec<[usize; 2]>>();
                    if matches.len() != 1 {
                        return None;
                    }
                    Some(ReactiveHitWrite::ContainerOccluder {
                        hit_index: matches[0],
                        widget_id,
                        rect,
                        clip_rect,
                    })
                } else {
                    None
                };
                Some(LocalReactiveSlotPlan { writes, hit_write })
            }
        }
        ReactiveScenePropertyValue::Scale {
            background,
            border,
            backdrop_blur,
            brush,
            texture,
            container_occluder,
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
            if let Some((
                texture,
                media_key,
                media_layout,
                frame,
                corner_radius,
                opacity,
                clip_rect,
                clip_mask,
            )) = texture
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
                        media_layout,
                        mask_tint: None,
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
                let hit_write = if let Some((widget_id, rect, clip_rect)) = container_occluder {
                    let matches = computed
                        .hit_regions
                        .iter()
                        .enumerate()
                        .filter(|(_, hit)| {
                            hit.rect == rect
                                && hit.clip_rect == clip_rect
                                && matches!(hit.geometry, HitGeometry::Rect)
                                && matches!(
                                    hit.interaction,
                                    HitInteraction::Occluder { id } if id == widget_id
                                )
                        })
                        .map(|(index, _)| index)
                        .collect::<SmallVec<[usize; 2]>>();
                    if matches.len() != 1 {
                        return None;
                    }
                    Some(ReactiveHitWrite::ContainerOccluder {
                        hit_index: matches[0],
                        widget_id,
                        rect,
                        clip_rect,
                    })
                } else {
                    None
                };
                Some(LocalReactiveSlotPlan { writes, hit_write })
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
                let slots = computed.scene.matching_overlay_text_slots(|text| {
                    text.rich_spans.is_none() && !text.force_color
                });
                (slots.len() == 1).then(|| LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::OverlayTextColor {
                        slot: slots[0],
                        color,
                    }],
                    hit_write: None,
                })
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
                let slots = computed.scene.matching_overlay_text_slots(|text| {
                    text.rich_spans.is_none() && !text.force_color
                });
                (slots.len() == 1).then(|| LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::OverlayTextContent {
                        slot: slots[0],
                        content,
                        font_family,
                    }],
                    hit_write: None,
                })
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
                let slots = computed.scene.matching_overlay_texture_slots(|texture| {
                    texture.frame == frame
                        && (texture.corner_radius - corner_radius).abs() <= f32::EPSILON
                });
                (slots.len() == 1).then(|| LocalReactiveSlotPlan {
                    writes: vec![ReactiveSlotWrite::OverlayTextureOpacity {
                        slot: slots[0],
                        opacity,
                    }],
                    hit_write: None,
                })
            }
        }
        ReactiveScenePropertyValue::TextureMaskTint { color } => {
            let slots = computed
                .scene
                .matching_texture_slots(|texture| texture.mask_tint.is_some());
            (slots.len() == 1).then(|| LocalReactiveSlotPlan {
                writes: vec![ReactiveSlotWrite::TextureMaskTint {
                    slot: slots[0],
                    color,
                }],
                hit_write: None,
            })
        }
        ReactiveScenePropertyValue::Texture {
            texture,
            media_key,
            media_layout,
            mask_tint,
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
                            media_layout,
                            mask_tint,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::TextureFrame;
    use crate::text::font::FontWeight;
    use crate::ui::widget::{
        CanvasTextHorizontalAlign, CanvasTextOverflow, CanvasTextVerticalAlign, CanvasTextWrap,
        RenderCommand, RenderPrimitive, SceneCounts,
    };

    fn shape(rect: Rect, color: Color) -> RenderPrimitive {
        RenderPrimitive {
            rect,
            color,
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect: None,
            clip_mask: None,
        }
    }

    fn text(content: &str, color: Color) -> TextPrimitive {
        TextPrimitive {
            content: Arc::from(content),
            rich_spans: None,
            frame: Rect::new(0.0, 0.0, 64.0, 20.0),
            quad: None,
            color,
            force_color: false,
            font_family: None,
            font_size: 14.0,
            font_weight: FontWeight::NORMAL,
            line_height: 18.0,
            letter_spacing: 0.0,
            wrap: CanvasTextWrap::Word,
            overflow: CanvasTextOverflow::Clip,
            horizontal_align: CanvasTextHorizontalAlign::Start,
            vertical_align: CanvasTextVerticalAlign::Start,
            clip_rect: None,
            clip_mask: None,
        }
    }

    fn texture(opacity: f32) -> TexturePrimitive {
        TexturePrimitive {
            texture: Arc::new(TextureFrame::new(1, 1, vec![255, 255, 255, 255])),
            media_key: None,
            media_layout: None,
            frame: Rect::new(0.0, 0.0, 12.0, 12.0),
            quad: None,
            uv_rect: None,
            corner_radius: 2.0,
            opacity,
            mask_tint: None,
            clip_rect: None,
            clip_mask: None,
        }
    }

    fn apply_patch<VM>(
        computed: &mut ComputedScene<VM>,
        binding: &LocalReactiveSlotBinding,
        value: ReactiveScenePropertyValue,
    ) {
        let patch = binding
            .patch_for(value)
            .expect("overlay reactive slot patch should be available");
        assert!(patch.hit_write.is_none());
        for write in patch.writes {
            assert!(write.write(computed, &SceneCounts::default()));
        }
    }

    #[test]
    fn overlay_shape_color_builds_reactive_slot_plan() {
        let mut computed = ComputedScene::<()>::default();
        let rect = Rect::new(1.0, 2.0, 30.0, 20.0);
        computed.scene.push_overlay_shape(shape(rect, Color::WHITE));

        let value = ReactiveScenePropertyValue::ShapeFillColor {
            rect,
            color: Color::RED,
            container_occluder: None,
        };
        let binding = slot_binding_for_reactive_value(&computed, value.clone())
            .expect("overlay shape should build a retained slot binding");
        assert!(matches!(
            binding.kind,
            ReactiveSlotBindingKind::OverlayShapeFillColor { .. }
        ));

        apply_patch(&mut computed, &binding, value);
        assert_eq!(computed.scene.overlay_shapes[0].color, Color::RED);
        match &computed.scene.overlay_commands[0] {
            RenderCommand::Shape(command) => assert_eq!(command.color, Color::RED),
            _ => panic!("expected overlay shape command"),
        }
    }

    #[test]
    fn container_background_slot_rejects_occluder_topology_changes() {
        let mut computed = ComputedScene::<()>::default();
        let rect = Rect::new(1.0, 2.0, 30.0, 20.0);
        computed.scene.push_shape(shape(rect, Color::TRANSPARENT));

        let initial = ReactiveScenePropertyValue::ShapeFillColor {
            rect,
            color: Color::TRANSPARENT,
            container_occluder: Some(false),
        };
        let binding = slot_binding_for_reactive_value(&computed, initial)
            .expect("transparent Container background should retain its shape slot");

        assert!(binding
            .patch_for(ReactiveScenePropertyValue::ShapeFillColor {
                rect,
                color: Color::rgba(255, 255, 255, 0),
                container_occluder: Some(false),
            })
            .is_some());
        assert!(binding
            .patch_for(ReactiveScenePropertyValue::ShapeFillColor {
                rect,
                color: Color::WHITE,
                container_occluder: Some(true),
            })
            .is_none());
    }

    #[test]
    fn overlay_text_content_builds_reactive_slot_plan() {
        let mut computed = ComputedScene::<()>::default();
        computed
            .scene
            .push_overlay_text(text("before", Color::WHITE));

        let value = ReactiveScenePropertyValue::TextContent {
            content: Arc::from("after"),
            font_family: Some(Arc::from("Inter")),
        };
        let binding = slot_binding_for_reactive_value(&computed, value.clone())
            .expect("overlay text should build a retained slot binding");
        assert!(matches!(
            binding.kind,
            ReactiveSlotBindingKind::OverlayTextContent { .. }
        ));

        apply_patch(&mut computed, &binding, value);
        assert_eq!(computed.scene.overlay_texts[0].content.as_ref(), "after");
        match &computed.scene.overlay_commands[0] {
            RenderCommand::Text(command) => {
                assert_eq!(command.content.as_ref(), "after");
                assert_eq!(command.font_family.as_deref(), Some("Inter"));
            }
            _ => panic!("expected overlay text command"),
        }
    }

    #[test]
    fn overlay_texture_opacity_builds_reactive_slot_plan() {
        let mut computed = ComputedScene::<()>::default();
        computed.scene.push_overlay_texture(texture(0.25));

        let value = ReactiveScenePropertyValue::TextureOpacity {
            frame: Rect::new(0.0, 0.0, 12.0, 12.0),
            corner_radius: 2.0,
            opacity: 0.75,
        };
        let binding = slot_binding_for_reactive_value(&computed, value.clone())
            .expect("overlay texture should build a retained slot binding");
        assert!(matches!(
            binding.kind,
            ReactiveSlotBindingKind::OverlayTextureOpacity { .. }
        ));

        apply_patch(&mut computed, &binding, value);
        assert_eq!(computed.scene.overlay_textures[0].opacity, 0.75);
        match &computed.scene.overlay_commands[0] {
            RenderCommand::Texture(command) => assert_eq!(command.opacity, 0.75),
            _ => panic!("expected overlay texture command"),
        }
    }

    #[test]
    fn texture_mask_tint_updates_only_the_retained_draw() {
        let mut computed = ComputedScene::<()>::default();
        let mut primitive = texture(1.0);
        primitive.mask_tint = Some(Color::WHITE);
        let texture_id = primitive.texture.id();
        let frame = primitive.frame;
        computed.scene.push_texture(primitive);
        let serial = computed.scene.prepare_cache_serial();

        let value = ReactiveScenePropertyValue::TextureMaskTint { color: Color::RED };
        let binding = slot_binding_for_reactive_value(&computed, value.clone())
            .expect("monochrome texture should build a retained tint binding");
        assert!(matches!(
            binding.kind,
            ReactiveSlotBindingKind::TextureMaskTint { .. }
        ));

        apply_patch(&mut computed, &binding, value);
        let texture = &computed.scene.textures[0];
        assert_eq!(texture.texture.id(), texture_id);
        assert_eq!(texture.frame, frame);
        assert_eq!(texture.mask_tint, Some(Color::RED));
        assert_eq!(computed.scene.prepare_cache_serial(), serial);
        assert!(!computed.scene.cache_liveness_dirty());
        assert_eq!(computed.scene.dirty_draw_ranges().len(), 1);
        match &computed.scene.commands[0] {
            RenderCommand::Texture(command) => {
                assert_eq!(command.texture.id(), texture_id);
                assert_eq!(command.frame, frame);
                assert_eq!(command.mask_tint, Some(Color::RED));
            }
            _ => panic!("expected texture command"),
        }
    }

    #[test]
    fn ordinary_texture_rejects_mask_tint_slot_for_fallback() {
        let mut computed = ComputedScene::<()>::default();
        computed.scene.push_texture(texture(1.0));
        assert!(slot_binding_for_reactive_value(
            &computed,
            ReactiveScenePropertyValue::TextureMaskTint { color: Color::RED },
        )
        .is_none());
    }
}
