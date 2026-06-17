use super::super::scene::ReactiveProgressLabel;
use super::resolved_freeze::lifecycle_snapshot;
use super::*;
use crate::ui::widget::r#virtual::{apply_virtual_runtime_state_to_element, VirtualViewportHint};
use crate::ui::widget::{FocusScopeState, TransformRecord};

mod chrome;
mod controls;
mod drawer;
mod layout_media;
mod menu;
mod modal;
mod popover;
pub(crate) mod portal;
mod toast;
mod tooltip;

struct CollectResolvedStyles {
    button_style: Option<ResolvedButtonStyle>,
    select_style: Option<ResolvedSelectStyle>,
    slider_style: Option<ResolvedSliderStyle>,
    progress_bar_style: Option<crate::ui::widget::style::ProgressBarStyle>,
    spinner_style: Option<crate::ui::widget::style::SpinnerStyle>,
    divider_style: Option<crate::ui::widget::style::DividerStyle>,
    switch_style: Option<crate::ui::widget::style::SwitchStyle>,
    input_style: Option<ResolvedInputStyle>,
    checkbox_style: Option<ResolvedCheckboxStyle>,
    radio_style: Option<ResolvedRadioStyle>,
}

struct CollectVisualState {
    frame: Rect,
    background_frame: Rect,
    background_radius: Dp,
    runtime_visual: VisualStyle,
    offset: Point,
    reactive_offset: bool,
    primitive_clip: Option<Rect>,
    overflow_clip: Option<Rect>,
    primitive_clip_mask: Option<ClipMask>,
    disabled: bool,
    widget_state: WidgetState,
    opacity: f32,
    border_width: Dp,
    border_radius: Dp,
    border_color: Color,
    background: Color,
    reactive_background: bool,
    reactive_border_color: bool,
    reactive_opacity: bool,
    styles: CollectResolvedStyles,
}

fn has_static_fixed_frame(layout: &LayoutStyle) -> bool {
    let fixed_width = matches!(
        &layout.width,
        Some(Value::Static(Length::Px(width))) if *width > Dp::ZERO
    );
    let fixed_height = matches!(
        &layout.height,
        Some(Value::Static(Length::Px(height))) if *height > Dp::ZERO
    );
    fixed_width && fixed_height
}

struct CollectCaches<'a, VM> {
    lifecycle_states: &'a mut HashMap<WidgetId, LifecycleEventState<VM>>,
    chunks: &'a mut HashMap<WidgetId, ComputedScene<VM>>,
    chunk_parts: &'a mut HashMap<WidgetId, SceneChunkParts<VM>>,
    visual_contexts: &'a mut HashMap<WidgetId, VisualContextSnapshot>,
}

pub(super) fn prepare_nested_scene_root<VM>(
    root: &mut Element<VM>,
    context: &CollectContext<'_, '_>,
    fallback_viewport: Rect,
) {
    apply_virtual_runtime_state_to_element(
        root,
        context.scroll_offsets,
        context.virtual_states,
        VirtualViewportHint {
            width: fallback_viewport.width,
            height: fallback_viewport.height,
        },
    );
}

impl<VM: 'static> ResolvedElement<VM> {
    pub(in crate::ui::widget::core) fn resolve_reactive_transform_offset(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> Option<Point> {
        let visual = self.resolve_collect_visual_state(layout_node, visual_context, context);
        visual.reactive_offset.then_some(visual.offset)
    }

    pub(in crate::ui::widget::core) fn resolve_reactive_scene_property_value(
        &self,
        property: PropertySlot,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> Option<ReactiveScenePropertyValue> {
        let visual = self.resolve_collect_visual_state(layout_node, visual_context, context);
        match property {
            PropertySlot::Background => {
                if visual.background_frame.is_empty()
                    || (visual.background.a == 0 && !visual.reactive_background)
                {
                    return None;
                }
                let preserve_solid_background =
                    matches!(self.kind, ResolvedWidgetKind::Switch { .. });
                let draws_base_background = visual.runtime_visual.background_image.is_some()
                    || visual.runtime_visual.background_brush.is_none()
                    || preserve_solid_background;
                draws_base_background.then_some(ReactiveScenePropertyValue::ShapeFillColor {
                    rect: visual.background_frame,
                    color: visual.background,
                })
            }
            PropertySlot::BackgroundBrush => {
                if visual.background_frame.is_empty() {
                    return None;
                }
                let brush = visual
                    .runtime_visual
                    .background_brush
                    .as_ref()?
                    .resolve_widget();
                Some(ReactiveScenePropertyValue::Brush(BrushPrimitive {
                    rect: visual.background_frame,
                    brush,
                    corner_radius: visual.background_radius.get(),
                    clip_rect: visual.primitive_clip,
                    clip_mask: visual.primitive_clip_mask,
                }))
            }
            PropertySlot::BackgroundBlur => {
                if visual.background_frame.is_empty() {
                    return None;
                }
                let blur_radius = visual
                    .runtime_visual
                    .background_blur
                    .resolve_widget_to_logical(
                        context.animations,
                        self.id,
                        WidgetProperty::BackgroundBlur,
                        context.now,
                        context.units,
                    )
                    .max(0.0);
                Some(ReactiveScenePropertyValue::BackdropBlur(
                    BackdropBlurPrimitive {
                        rect: visual.background_frame,
                        corner_radius: visual.background_radius.get(),
                        blur_radius,
                        clip_rect: visual.primitive_clip,
                        clip_mask: visual.primitive_clip_mask,
                    },
                ))
            }
            PropertySlot::BorderColor => {
                if visual.frame.is_empty()
                    || (visual.border_color.a == 0 && !visual.reactive_border_color)
                {
                    return None;
                }
                let stroke_width = visual
                    .border_width
                    .get()
                    .min((visual.frame.width * 0.5).get())
                    .min((visual.frame.height * 0.5).get())
                    .max(0.0);
                (stroke_width > 0.0).then_some(ReactiveScenePropertyValue::ShapeStrokeColor {
                    rect: visual.frame,
                    stroke_width,
                    color: visual.border_color,
                })
            }
            PropertySlot::BorderWidth => {
                if visual.runtime_visual.shadow.is_some()
                    || visual.runtime_visual.background_brush.is_some()
                    || visual.runtime_visual.background_image.is_some()
                    || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                {
                    return None;
                }
                let background = if !visual.background_frame.is_empty() && visual.background.a > 0 {
                    Some((
                        visual.background_frame,
                        visual.background,
                        visual.background_radius.get(),
                    ))
                } else {
                    None
                };
                let stroke_width = visual
                    .border_width
                    .get()
                    .min((visual.frame.width * 0.5).get())
                    .min((visual.frame.height * 0.5).get())
                    .max(0.0);
                let border = if !visual.frame.is_empty()
                    && visual.border_color.a > 0
                    && stroke_width > 0.0
                {
                    Some((visual.frame, visual.border_color, stroke_width))
                } else {
                    None
                };
                if background.is_none() && border.is_none() {
                    return None;
                }
                Some(ReactiveScenePropertyValue::BorderWidth {
                    frame: visual.frame,
                    background,
                    border,
                })
            }
            PropertySlot::BorderRadius => {
                if visual.runtime_visual.shadow.is_some()
                    || visual.runtime_visual.background_brush.is_some()
                    || visual.runtime_visual.background_image.is_some()
                    || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                {
                    return None;
                }

                let background = if !visual.background_frame.is_empty() && visual.background.a > 0 {
                    Some((
                        visual.background_frame,
                        visual.background,
                        visual.background_radius.get(),
                    ))
                } else {
                    None
                };
                let stroke_width = visual
                    .border_width
                    .get()
                    .min((visual.frame.width * 0.5).get())
                    .min((visual.frame.height * 0.5).get())
                    .max(0.0);
                let border = if !visual.frame.is_empty()
                    && visual.border_color.a > 0
                    && stroke_width > 0.0
                {
                    Some((
                        visual.frame,
                        stroke_width,
                        visual.border_color,
                        visual.border_radius.get(),
                    ))
                } else {
                    None
                };
                if background.is_none() && border.is_none() {
                    return None;
                }
                Some(ReactiveScenePropertyValue::BorderRadius { background, border })
            }
            PropertySlot::Opacity => match &self.kind {
                ResolvedWidgetKind::Text { text, .. } => {
                    if text.user_select {
                        return None;
                    }
                    let color = text
                        .color
                        .as_ref()
                        .map(|color| {
                            color.resolve_widget(
                                context.animations,
                                self.id,
                                WidgetProperty::TextColor,
                                context.now,
                            )
                        })
                        .unwrap_or(context.theme.colors.on_surface)
                        .with_alpha_factor(visual.opacity);
                    Some(ReactiveScenePropertyValue::Opacity {
                        background: None,
                        border: None,
                        text: Some(color),
                    })
                }
                ResolvedWidgetKind::Container { children, .. } if children.is_empty() => {
                    if visual.runtime_visual.shadow.is_some()
                        || visual.runtime_visual.background_brush.is_some()
                        || visual.runtime_visual.background_image.is_some()
                        || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                    {
                        return None;
                    }
                    let background = if !visual.background_frame.is_empty()
                        && (visual.background.a > 0 || visual.reactive_opacity)
                    {
                        Some((visual.background_frame, visual.background))
                    } else {
                        None
                    };
                    let stroke_width = visual
                        .border_width
                        .get()
                        .min((visual.frame.width * 0.5).get())
                        .min((visual.frame.height * 0.5).get())
                        .max(0.0);
                    let border = if !visual.frame.is_empty()
                        && (visual.border_color.a > 0 || visual.reactive_opacity)
                        && stroke_width > 0.0
                    {
                        Some((visual.frame, stroke_width, visual.border_color))
                    } else {
                        None
                    };
                    if background.is_none() && border.is_none() {
                        return None;
                    }
                    Some(ReactiveScenePropertyValue::Opacity {
                        background,
                        border,
                        text: None,
                    })
                }
                ResolvedWidgetKind::Image { image, .. } => {
                    let has_border = visual.border_color.a > 0
                        && visual
                            .border_width
                            .get()
                            .min((visual.frame.width * 0.5).get())
                            .min((visual.frame.height * 0.5).get())
                            .max(0.0)
                            > 0.0;
                    if visual.background.a > 0
                        || has_border
                        || visual.runtime_visual.shadow.is_some()
                        || visual.runtime_visual.background_brush.is_some()
                        || visual.runtime_visual.background_image.is_some()
                        || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                    {
                        return None;
                    }
                    let source = image.source.resolve();
                    let metadata = context.media.image_snapshot(&source, None);
                    let target_frame = crate::media::resolve_media_rect(
                        visual.background_frame,
                        metadata.intrinsic_size,
                        image.fit,
                    );
                    let snapshot = if let Some(raster_request) =
                        crate::media::RasterRequest::from_frame(
                            target_frame,
                            context.units.scale_factor(),
                        ) {
                        context.media.image_snapshot(&source, Some(raster_request))
                    } else {
                        metadata
                    };
                    snapshot
                        .texture
                        .as_ref()
                        .map(|_| ReactiveScenePropertyValue::TextureOpacity {
                            frame: target_frame,
                            corner_radius: visual.background_radius.get(),
                            opacity: visual.opacity,
                        })
                }
                _ => None,
            },
            PropertySlot::Texture => match &self.kind {
                ResolvedWidgetKind::Image { image, .. } => {
                    let has_border = visual.border_color.a > 0
                        && visual
                            .border_width
                            .get()
                            .min((visual.frame.width * 0.5).get())
                            .min((visual.frame.height * 0.5).get())
                            .max(0.0)
                            > 0.0;
                    if visual.background.a > 0
                        || has_border
                        || visual.runtime_visual.shadow.is_some()
                        || visual.runtime_visual.background_brush.is_some()
                        || visual.runtime_visual.background_image.is_some()
                        || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                    {
                        return None;
                    }
                    let source = image.source.resolve();
                    let metadata = context.media.image_snapshot(&source, None);
                    let target_frame = crate::media::resolve_media_rect(
                        visual.background_frame,
                        metadata.intrinsic_size,
                        image.fit,
                    );
                    let raster_request = crate::media::RasterRequest::from_frame(
                        target_frame,
                        context.units.scale_factor(),
                    )?;
                    let snapshot = context.media.image_snapshot(&source, Some(raster_request));
                    snapshot
                        .texture
                        .map(|texture| ReactiveScenePropertyValue::Texture {
                            texture,
                            media_key: Some(crate::media::MediaTextureKey::new(
                                source,
                                raster_request,
                            )),
                            media_layout: Some(crate::media::MediaTextureLayout::new(
                                visual.background_frame,
                                image.fit,
                                context.units.scale_factor(),
                            )),
                            frame: target_frame,
                            corner_radius: visual.background_radius.get(),
                            opacity: visual.opacity.clamp(0.0, 1.0),
                            clip_rect: visual.primitive_clip,
                            clip_mask: visual.primitive_clip_mask,
                        })
                }
                ResolvedWidgetKind::Container { children, .. } if children.is_empty() => {
                    let has_border = visual.border_color.a > 0
                        && visual
                            .border_width
                            .get()
                            .min((visual.frame.width * 0.5).get())
                            .min((visual.frame.height * 0.5).get())
                            .max(0.0)
                            > 0.0;
                    if visual.background.a > 0
                        || has_border
                        || self.interactions.has_any()
                        || self.focus.focusable.is_some()
                        || self.focus.tab_index.is_some()
                        || self.focus.scope.is_some()
                        || visual.runtime_visual.shadow.is_some()
                        || visual.runtime_visual.background_brush.is_some()
                        || visual.runtime_visual.background_blur.resolve() > Dp::ZERO
                    {
                        return None;
                    }
                    let background_image = visual
                        .runtime_visual
                        .background_image
                        .as_ref()?
                        .resolve_widget();
                    let metadata = context.media.image_snapshot(&background_image.source, None);
                    let target_frame = crate::media::resolve_media_rect(
                        visual.background_frame,
                        metadata.intrinsic_size,
                        background_image.fit,
                    );
                    let raster_request = crate::media::RasterRequest::from_frame(
                        target_frame,
                        context.units.scale_factor(),
                    )?;
                    let snapshot = context
                        .media
                        .image_snapshot(&background_image.source, Some(raster_request));
                    snapshot
                        .texture
                        .map(|texture| ReactiveScenePropertyValue::Texture {
                            texture,
                            media_key: Some(crate::media::MediaTextureKey::new(
                                background_image.source,
                                raster_request,
                            )),
                            media_layout: Some(crate::media::MediaTextureLayout::new(
                                visual.background_frame,
                                background_image.fit,
                                context.units.scale_factor(),
                            )),
                            frame: target_frame,
                            corner_radius: visual.background_radius.get(),
                            opacity: 1.0,
                            clip_rect: visual.primitive_clip,
                            clip_mask: visual.primitive_clip_mask,
                        })
                }
                _ => None,
            },
            PropertySlot::Offset => {
                let ResolvedWidgetKind::Container { children, .. } = &self.kind else {
                    return None;
                };
                if !children.is_empty()
                    || self.interactions.has_any()
                    || self.focus.focusable.is_some()
                    || self.focus.tab_index.is_some()
                    || self.focus.scope.is_some()
                    || visual.runtime_visual.shadow.is_some()
                {
                    return None;
                }
                let draws_base_background = visual.runtime_visual.background_image.is_some()
                    || visual.runtime_visual.background_brush.is_none();
                let background = if draws_base_background
                    && !visual.background_frame.is_empty()
                    && visual.background.a > 0
                {
                    Some((visual.background_frame, visual.background))
                } else {
                    None
                };
                let backdrop_blur = if !visual.background_frame.is_empty() {
                    let blur_radius = visual
                        .runtime_visual
                        .background_blur
                        .resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BackgroundBlur,
                            context.now,
                            context.units,
                        )
                        .max(0.0);
                    (blur_radius > 0.0
                        || matches!(&visual.runtime_visual.background_blur, Value::Signal(_)))
                    .then_some(BackdropBlurPrimitive {
                        rect: visual.background_frame,
                        corner_radius: visual.background_radius.get(),
                        blur_radius,
                        clip_rect: visual.primitive_clip,
                        clip_mask: visual.primitive_clip_mask,
                    })
                } else {
                    None
                };
                let brush = if !visual.background_frame.is_empty() {
                    visual
                        .runtime_visual
                        .background_brush
                        .as_ref()
                        .map(|brush| BrushPrimitive {
                            rect: visual.background_frame,
                            brush: brush.resolve_widget(),
                            corner_radius: visual.background_radius.get(),
                            clip_rect: visual.primitive_clip,
                            clip_mask: visual.primitive_clip_mask,
                        })
                } else {
                    None
                };
                let texture = visual
                    .runtime_visual
                    .background_image
                    .as_ref()
                    .and_then(|image| {
                        let image = image.resolve_widget();
                        let metadata = context.media.image_snapshot(&image.source, None);
                        let target_frame = crate::media::resolve_media_rect(
                            visual.background_frame,
                            metadata.intrinsic_size,
                            image.fit,
                        );
                        let raster_request = crate::media::RasterRequest::from_frame(
                            target_frame,
                            context.units.scale_factor(),
                        )?;
                        let snapshot = context
                            .media
                            .image_snapshot(&image.source, Some(raster_request));
                        snapshot.texture.map(|texture| {
                            (
                                texture,
                                Some(crate::media::MediaTextureKey::new(
                                    image.source,
                                    raster_request,
                                )),
                                Some(crate::media::MediaTextureLayout::new(
                                    visual.background_frame,
                                    image.fit,
                                    context.units.scale_factor(),
                                )),
                                target_frame,
                                visual.background_radius.get(),
                                1.0,
                                visual.primitive_clip,
                                visual.primitive_clip_mask,
                            )
                        })
                    });
                let stroke_width = visual
                    .border_width
                    .get()
                    .min((visual.frame.width * 0.5).get())
                    .min((visual.frame.height * 0.5).get())
                    .max(0.0);
                let border = if !visual.frame.is_empty()
                    && visual.border_color.a > 0
                    && stroke_width > 0.0
                {
                    Some((visual.frame, stroke_width, visual.border_color))
                } else {
                    None
                };
                if background.is_none()
                    && border.is_none()
                    && backdrop_blur.is_none()
                    && brush.is_none()
                    && texture.is_none()
                {
                    return None;
                }
                Some(ReactiveScenePropertyValue::Offset {
                    background,
                    border,
                    backdrop_blur,
                    brush,
                    texture,
                })
            }
            PropertySlot::Scale => {
                let ResolvedWidgetKind::Container { children, .. } = &self.kind else {
                    return None;
                };
                if !children.is_empty()
                    || self.interactions.has_any()
                    || self.focus.focusable.is_some()
                    || self.focus.tab_index.is_some()
                    || self.focus.scope.is_some()
                    || visual.runtime_visual.shadow.is_some()
                {
                    return None;
                }
                let draws_base_background = visual.runtime_visual.background_image.is_some()
                    || visual.runtime_visual.background_brush.is_none();
                let background = if draws_base_background
                    && !visual.background_frame.is_empty()
                    && visual.background.a > 0
                {
                    Some((
                        visual.background_frame,
                        visual.background,
                        visual.background_radius.get(),
                    ))
                } else {
                    None
                };
                let backdrop_blur = if !visual.background_frame.is_empty() {
                    let blur_radius = visual
                        .runtime_visual
                        .background_blur
                        .resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BackgroundBlur,
                            context.now,
                            context.units,
                        )
                        .max(0.0);
                    (blur_radius > 0.0
                        || matches!(&visual.runtime_visual.background_blur, Value::Signal(_)))
                    .then_some(BackdropBlurPrimitive {
                        rect: visual.background_frame,
                        corner_radius: visual.background_radius.get(),
                        blur_radius,
                        clip_rect: visual.primitive_clip,
                        clip_mask: visual.primitive_clip_mask,
                    })
                } else {
                    None
                };
                let brush = if !visual.background_frame.is_empty() {
                    visual
                        .runtime_visual
                        .background_brush
                        .as_ref()
                        .map(|brush| BrushPrimitive {
                            rect: visual.background_frame,
                            brush: brush.resolve_widget(),
                            corner_radius: visual.background_radius.get(),
                            clip_rect: visual.primitive_clip,
                            clip_mask: visual.primitive_clip_mask,
                        })
                } else {
                    None
                };
                let texture = visual
                    .runtime_visual
                    .background_image
                    .as_ref()
                    .and_then(|image| {
                        let image = image.resolve_widget();
                        let metadata = context.media.image_snapshot(&image.source, None);
                        let target_frame = crate::media::resolve_media_rect(
                            visual.background_frame,
                            metadata.intrinsic_size,
                            image.fit,
                        );
                        let raster_request = crate::media::RasterRequest::from_frame(
                            target_frame,
                            context.units.scale_factor(),
                        )?;
                        let snapshot = context
                            .media
                            .image_snapshot(&image.source, Some(raster_request));
                        snapshot.texture.map(|texture| {
                            (
                                texture,
                                Some(crate::media::MediaTextureKey::new(
                                    image.source,
                                    raster_request,
                                )),
                                Some(crate::media::MediaTextureLayout::new(
                                    visual.background_frame,
                                    image.fit,
                                    context.units.scale_factor(),
                                )),
                                target_frame,
                                visual.background_radius.get(),
                                1.0,
                                visual.primitive_clip,
                                visual.primitive_clip_mask,
                            )
                        })
                    });
                let stroke_width = visual
                    .border_width
                    .get()
                    .min((visual.frame.width * 0.5).get())
                    .min((visual.frame.height * 0.5).get())
                    .max(0.0);
                let border = if !visual.frame.is_empty()
                    && visual.border_color.a > 0
                    && stroke_width > 0.0
                {
                    Some((
                        visual.frame,
                        stroke_width,
                        visual.border_color,
                        visual.border_radius.get(),
                    ))
                } else {
                    None
                };
                if background.is_none()
                    && border.is_none()
                    && backdrop_blur.is_none()
                    && brush.is_none()
                    && texture.is_none()
                {
                    return None;
                }
                Some(ReactiveScenePropertyValue::Scale {
                    background,
                    border,
                    backdrop_blur,
                    brush,
                    texture,
                })
            }
            PropertySlot::TextColor => {
                let ResolvedWidgetKind::Text { text, .. } = &self.kind else {
                    return None;
                };
                let color = text.color.as_ref()?.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::TextColor,
                    context.now,
                );
                Some(ReactiveScenePropertyValue::TextColor {
                    color: color.with_alpha_factor(visual.opacity),
                })
            }
            PropertySlot::TextContent => match &self.kind {
                ResolvedWidgetKind::Text { text, .. } => {
                    if text.user_select {
                        return None;
                    }
                    if !has_static_fixed_frame(&self.layout) {
                        return None;
                    }
                    let content = text.content.resolve();
                    if content.contains('\n') {
                        return None;
                    }
                    let default_style = &context.theme.typography.body;
                    let text_request = TextFontRequest {
                        preferred_font: text
                            .font_family
                            .as_deref()
                            .or(default_style.font_family.as_deref()),
                        weight: text.font_weight.unwrap_or(default_style.weight),
                    };
                    let resolved = context.font_manager.resolve_text(&content, text_request);
                    Some(ReactiveScenePropertyValue::TextContent {
                        content: std::sync::Arc::from(content),
                        font_family: Some(std::sync::Arc::from(resolved.primary_font)),
                    })
                }
                ResolvedWidgetKind::TextEditor {
                    controller,
                    placeholder,
                    multiline,
                    show_scrollbar,
                    auto_wrap,
                    ..
                } => {
                    if *multiline || context.focused_input == Some(self.id) {
                        return None;
                    }
                    if !has_static_fixed_frame(&self.layout) {
                        return None;
                    }
                    let input_style = visual.styles.input_style.as_ref()?;
                    let show_scrollbar = show_scrollbar.resolve();
                    let auto_wrap = auto_wrap.resolve();
                    let padding = Insets::symmetric(input_style.padding_x, input_style.padding_y);
                    let content_viewport = text_input_content_viewport(
                        visual.frame,
                        padding,
                        *multiline,
                        show_scrollbar,
                        context.theme,
                        context.units,
                    );
                    let content = controller.text();
                    let is_placeholder = content.is_empty();
                    let display_content = if is_placeholder {
                        placeholder.resolve()
                    } else {
                        content
                    };
                    if display_content.contains('\n') {
                        return None;
                    }
                    let text = text_with_typography("", &input_style.text_style);
                    let text_color = context.animations.resolve_color(
                        crate::animation::AnimationKey::Widget {
                            id: self.id.raw(),
                            property: WidgetProperty::TextColor,
                        },
                        if is_placeholder {
                            input_style.placeholder
                        } else {
                            input_style.text
                        },
                        default_state_transition(context.theme, context.reduced_motion),
                        context.now,
                    );
                    let (font_size, line_height, letter_spacing) =
                        resolved_text_metrics(&text, context.theme, context.units);
                    let default_style = &context.theme.typography.body;
                    let text_request = TextFontRequest {
                        preferred_font: text
                            .font_family
                            .as_deref()
                            .or(default_style.font_family.as_deref()),
                        weight: text.font_weight.unwrap_or(default_style.weight),
                    };
                    let layout = if auto_wrap {
                        context.font_manager.measure_text_layout_wrapped(
                            &display_content,
                            text_request.clone(),
                            font_size,
                            line_height,
                            letter_spacing,
                            text_input_layout_width(
                                content_viewport,
                                *multiline,
                                auto_wrap,
                                CARET_WIDTH,
                            ),
                        )
                    } else {
                        context.font_manager.measure_text_layout(
                            &display_content,
                            text_request.clone(),
                            font_size,
                            line_height,
                            letter_spacing,
                        )
                    };
                    let TextInputContentGeometry { content_frame, .. } =
                        text_input_content_geometry(
                            &layout,
                            line_height,
                            content_viewport,
                            *multiline,
                            auto_wrap,
                            Point::ZERO,
                            CARET_WIDTH,
                        );
                    let resolved = context
                        .font_manager
                        .resolve_text(&display_content, text_request);
                    let content_clip_rect = visual
                        .primitive_clip
                        .map(|clip| clip.intersect(content_viewport))
                        .unwrap_or(Some(content_viewport));
                    Some(ReactiveScenePropertyValue::TextInputContent(
                        TextPrimitive {
                            content: std::sync::Arc::from(display_content),
                            rich_spans: None,
                            frame: content_frame,
                            quad: None,
                            color: text_color.with_alpha_factor(visual.opacity),
                            force_color: false,
                            font_family: Some(std::sync::Arc::from(resolved.primary_font)),
                            font_size,
                            font_weight: text.font_weight.unwrap_or(default_style.weight),
                            line_height,
                            letter_spacing,
                            wrap: crate::ui::widget::CanvasTextWrap::Word,
                            overflow: crate::ui::widget::CanvasTextOverflow::Clip,
                            horizontal_align: crate::ui::widget::CanvasTextHorizontalAlign::Start,
                            vertical_align: crate::ui::widget::CanvasTextVerticalAlign::Start,
                            clip_rect: content_clip_rect,
                            clip_mask: visual.primitive_clip_mask,
                        },
                    ))
                }
                _ => None,
            },
            PropertySlot::ProgressValue => {
                let ResolvedWidgetKind::ProgressBar {
                    value,
                    indeterminate,
                    show_label,
                    label,
                    ..
                } = &self.kind
                else {
                    return None;
                };
                if indeterminate.resolve() {
                    return None;
                }
                let style = visual.styles.progress_bar_style.as_ref()?;
                let progress = value.resolve().clamp(0.0, 1.0);
                let track_rect = progress_bar_track_rect(
                    visual.frame,
                    style,
                    *show_label,
                    context.theme,
                    context.units,
                );
                if track_rect.width <= Dp::ZERO {
                    return None;
                }
                let fill_width = Dp::new(track_rect.width.get() * progress).min(track_rect.width);
                let track_color = style
                    .track_color
                    .resolve()
                    .with_alpha_factor(visual.opacity);
                let fill_color = style.fill_color.resolve().with_alpha_factor(visual.opacity);
                if track_color == fill_color {
                    return None;
                }
                let label = if *show_label && label.is_none() {
                    let content = format!("{:.0}%", progress * 100.0);
                    let label_text =
                        text_with_typography(Value::Static(content.clone()), &style.text_style);
                    let default_style = &context.theme.typography.body;
                    let text_request = TextFontRequest {
                        preferred_font: label_text
                            .font_family
                            .as_deref()
                            .or(default_style.font_family.as_deref()),
                        weight: label_text.font_weight.unwrap_or(default_style.weight),
                    };
                    let resolved = context.font_manager.resolve_text(&content, text_request);
                    Some(ReactiveProgressLabel {
                        frame: progress_bar_label_frame(
                            visual.frame,
                            style,
                            context.theme,
                            context.units,
                        ),
                        content: std::sync::Arc::from(content),
                        font_family: Some(std::sync::Arc::from(resolved.primary_font)),
                    })
                } else {
                    None
                };
                Some(ReactiveScenePropertyValue::ProgressFill {
                    track_rect,
                    fill_rect: Rect::new(track_rect.x, track_rect.y, fill_width, track_rect.height),
                    track_color,
                    fill_color,
                    label,
                })
            }
            PropertySlot::SliderValue => {
                let ResolvedWidgetKind::Slider {
                    value,
                    min,
                    max,
                    step,
                    orientation,
                    show_ticks,
                    show_value_label,
                    value_formatter,
                    ..
                } = &self.kind
                else {
                    return None;
                };
                if *show_ticks {
                    return None;
                }
                let style = visual.styles.slider_style.as_ref()?;
                if style.thumb_shadow.is_some() {
                    return None;
                }

                let mut geometry = slider_geometry(
                    visual.frame,
                    style,
                    *orientation,
                    *show_value_label,
                    context.units,
                );
                if geometry.track_rect.width <= Dp::ZERO
                    || geometry.track_rect.height <= Dp::ZERO
                    || geometry.thumb_rect.width <= Dp::ZERO
                    || geometry.thumb_rect.height <= Dp::ZERO
                {
                    return None;
                }

                let resolved_value = crate::ui::widget::common::slider_resolve_value(
                    value.resolve(),
                    *min,
                    *max,
                    *step,
                );
                let display_value = context
                    .active_slider_value
                    .filter(|(widget_id, _)| *widget_id == self.id)
                    .map(|(_, raw_value)| {
                        crate::ui::widget::common::slider_resolve_value(
                            raw_value, *min, *max, *step,
                        )
                    })
                    .unwrap_or(resolved_value);
                let normalized = crate::ui::widget::common::slider_normalized_value(
                    display_value,
                    *min,
                    *max,
                    *step,
                )
                .clamp(0.0, 1.0);
                let thumb_offset = if orientation.is_horizontal() {
                    Dp::new(geometry.track_rect.width.get() * normalized)
                } else {
                    Dp::new(geometry.track_rect.height.get() * (1.0 - normalized))
                };
                let active_extent = if orientation.is_horizontal() {
                    Dp::new(geometry.track_rect.width.get() * normalized)
                } else {
                    Dp::new(geometry.track_rect.height.get() * normalized)
                };

                let active_rect = if orientation.is_horizontal() {
                    Rect::new(
                        geometry.track_rect.x,
                        geometry.track_rect.y,
                        active_extent.min(geometry.track_rect.width),
                        geometry.track_rect.height,
                    )
                } else {
                    let height = active_extent.min(geometry.track_rect.height);
                    Rect::new(
                        geometry.track_rect.x,
                        geometry.track_rect.bottom() - height,
                        geometry.track_rect.width,
                        height,
                    )
                };

                if orientation.is_horizontal() {
                    geometry.thumb_rect.x = (geometry.track_rect.x + thumb_offset
                        - (geometry.thumb_rect.width * 0.5))
                        .clamp(
                            visual.frame.x,
                            (visual.frame.right() - geometry.thumb_rect.width).max(visual.frame.x),
                        );
                } else {
                    let min_y = geometry.track_rect.y - (geometry.thumb_rect.height * 0.5);
                    let max_y = geometry.track_rect.bottom() - (geometry.thumb_rect.height * 0.5);
                    let min_y = min_y.max(visual.frame.y);
                    let max_y = max_y
                        .min(
                            (visual.frame.bottom() - geometry.thumb_rect.height)
                                .max(visual.frame.y),
                        )
                        .max(min_y);
                    geometry.thumb_rect.y = (geometry.track_rect.y + thumb_offset
                        - (geometry.thumb_rect.height * 0.5))
                        .clamp(min_y, max_y);
                }

                let transition = default_state_transition(context.theme, context.reduced_motion);
                let track_color = context
                    .animations
                    .resolve_color(
                        crate::animation::AnimationKey::Widget {
                            id: self.id.raw(),
                            property: WidgetProperty::SliderTrackColor,
                        },
                        style.track,
                        transition,
                        context.now,
                    )
                    .with_alpha_factor(visual.opacity);
                let active_track_color = context
                    .animations
                    .resolve_color(
                        crate::animation::AnimationKey::Widget {
                            id: self.id.raw(),
                            property: WidgetProperty::SliderActiveTrackColor,
                        },
                        style.active_track,
                        transition,
                        context.now,
                    )
                    .with_alpha_factor(visual.opacity);
                if track_color == active_track_color {
                    return None;
                }
                let thumb_color = context
                    .animations
                    .resolve_color(
                        crate::animation::AnimationKey::Widget {
                            id: self.id.raw(),
                            property: WidgetProperty::SliderThumbColor,
                        },
                        style.thumb,
                        transition,
                        context.now,
                    )
                    .with_alpha_factor(visual.opacity);
                let thumb_border_width = context
                    .units
                    .resolve_dp(style.border_width)
                    .max(0.0)
                    .min((geometry.thumb_rect.width.get() * 0.5).max(0.0));
                let thumb_border =
                    (thumb_border_width > 0.0).then_some((track_color, thumb_border_width));
                let label = if *show_value_label {
                    let content = value_formatter
                        .as_ref()
                        .map(|formatter| formatter.format(display_value))
                        .unwrap_or_else(|| format!("{display_value:.2}"));
                    if content.contains('\n') {
                        return None;
                    }
                    let label_text =
                        text_with_typography(Value::Static(content.clone()), &style.text_style);
                    let default_style = &context.theme.typography.body;
                    let text_request = TextFontRequest {
                        preferred_font: label_text
                            .font_family
                            .as_deref()
                            .or(default_style.font_family.as_deref()),
                        weight: label_text.font_weight.unwrap_or(default_style.weight),
                    };
                    let resolved = context.font_manager.resolve_text(&content, text_request);
                    let (_, line_height, _) =
                        resolved_text_metrics(&label_text, context.theme, context.units);
                    Some(ReactiveProgressLabel {
                        frame: Rect::new(
                            visual.frame.x,
                            visual.frame.y,
                            visual.frame.width,
                            Dp::new(line_height),
                        ),
                        content: std::sync::Arc::from(content),
                        font_family: Some(std::sync::Arc::from(resolved.primary_font)),
                    })
                } else {
                    None
                };

                Some(ReactiveScenePropertyValue::SliderValue {
                    widget_id: self.id,
                    value: display_value,
                    track_rect: geometry.track_rect,
                    active_rect,
                    thumb_rect: geometry.thumb_rect,
                    track_color,
                    active_track_color,
                    thumb_color,
                    thumb_border,
                    label,
                })
            }
            PropertySlot::Width
            | PropertySlot::Height
            | PropertySlot::MinWidth
            | PropertySlot::MinHeight
            | PropertySlot::MaxWidth
            | PropertySlot::MaxHeight
            | PropertySlot::Margin
            | PropertySlot::Padding
            | PropertySlot::Grow
            | PropertySlot::Shrink
            | PropertySlot::Basis
            | PropertySlot::AspectRatio
            | PropertySlot::GridRow
            | PropertySlot::GridColumn
            | PropertySlot::Inset => None,
        }
    }

    pub(super) fn can_skip_when_fully_clipped(&self) -> bool {
        if !self.clip_cull_safe_self() {
            return false;
        }

        match &self.kind {
            ResolvedWidgetKind::Text { text, .. } => !text.user_select,
            ResolvedWidgetKind::Container {
                layout, children, ..
            } => {
                layout.overflow_x != Overflow::Scroll
                    && layout.overflow_y != Overflow::Scroll
                    && children
                        .iter()
                        .all(ResolvedElement::can_skip_when_fully_clipped)
            }
            _ => false,
        }
    }

    fn retained_transform_record_candidate(&self, visual: &CollectVisualState) -> bool {
        if !visual.reactive_offset {
            return false;
        }
        let ResolvedWidgetKind::Container { layout, .. } = &self.kind else {
            return false;
        };
        let has_runtime_overlay = self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some();
        let has_runtime_role = self.tab_trigger.is_some()
            || self.list_item.is_some()
            || self.tree_root.is_some()
            || self.tree_node.is_some()
            || self.data_grid_root.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
            || self.splitter_handle.is_some()
            || self.carousel_auto_play.is_some();

        layout.overflow_x == Overflow::Visible
            && layout.overflow_y == Overflow::Visible
            && layout.scroll_view.is_none()
            && !self.interactions.has_any()
            && !self.lifecycle_events.has_any()
            && !self.media_events.has_any()
            && self.focus.focusable.is_none()
            && self.focus.tab_index.is_none()
            && self.focus.scope.is_none()
            && !has_runtime_overlay
            && !has_runtime_role
            && visual.runtime_visual.shadow.is_none()
            && matches!(&visual.runtime_visual.background_blur, Value::Static(value) if *value == Dp::ZERO)
            && matches!(&visual.runtime_visual.scale, Value::Static(value) if (*value - 1.0).abs() <= f32::EPSILON)
    }

    fn collected_scene_allows_retained_transform(&self, computed: &ComputedScene<VM>) -> bool {
        let scene = &computed.scene;
        computed
            .hit_regions
            .iter()
            .all(|hit| hit.supports_retained_transform())
            && computed.overlay_hit_regions.is_empty()
            && computed.overlay_close_handlers.is_empty()
            && computed.focus_scopes.is_empty()
            && computed.carousel_auto_play.is_empty()
            && computed.overlay_anchors.is_empty()
            && computed.portal_entries.is_empty()
            && computed.external_portal_requests.is_empty()
            && computed.ime_cursor_area.is_none()
            && computed.virtual_state_updates.is_empty()
            && computed.scroll_regions.iter().all(|region| {
                region.overflow_x == Overflow::Visible && region.overflow_y == Overflow::Visible
            })
            && scene.backdrop_blurs.is_empty()
            && scene.brushes.is_empty()
            && scene.canvas_composites.is_empty()
            && scene.meshes.is_empty()
            && scene.shapes.iter().all(|shape| shape.clip_mask.is_none())
            && scene
                .textures
                .iter()
                .all(|texture| texture.clip_mask.is_none())
            && scene.texts.iter().all(|text| text.clip_mask.is_none())
            && scene
                .text_decorations
                .iter()
                .all(|decoration| decoration.clip_mask.is_none())
            && {
                #[cfg(feature = "video")]
                {
                    scene
                        .video_textures
                        .iter()
                        .all(|texture| texture.clip_mask.is_none())
                }
                #[cfg(not(feature = "video"))]
                {
                    true
                }
            }
            && scene.overlay_shapes.is_empty()
            && scene.overlay_textures.is_empty()
            && scene.overlay_meshes.is_empty()
            && scene.overlay_texts.is_empty()
            && scene.overlay_text_decorations.is_empty()
            && scene.overlay_commands.is_empty()
            && scene.overlay_command_gpu_scroll_containers.is_empty()
            && scene.overlay_command_transform_chains.is_empty()
    }

    fn clip_cull_safe_self(&self) -> bool {
        let has_runtime_overlay = self.tooltip.is_some()
            || self.popover.is_some()
            || self.menu.is_some()
            || self.context_menu.is_some()
            || self.modal.is_some()
            || self.drawer.is_some();
        let has_runtime_role = self.tab_trigger.is_some()
            || self.list_item.is_some()
            || self.tree_root.is_some()
            || self.tree_node.is_some()
            || self.data_grid_root.is_some()
            || self.data_grid_cell.is_some()
            || self.data_grid_header.is_some()
            || self.data_grid_resize_handle.is_some()
            || self.splitter_handle.is_some()
            || self.carousel_auto_play.is_some();

        !self.interactions.has_any()
            && !self.lifecycle_events.has_any()
            && !self.media_events.has_any()
            && self.focus.focusable.is_none()
            && self.focus.tab_index.is_none()
            && self.focus.scope.is_none()
            && !has_runtime_overlay
            && !has_runtime_role
            && self.visual.shadow.is_none()
            && matches!(&self.visual.background_blur, Value::Static(value) if *value == Dp::ZERO)
            && matches!(&self.visual.offset, Value::Static(value) if *value == Point::ZERO)
            && matches!(&self.visual.scale, Value::Static(value) if *value == 1.0)
    }

    /// 收集 `self` 子树的场景 chunk,把结果**移动**进 `chunks[self.id]`,
    /// 并返回该节点的 `WidgetId`。
    ///
    /// 旧实现这里返回整棵合并后的子树(owned `ComputedScene`),再被父节点 `extend`
    /// 后丢弃 —— 也就是说每个节点都把自己的合并子树深拷贝了两次(一次进 `chunks`,
    /// 一次作为返回值)。现在结果只在 `chunks` 里存一份:
    /// - 父节点通过 `chunks.get(&child.id)` 只读引用来 `extend`,不再深拷贝;
    /// - 需要 owned 场景的根/overlay 调用方在收集结束后 `chunks.get(&id).cloned()`,
    ///   仅根节点保留这一次必要的克隆。
    pub(in super::super) fn collect_subtree_cache(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
        chunk_parts: &mut HashMap<WidgetId, SceneChunkParts<VM>>,
        visual_contexts: &mut HashMap<WidgetId, VisualContextSnapshot>,
    ) -> WidgetId {
        super::super::tree::with_widget_stack_frame(|| {
            let owner = self.id.dependency_owner(DependencyPhase::Scene);
            track_dependency_scope(owner, || {
                self.collect_subtree_cache_tracked(
                    layout_node,
                    visual_context,
                    context,
                    lifecycle_states,
                    chunks,
                    chunk_parts,
                    visual_contexts,
                )
            })
        })
    }

    fn collect_subtree_cache_tracked(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
        chunk_parts: &mut HashMap<WidgetId, SceneChunkParts<VM>>,
        visual_contexts: &mut HashMap<WidgetId, VisualContextSnapshot>,
    ) -> WidgetId {
        let previous_focus = context.focus.clone();
        if let Some(scope) = self.focus.scope.as_ref() {
            let active = scope.is_active();
            context.focus.scope_path.push(self.id);
            if !active {
                context.focus.disabled_depth += 1;
            }
        }
        let mut caches = CollectCaches {
            lifecycle_states,
            chunks,
            chunk_parts,
            visual_contexts,
        };
        self.collect_runtime_lifecycle_state(caches.lifecycle_states);

        let mut computed = ComputedScene::default();
        computed
            .scene
            .set_active_gpu_scroll_container(context.gpu_scroll_container);
        computed
            .scene
            .set_active_transform_chain(&context.transform_stack);
        if let Some(scope) = self.focus.scope.as_ref() {
            computed.register_focus_scope(FocusScopeState {
                scope_id: self.id,
                path: context.focus.scope_path.clone(),
                options: scope.clone(),
                active: scope.is_active(),
            });
        }
        use super::super::collect_profile::{record_node, record_node_visible, timed, Phase};
        record_node();
        let visual = timed(Phase::VisualState, || {
            self.resolve_collect_visual_state(layout_node, visual_context, context)
        });
        let previous_transform_stack_len = context.transform_stack.len();
        let retained_transform_candidate = self.retained_transform_record_candidate(&visual);
        if retained_transform_candidate {
            context.transform_stack.push(self.id);
            computed
                .scene
                .set_active_transform_chain(&context.transform_stack);
        }
        // 节点 frame 与视口相交即记一次「可见」，配合 `record_node` 给出 recollect/visible 比值。
        if visual.frame.intersect(context.viewport).is_some() {
            record_node_visible();
        }
        timed(Phase::Surface, || {
            self.push_surface_primitives_and_base_hit_regions(&mut computed, context, &visual)
        });
        if let Some(auto_play) = self.carousel_auto_play.as_ref() {
            let mut auto_play = auto_play.clone();
            auto_play.frame = visual.frame;
            computed.carousel_auto_play.push(auto_play);
        }

        let kind_handled = timed(Phase::KindBody, || {
            self.collect_layout_media_kind(
                layout_node,
                visual_context,
                context,
                &mut caches,
                &mut computed,
                &visual,
            )
        });
        if !kind_handled {
            let handled = self.collect_control_kind(context, &mut computed, &visual);
            debug_assert!(
                handled,
                "unhandled widget kind in collect_subtree_cache_tracked"
            );
        }
        self.clear_closed_modal_interactions(&mut computed);
        // `before_overlays` 仅在 `Container`/`Virtual` 节点上被消费(用于把 overlay 增量
        // 并入 `chunk_parts.after_children`)。叶子节点占节点总数绝大多数,为它们克隆整份
        // `computed` 是纯浪费 —— 因此把这次快照限定到带子节点的 kind。
        let is_container_like = matches!(
            self.kind,
            ResolvedWidgetKind::Container { .. } | ResolvedWidgetKind::Virtual { .. }
        );
        let before_overlays = is_container_like.then(|| computed.clone());

        self.emit_tooltip_if_visible(context, &mut computed, &visual);
        self.emit_popover_overlay_if_visible(context, &mut computed, &visual);
        self.emit_menu_overlay_if_open(context, &mut computed, &visual);
        self.emit_modal_close_overlay_if_open(context, &mut computed, &visual);
        self.emit_drawer_close_overlay_if_open(context, &mut computed, &visual);
        self.emit_toast_overlay_if_visible(context, &mut computed, &visual);
        self.emit_portal_if_open(context, &mut computed, &visual);

        if let Some(before_overlays) = before_overlays {
            let overlay_delta = computed.delta_since(&before_overlays);
            if let Some(parts) = caches.chunk_parts.get_mut(&self.id) {
                parts.after_children.extend(&overlay_delta);
            }
        }

        timed(Phase::Bookkeeping, || {
            // `chunk_parts` 只被 `recompose_scene_chunk` 读取(scene_layout.rs),而 recompose
            // 仅在「祖先」节点上运行(scene_patch.rs / bench_support.rs 都由 `parent_of` 向上推导
            // 祖先集合),且仅对 `Container` / `Virtual` 这两种带子节点的 kind 迭代子 chunk。
            // 其余 kind 在 resolved 树里都是叶子,永远不会成为祖先 → 它们的 `chunk_parts` 是
            // 只写不读的死数据。而 `Container` / `Virtual` 收集臂(layout_media.rs)又各自显式
            // `insert` 了自己的 `chunk_parts`,所以这里的兜底 `or_insert_with` 对它们也已是空操作。
            //
            // 因此把兜底克隆限定到带子节点的 kind:对叶子(占节点总数绝大多数,且每个都要
            // `before_children: computed.clone()`)直接跳过,消除逐帧重收集里最大的单项独占开销
            // (n=1000 长列表约 22%)。即便未来出现意外读取,`recompose` 的 `chunk_parts.get(&id)?`
            // 会得到 `None` 并安全回退到整帧重收集,绝不会产生错误渲染。
            if is_container_like {
                caches
                    .chunk_parts
                    .entry(self.id)
                    .or_insert_with(|| SceneChunkParts {
                        before_children: computed.clone(),
                        after_children: ComputedScene::default(),
                    });
            }
            if let Some(gpu_scroll_container) = context.gpu_scroll_container {
                computed.fill_gpu_scroll_container(gpu_scroll_container);
                if let Some(parts) = caches.chunk_parts.get_mut(&self.id) {
                    parts
                        .before_children
                        .fill_gpu_scroll_container(gpu_scroll_container);
                    parts
                        .after_children
                        .fill_gpu_scroll_container(gpu_scroll_container);
                }
            }
            if retained_transform_candidate
                && self.collected_scene_allows_retained_transform(&computed)
            {
                computed.transform_records.insert(
                    self.id,
                    TransformRecord {
                        id: self.id,
                        base_offset: visual.offset,
                        current_offset: visual.offset,
                    },
                );
            }
            context
                .transform_stack
                .truncate(previous_transform_stack_len);
            computed
                .scene
                .set_active_transform_chain(&context.transform_stack);
            caches
                .visual_contexts
                .insert(self.id, visual_context.into());
            context.focus = previous_focus;
            // 把合并后的子树移动进 chunks(此前是 `clone` + 返回 owned 的双份拷贝)。
            // 父节点改为 `chunks.get(&child.id)` 只读引用来 extend。
            caches.chunks.insert(self.id, computed);
        });
        self.id
    }

    fn collect_runtime_lifecycle_state(
        &self,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
    ) {
        if self.lifecycle_events.has_any() || self.requires_runtime_lifecycle() {
            lifecycle_states.insert(
                self.id,
                LifecycleEventState {
                    widget_id: self.id,
                    snapshot: lifecycle_snapshot(self),
                    handlers: self.lifecycle_events.clone(),
                },
            );
        }
    }
}
