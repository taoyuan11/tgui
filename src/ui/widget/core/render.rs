use super::*;

pub(super) fn push_media_texture_or_placeholder<VM>(
    widget_id: WidgetId,
    source: &crate::media::MediaSource,
    fit: ContentFit,
    frame: Rect,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    opacity: f32,
    loading_background: Color,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
    kind: &str,
) {
    let metadata = context.media.image_snapshot(source, None);
    let target_frame = resolve_media_rect(content_frame, metadata.intrinsic_size, fit);
    let snapshot = if let Some(raster_request) =
        RasterRequest::from_frame(target_frame, context.units.scale_factor())
    {
        context.media.image_snapshot(source, Some(raster_request))
    } else {
        metadata
    };

    if let Some(texture) = snapshot.texture.as_ref() {
        computed.scene.push_texture(TexturePrimitive {
            texture: Arc::clone(texture),
            frame: target_frame,
            corner_radius: content_corner_radius,
            clip_rect,
            clip_mask,
        });
        return;
    }

    push_media_placeholder(
        frame,
        content_frame,
        content_corner_radius,
        clip_rect,
        clip_mask,
        opacity,
        context,
        &mut computed.scene,
        widget_id,
        kind,
        snapshot.loading,
        snapshot.error.as_deref(),
        loading_background,
        snapshot.loading,
    );
}

pub(super) fn push_background_media_texture<VM>(
    source: &crate::media::MediaSource,
    fit: ContentFit,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
) {
    let metadata = context.media.image_snapshot(source, None);
    let target_frame = resolve_media_rect(content_frame, metadata.intrinsic_size, fit);
    let snapshot = if let Some(raster_request) =
        RasterRequest::from_frame(target_frame, context.units.scale_factor())
    {
        context.media.image_snapshot(source, Some(raster_request))
    } else {
        metadata
    };

    if let Some(texture) = snapshot.texture.as_ref() {
        computed.scene.push_texture(TexturePrimitive {
            texture: Arc::clone(texture),
            frame: target_frame,
            corner_radius: content_corner_radius,
            clip_rect,
            clip_mask,
        });
    }
}

#[cfg(feature = "video")]
pub(super) fn push_video_texture_or_placeholder<VM>(
    widget_id: WidgetId,
    video: &PublicVideoSurface,
    frame: Rect,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    opacity: f32,
    loading_background: Color,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
) {
    let snapshot = video.controller.surface_snapshot();
    let target_frame = resolve_media_rect(content_frame, snapshot.intrinsic_size, video.fit);
    let use_surface_background =
        snapshot.loading || (snapshot.texture.is_none() && snapshot.error.is_none());

    if let Some(texture) = snapshot.texture.as_ref() {
        computed.scene.push_texture(TexturePrimitive {
            texture: Arc::clone(texture),
            frame: target_frame,
            corner_radius: content_corner_radius,
            clip_rect,
            clip_mask,
        });
        return;
    }

    push_media_placeholder(
        frame,
        content_frame,
        content_corner_radius,
        clip_rect,
        clip_mask,
        opacity,
        context,
        &mut computed.scene,
        widget_id,
        "video",
        snapshot.loading,
        snapshot.error.as_deref(),
        loading_background,
        use_surface_background,
    );
}

pub(super) fn push_media_placeholder(
    frame: Rect,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    opacity: f32,
    context: &mut CollectContext<'_, '_>,
    scene: &mut ScenePrimitives,
    widget_id: WidgetId,
    kind: &str,
    loading: bool,
    error: Option<&str>,
    loading_background: Color,
    use_loading_background: bool,
) {
    let placeholder =
        media_loading_fill_color(loading, error, loading_background, use_loading_background)
            .with_alpha_factor(opacity);
    if content_frame.width > Dp::ZERO && content_frame.height > Dp::ZERO {
        scene.push_shape(RenderPrimitive {
            rect: content_frame,
            color: placeholder,
            corner_radius: content_corner_radius,
            stroke_width: 0.0,
            clip_rect,
            clip_mask,
        });
    }

    let label = media_placeholder_label(kind, loading, error);
    let mut text = Text::new(label);
    text.font_size = Some((context.theme.typography.body_small.size - sp(1.0)).max(sp(12.0)));
    push_text_primitives(
        &text,
        frame,
        context.font_manager,
        context.theme,
        context.units,
        context.animations,
        context.now,
        scene,
        false,
        true,
        Insets::all(dp(12.0)),
        None,
        None,
        Color::hexa(0xE5E7EBFF),
        opacity,
        widget_id,
        clip_rect,
        clip_mask,
    );
}

pub(super) fn media_loading_fill_color(
    loading: bool,
    error: Option<&str>,
    loading_background: Color,
    use_loading_background: bool,
) -> Color {
    if use_loading_background {
        loading_background
    } else {
        media_placeholder_color(loading, error)
    }
}

pub(super) fn push_text_primitives(
    text: &Text,
    frame: Rect,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    show_caret: bool,
    center_horizontally: bool,
    padding: Insets,
    caret_content: Option<&str>,
    selection_state: Option<&TextEditState>,
    fallback_color: Color,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    let content = text.content.resolve();
    let default_style = &theme.typography.body;
    let text_request = TextFontRequest {
        preferred_font: text
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text.font_weight.unwrap_or(default_style.weight),
    };
    let resolved = font_manager.resolve_text(&content, text_request.clone());

    let color = text
        .color
        .as_ref()
        .map(|color| color.resolve_widget(animations, widget_id, WidgetProperty::TextColor, now))
        .unwrap_or(fallback_color);
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    let inner = frame.inset(padding);
    let current_layout = font_manager.measure_text_layout(
        &content,
        text_request.clone(),
        font_size,
        line_height,
        letter_spacing,
    );
    let content_frame = centered_text_frame(
        inner,
        current_layout.width,
        current_layout.height,
        line_height,
        center_horizontally,
    );

    if let Some((selection_start, selection_end)) = selection_state
        .cloned()
        .unwrap_or_else(|| TextEditState::caret_at(&content))
        .clamped_to(&content)
        .selection_range()
    {
        let selection_start = selection_start.min(content.len());
        let selection_end = selection_end.min(content.len());
        let selection_start_x = current_layout.x_for_index(selection_start);
        let selection_end_x = current_layout.x_for_index(selection_end);
        let selection_width = (selection_end_x - selection_start_x).max(0.0);
        if selection_width > 0.0 {
            scene.push_shape(RenderPrimitive {
                rect: Rect::new(
                    content_frame.x + selection_start_x,
                    content_frame.y,
                    selection_width,
                    content_frame.height.max(Dp::new(line_height)),
                ),
                color: theme.colors.selection.with_alpha_factor(opacity),
                corner_radius: 4.0,
                stroke_width: 0.0,
                clip_rect,
                clip_mask,
            });
        }
    }

    scene.push_text(TextPrimitive {
        content: content.clone(),
        frame: content_frame,
        color: color.with_alpha_factor(opacity),
        force_color: false,
        font_family: Some(resolved.primary_font),
        font_size,
        font_weight: text.font_weight.unwrap_or(default_style.weight),
        line_height,
        letter_spacing,
        clip_rect,
        clip_mask,
    });

    if show_caret {
        let caret_width = caret_content
            .map(|caret_text| {
                font_manager
                    .measure_text_raw(
                        caret_text,
                        text_request,
                        font_size,
                        line_height,
                        letter_spacing,
                    )
                    .0
            })
            .unwrap_or(current_layout.width);
        let caret_x = (inner.x + inner.width.min(caret_width) + CARET_END_GAP).max(inner.x);
        scene.push_overlay_shape(RenderPrimitive {
            rect: Rect::new(
                caret_x,
                content_frame.y,
                CARET_WIDTH,
                content_frame.height.max(Dp::new(line_height)),
            ),
            color: theme.colors.on_surface.with_alpha_factor(opacity),
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect,
            clip_mask,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_text_input_primitives(
    text: &Text,
    frame: Rect,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    show_caret: bool,
    multiline: bool,
    padding: Insets,
    edit_state: Option<&TextEditState>,
    fallback_color: Color,
    selection_color: Option<Color>,
    caret_color: Option<Color>,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) -> Option<Rect> {
    let resolved_content = text.content.resolve();
    let default_style = &theme.typography.body;
    let text_request = TextFontRequest {
        preferred_font: text
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text.font_weight.unwrap_or(default_style.weight),
    };
    let resolved_font = font_manager.resolve_text(&resolved_content, text_request.clone());

    let text_color = text
        .color
        .as_ref()
        .map(|color| color.resolve_widget(animations, widget_id, WidgetProperty::TextColor, now))
        .unwrap_or(fallback_color);
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    let inner = frame.inset(padding);
    let content_clip_rect = clip_rect
        .map(|clip| clip.intersect(inner))
        .unwrap_or(Some(inner));
    let wrap_width = inner.width.get().max(0.0);
    let base_state = edit_state
        .cloned()
        .unwrap_or_else(|| TextEditState::caret_at(&resolved_content))
        .clamped_to(&resolved_content);

    let (display_content, display_state, composition_range) =
        if let Some(composition) = base_state.composition.as_ref() {
            let start = composition.replace_range.0.min(resolved_content.len());
            let end = composition.replace_range.1.min(resolved_content.len());
            let mut display = String::with_capacity(
                resolved_content.len() + composition.text.len().saturating_sub(end - start),
            );
            display.push_str(&resolved_content[..start]);
            display.push_str(&composition.text);
            display.push_str(&resolved_content[end..]);
            let composition_end = start + composition.text.len();
            let caret_offset = composition
                .cursor
                .map(|(_, end)| end.min(composition.text.len()))
                .unwrap_or(composition.text.len());
            let caret = start + caret_offset;
            (
                display,
                TextEditState {
                    cursor: caret,
                    anchor: caret,
                    composition: None,
                    scroll_x: base_state.scroll_x,
                    scroll_y: base_state.scroll_y,
                    preferred_column_x: base_state.preferred_column_x,
                },
                Some((start, composition_end)),
            )
        } else {
            (resolved_content.clone(), base_state.clone(), None)
        };

    let layout = if multiline {
        font_manager.measure_text_layout_wrapped(
            &display_content,
            text_request.clone(),
            font_size,
            line_height,
            letter_spacing,
            wrap_width,
        )
    } else {
        font_manager.measure_text_layout(
            &display_content,
            text_request.clone(),
            font_size,
            line_height,
            letter_spacing,
        )
    };

    let content_width = if multiline {
        inner.width.max(0.0)
    } else {
        Dp::new(layout.width.max(inner.width.get() + CARET_WIDTH))
    };
    let content_height = if multiline {
        Dp::new(layout.height.max(line_height))
    } else {
        inner
            .height
            .min(layout.height.max(line_height))
            .max(Dp::new(line_height))
    };
    let scroll_x = if multiline {
        Dp::ZERO
    } else {
        display_state.scroll_x.clamp(
            0.0,
            (layout.width + CARET_WIDTH - inner.width.get()).max(0.0),
        )
    };
    let scroll_y = if multiline {
        display_state
            .scroll_y
            .clamp(0.0, (layout.height - inner.height.get()).max(0.0))
    } else {
        Dp::ZERO
    };
    let content_frame = Rect::new(
        inner.x - scroll_x,
        if multiline {
            inner.y - scroll_y
        } else {
            inner.y + ((inner.height - content_height).max(0.0) * 0.5)
        },
        content_width,
        content_height,
    );

    let selection_fill = selection_color.unwrap_or(theme.colors.selection);
    let caret_fill = caret_color.unwrap_or(theme.colors.on_surface);
    let mut selection_segments = Vec::new();
    if let Some((selection_start, selection_end)) = display_state.selection_range() {
        let start = selection_start.min(display_content.len());
        let end = selection_end.min(display_content.len());
        if start < end {
            let start_line = layout.line_index_for_index(start);
            let end_line = layout.line_index_for_index(end);
            for line_index in start_line..=end_line {
                let line_start = start.max(layout.line_start(line_index));
                let line_end = end.min(layout.line_end(line_index));
                let x0 = layout.x_for_index(line_start);
                let x1 = layout.x_for_index(line_end);
                let width = (x1 - x0).max(0.0);
                if width <= 0.0 {
                    continue;
                }
                selection_segments.push(Rect::new(
                    content_frame.x + x0,
                    content_frame.y + Dp::new(layout.line_top(line_index)),
                    width,
                    Dp::new(layout.line_height(line_index)),
                ));
            }
        }
    }
    if let Some((composition_start, composition_end)) = composition_range {
        let start_line = layout.line_index_for_index(composition_start);
        let end_line = layout.line_index_for_index(composition_end);
        for line_index in start_line..=end_line {
            let line_start = composition_start.max(layout.line_start(line_index));
            let line_end = composition_end.min(layout.line_end(line_index));
            let x0 = layout.x_for_index(line_start);
            let x1 = layout.x_for_index(line_end);
            let width = (x1 - x0).max(0.0);
            if width <= 0.0 {
                continue;
            }
            selection_segments.push(Rect::new(
                content_frame.x + x0,
                content_frame.y + Dp::new(layout.line_top(line_index)),
                width,
                Dp::new(layout.line_height(line_index)),
            ));
        }
    }
    for segment in selection_segments {
        scene.push_shape(RenderPrimitive {
            rect: segment,
            color: selection_fill.with_alpha_factor(opacity),
            corner_radius: 4.0,
            stroke_width: 0.0,
            clip_rect: content_clip_rect,
            clip_mask,
        });
    }

    scene.push_text(TextPrimitive {
        content: display_content.clone(),
        frame: content_frame,
        color: text_color.with_alpha_factor(opacity),
        force_color: false,
        font_family: Some(resolved_font.primary_font),
        font_size,
        font_weight: text.font_weight.unwrap_or(default_style.weight),
        line_height,
        letter_spacing,
        clip_rect: content_clip_rect,
        clip_mask,
    });

    let mut ime_cursor_area = None;
    if show_caret {
        let caret_index = display_state.cursor.min(display_content.len());
        let caret_x = content_frame.x + layout.x_for_index(caret_index);
        let caret_y = content_frame.y + Dp::new(layout.top_for_index(caret_index));
        let caret_height = Dp::new(layout.line_height_for_index(caret_index).max(line_height));
        let caret_rect = Rect::new(caret_x, caret_y, CARET_WIDTH, caret_height);
        ime_cursor_area = Some(caret_rect);
        scene.push_overlay_shape(RenderPrimitive {
            rect: caret_rect,
            color: caret_fill.with_alpha_factor(opacity),
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect: content_clip_rect,
            clip_mask,
        });
    }

    ime_cursor_area
}

pub(super) fn measure_select_content(
    selected_label: Option<&str>,
    placeholder: &Value<String>,
    select_style: &ResolvedSelectStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let display = selected_label
        .map(|label| select_display_text(text_from_content(label.to_string()), select_style))
        .unwrap_or_else(|| {
            select_display_text(text_from_content(placeholder.clone()), select_style)
        });
    let text_size = measure_text_content(&display, font_manager, theme, units);
    let horizontal = units.resolve_dp(select_style.padding_x) * 2.0 + units.resolve_dp(dp(24.0));
    let vertical = units.resolve_dp(select_style.padding_y) * 2.0;
    (
        SELECT_DEFAULT_WIDTH.max(text_size.0 + horizontal),
        text_size
            .1
            .max(units.resolve_dp(select_style.min_height))
            .max(text_size.1 + vertical),
    )
}

pub(super) fn select_display_text(mut text: Text, select_style: &ResolvedSelectStyle) -> Text {
    if text.font_family.is_none() {
        text.font_family = select_style.text_style.font_family.clone();
    }
    if text.font_size.is_none() {
        text.font_size = Some(select_style.text_style.size);
    }
    if text.font_weight.is_none() {
        text.font_weight = Some(select_style.text_style.weight);
    }
    if text.letter_spacing.is_none() {
        text.letter_spacing = select_style.text_style.letter_spacing;
    }
    text
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_select_primitives(
    frame: Rect,
    selected_label: Option<String>,
    placeholder: &Value<String>,
    select_style: &ResolvedSelectStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    padding: Insets,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    let arrow_width = dp(24.0);
    let text_frame = Rect::new(
        frame.x,
        frame.y,
        (frame.width - arrow_width).max(Dp::ZERO),
        frame.height,
    );
    match selected_label {
        Some(label) => push_select_text(
            &select_display_text(text_from_content(label), select_style),
            text_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            padding,
            select_style.text,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
            false,
        ),
        None => push_select_text(
            &select_display_text(text_from_content(placeholder.clone()), select_style),
            text_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            padding,
            select_style.placeholder,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
            false,
        ),
    }

    push_select_icon(
        Rect::new(
            (frame.right() - arrow_width).max(frame.x),
            frame.y,
            arrow_width.min(frame.width),
            frame.height,
        ),
        font_manager,
        select_style,
        units,
        scene,
        opacity,
        clip_rect,
        clip_mask,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_select_menu_primitives<VM>(
    widget_id: WidgetId,
    trigger_frame: Rect,
    viewport: Rect,
    options: &[SelectOptionState<VM>],
    on_open_change: Option<&ValueCommand<VM, bool>>,
    select_style: &ResolvedSelectStyle,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
    opacity: f32,
) {
    if options.is_empty() {
        return;
    }

    let option_height = context
        .units
        .resolve_dp(select_style.option_height)
        .max(1.0);
    let menu_height = option_height * options.len() as f32;
    let menu_gap = context.units.resolve_dp(select_style.menu_gap);
    let below_space = (viewport.bottom().get() - trigger_frame.bottom().get() - menu_gap).max(0.0);
    let above_space = (trigger_frame.y.get() - viewport.y.get() - menu_gap).max(0.0);
    let open_down = below_space >= menu_height || below_space >= above_space;
    let available_height = if open_down { below_space } else { above_space };
    let visible_height = menu_height.min(available_height).max(0.0);
    if visible_height <= 0.0 {
        return;
    }

    let menu_y = if open_down {
        trigger_frame.bottom().get() + menu_gap
    } else {
        trigger_frame.y.get() - menu_gap - visible_height
    };
    let menu_frame = Rect::new(trigger_frame.x, menu_y, trigger_frame.width, visible_height);
    let Some(menu_clip) = viewport.intersect(menu_frame) else {
        return;
    };
    let menu_clip = Some(menu_clip);
    let menu_corner_radius = context.units.resolve_dp(select_style.radius);
    let menu_clip_mask = Some(ClipMask {
        rect: menu_frame,
        corner_radius: menu_corner_radius,
    });

    computed.scene.push_overlay_shape(RenderPrimitive {
        rect: menu_frame,
        color: select_style.menu_background.with_alpha_factor(opacity),
        corner_radius: menu_corner_radius,
        stroke_width: 0.0,
        clip_rect: menu_clip,
        clip_mask: None,
    });

    let option_padding = Insets::symmetric(select_style.padding_x, Dp::ZERO);
    let disabled_text = default_select_disabled_text_color(context.theme);
    let mut option_interactions = InteractionHandlers::default();
    option_interactions.cursor_style = Some(Value::Static(CursorStyle::Pointer));

    for (index, option) in options.iter().enumerate() {
        let option_frame = Rect::new(
            menu_frame.x,
            menu_frame.y + option_height * index as f32,
            menu_frame.width,
            option_height,
        );
        let selected = option.selected.resolve();
        let option_disabled = option.disabled.resolve();
        let mut option_state = context.widget_states.get_select_option(widget_id, index);
        option_state.disabled = option_disabled;
        let hovered_option_color = default_select_menu_option_color(context.theme, option_state);
        let option_color = if option_state.hovered || option_state.pressed {
            hovered_option_color
        } else if selected {
            select_style.selected_option_background
        } else {
            hovered_option_color
        };
        if selected || option_color.a > 0 {
            computed.scene.push_overlay_shape(RenderPrimitive {
                rect: option_frame,
                color: option_color.with_alpha_factor(opacity),
                corner_radius: 0.0,
                stroke_width: 0.0,
                clip_rect: menu_clip,
                clip_mask: menu_clip_mask,
            });
        }

        push_select_text(
            &select_display_text(text_from_content(option.label.clone()), select_style),
            option_frame,
            context.font_manager,
            context.theme,
            context.units,
            context.animations,
            context.now,
            &mut computed.scene,
            option_padding,
            if option_disabled {
                disabled_text
            } else {
                select_style.text
            },
            opacity,
            widget_id,
            menu_clip,
            None,
            true,
        );

        computed.overlay_hit_regions.push(HitRegion {
            rect: option_frame,
            clip_rect: menu_clip,
            geometry: HitGeometry::Rect,
            interaction: if option_disabled {
                HitInteraction::Disabled { id: widget_id }
            } else {
                HitInteraction::SelectOption {
                    id: widget_id,
                    option_index: index,
                    interactions: option_interactions.clone(),
                    on_select: option.on_select.clone(),
                    on_open_change: on_open_change.cloned(),
                }
            },
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_select_text(
    text: &Text,
    frame: Rect,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    padding: Insets,
    fallback_color: Color,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    overlay: bool,
) {
    let content = text.content.resolve();
    let default_style = &theme.typography.body;
    let text_request = TextFontRequest {
        preferred_font: text
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text.font_weight.unwrap_or(default_style.weight),
    };
    let resolved = font_manager.resolve_text(&content, text_request.clone());
    let color = text
        .color
        .as_ref()
        .map(|color| color.resolve_widget(animations, widget_id, WidgetProperty::TextColor, now))
        .unwrap_or(fallback_color)
        .with_alpha_factor(opacity);
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    let inner = frame.inset(padding);
    let layout = font_manager.measure_text_layout(
        &content,
        text_request,
        font_size,
        line_height,
        letter_spacing,
    );
    let content_frame = centered_text_frame(inner, layout.width, layout.height, line_height, false);
    let primitive = TextPrimitive {
        content,
        frame: content_frame,
        color,
        force_color: false,
        font_family: Some(resolved.primary_font),
        font_size,
        font_weight: text.font_weight.unwrap_or(default_style.weight),
        line_height,
        letter_spacing,
        clip_rect,
        clip_mask,
    };
    if overlay {
        scene.push_overlay_text(primitive);
    } else {
        scene.push_text(primitive);
    }
}

pub(super) fn push_select_icon(
    frame: Rect,
    font_manager: &FontManager,
    select_style: &ResolvedSelectStyle,
    units: UnitContext,
    scene: &mut ScenePrimitives,
    opacity: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    let font_size = units
        .resolve_sp(select_style.text_style.size)
        .min(frame.width.get())
        .min(frame.height.get())
        .max(1.0);
    let line_height = font_size;
    let letter_spacing = 0.0;
    let text_request = TextFontRequest {
        preferred_font: Some(ICON_FONT_FAMILY),
        weight: select_style.text_style.weight,
    };
    let resolved = font_manager.resolve_text(SELECT_ARROW_ICON, text_request.clone());
    let layout = font_manager.measure_text_layout(
        SELECT_ARROW_ICON,
        text_request,
        font_size,
        line_height,
        letter_spacing,
    );
    let icon_frame = centered_text_frame(
        frame,
        layout.width.max(font_size),
        layout.height.max(line_height),
        line_height,
        true,
    );

    scene.push_text(TextPrimitive {
        content: SELECT_ARROW_ICON.to_string(),
        frame: icon_frame,
        color: select_style.arrow.with_alpha_factor(opacity),
        force_color: true,
        font_family: Some(resolved.primary_font),
        font_size,
        font_weight: select_style.text_style.weight,
        line_height,
        letter_spacing,
        clip_rect,
        clip_mask,
    });
}

pub(super) fn default_switch_transition() -> crate::animation::Transition {
    crate::animation::Transition::ease_in_out(std::time::Duration::from_millis(180))
}

pub(super) fn push_checkbox_primitives(
    frame: Rect,
    checked: bool,
    label: Option<&Value<String>>,
    checkbox_style: &ResolvedCheckboxStyle,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
) {
    let box_size = units.resolve_dp(checkbox_style.size);
    let box_frame = Rect::new(
        frame.x,
        frame.y + ((frame.height - box_size) * 0.5).max(Dp::ZERO),
        box_size,
        box_size,
    );
    let radius = units.resolve_dp(checkbox_style.radius);
    scene.push_shape(RenderPrimitive {
        rect: box_frame,
        color: checkbox_style.background.with_alpha_factor(opacity),
        corner_radius: radius,
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });
    let border_width = units.resolve_dp(checkbox_style.border_width);
    push_border_primitives(
        scene,
        box_frame,
        border_width,
        checkbox_style.border.with_alpha_factor(opacity),
        radius,
        clip_rect,
        clip_mask,
    );
    push_focus_ring_primitives(
        scene,
        box_frame,
        radius,
        checkbox_style.focus_ring.as_ref(),
        opacity,
    );

    if checked {
        push_checkbox_checkmark_primitives(
            box_frame,
            checkbox_style,
            opacity,
            font_manager,
            units,
            clip_rect,
            clip_mask,
            scene,
        );
    }

    if let Some(label) = label {
        let label = checkbox_label_with_theme(label, checkbox_style);
        let label_x = box_frame.right() + checkbox_style.label_gap;
        let label_frame = Rect::new(
            label_x,
            frame.y + dp(1.0),
            (frame.right() - label_x).max(Dp::ZERO),
            frame.height,
        );
        push_text_primitives(
            &label,
            label_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            false,
            false,
            Insets::ZERO,
            None,
            None,
            checkbox_style.label,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
        );
    }
}

pub(super) fn push_checkbox_checkmark_primitives(
    box_frame: Rect,
    checkbox_style: &ResolvedCheckboxStyle,
    opacity: f32,
    font_manager: &FontManager,
    units: UnitContext,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    scene: &mut ScenePrimitives,
) {
    let font_size = units
        .resolve_sp(checkbox_style.text_style.size)
        .min(box_frame.width.get())
        .min(box_frame.height.get())
        .max(1.0);
    let line_height = font_size;
    let letter_spacing = 0.0;
    let text_request = TextFontRequest {
        preferred_font: Some(ICON_FONT_FAMILY),
        weight: checkbox_style.text_style.weight,
    };
    let resolved = font_manager.resolve_text(CHECKBOX_CHECKMARK_ICON, text_request.clone());
    let layout = font_manager.measure_text_layout(
        CHECKBOX_CHECKMARK_ICON,
        text_request,
        font_size,
        line_height,
        letter_spacing,
    );
    let mut icon_frame = centered_text_frame(
        box_frame,
        layout.width.max(font_size),
        layout.height.max(line_height),
        line_height,
        true,
    );

    // 閻忓繐妫楅顕€宕熼幆褎绂堥柡宥呮搐閹粍绋夌€ｎ儷鈺呭礉?dp
    icon_frame.y += dp(1.0);

    scene.push_text(TextPrimitive {
        content: CHECKBOX_CHECKMARK_ICON.to_string(),
        frame: icon_frame,
        color: checkbox_style.checkmark.with_alpha_factor(opacity),
        force_color: true,
        font_family: Some(resolved.primary_font),
        font_size,
        font_weight: checkbox_style.text_style.weight,
        line_height,
        letter_spacing,
        clip_rect,
        clip_mask,
    });
}

pub(super) fn push_radio_primitives(
    frame: Rect,
    checked: bool,
    label: Option<&Value<String>>,
    radio_style: &ResolvedRadioStyle,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
) {
    let size = units.resolve_dp(radio_style.size);
    let control_frame = Rect::new(
        frame.x,
        frame.y + ((frame.height - size) * 0.5).max(Dp::ZERO),
        size,
        size,
    );
    let radius = units
        .resolve_dp(radio_style.radius)
        .min(size * 0.5)
        .max(0.0);
    scene.push_shape(RenderPrimitive {
        rect: control_frame,
        color: radio_style.background.with_alpha_factor(opacity),
        corner_radius: radius,
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });
    push_border_primitives(
        scene,
        control_frame,
        units.resolve_dp(radio_style.border_width),
        radio_style.border.with_alpha_factor(opacity),
        radius,
        clip_rect,
        clip_mask,
    );
    push_focus_ring_primitives(
        scene,
        control_frame,
        radius,
        radio_style.focus_ring.as_ref(),
        opacity,
    );

    if checked {
        let inset = dp(size * 0.28);
        let indicator_frame = control_frame.inset(Insets::all(inset));
        if indicator_frame.width > Dp::ZERO && indicator_frame.height > Dp::ZERO {
            let indicator_radius = (indicator_frame.width.min(indicator_frame.height).get() * 0.5)
                .min(radius)
                .max(0.0);
            scene.push_overlay_shape(RenderPrimitive {
                rect: indicator_frame,
                color: radio_style.indicator.with_alpha_factor(opacity),
                corner_radius: indicator_radius,
                stroke_width: 0.0,
                clip_rect,
                clip_mask,
            });
        }
    }

    if let Some(label) = label {
        let label = radio_label_with_theme(label, radio_style);
        let label_x = control_frame.right() + radio_style.label_gap;
        let label_frame = Rect::new(
            label_x,
            frame.y + dp(1.0),
            (frame.right() - label_x).max(Dp::ZERO),
            frame.height,
        );
        push_text_primitives(
            &label,
            label_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            false,
            false,
            Insets::ZERO,
            None,
            None,
            radio_style.label,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
        );
    }
}

pub(super) fn push_switch_primitives(
    background_frame: Rect,
    background_radius: f32,
    padding: Insets,
    checked: bool,
    active_thumb_color: Color,
    inactive_thumb_color: Color,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    animations: &mut AnimationEngine,
    scene: &mut ScenePrimitives,
    now: std::time::Instant,
) {
    let inner = background_frame.inset(padding);
    if inner.width <= Dp::ZERO || inner.height <= Dp::ZERO {
        return;
    }

    let thumb_diameter = inner.height.min(inner.width);
    if thumb_diameter <= Dp::ZERO {
        return;
    }

    let travel = (inner.width - thumb_diameter).max(Dp::ZERO);
    let thumb_offset = animations.resolve_dp(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::SwitchThumbOffset,
        },
        if checked { travel } else { Dp::ZERO },
        Some(default_switch_transition()),
        now,
    );
    let thumb_color = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::SwitchThumbColor,
        },
        if checked {
            active_thumb_color
        } else {
            inactive_thumb_color
        },
        Some(default_switch_transition()),
        now,
    );

    scene.push_overlay_shape(RenderPrimitive {
        rect: Rect::new(
            inner.x + thumb_offset,
            inner.y + ((inner.height - thumb_diameter) / 2.0),
            thumb_diameter,
            thumb_diameter,
        ),
        color: thumb_color.with_alpha_factor(opacity),
        corner_radius: (thumb_diameter.get() * 0.5).min(background_radius),
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });
}

pub(super) fn push_focus_ring_primitives(
    scene: &mut ScenePrimitives,
    frame: Rect,
    border_radius: f32,
    focus_ring: Option<&crate::theme::FocusRingStyle>,
    opacity: f32,
) {
    let Some(focus_ring) = focus_ring else {
        return;
    };
    if !focus_ring.enabled {
        return;
    }

    let width = focus_ring.width.get().max(0.0);
    if width <= 0.0 {
        return;
    }
    let gap = focus_ring.gap.get().max(0.0);
    let expansion = gap + (width * 0.5);
    let ring_frame = Rect::new(
        frame.x - expansion,
        frame.y - expansion,
        frame.width + expansion * 2.0,
        frame.height + expansion * 2.0,
    );
    if ring_frame.is_empty() {
        return;
    }

    scene.push_overlay_shape(RenderPrimitive {
        rect: ring_frame,
        color: focus_ring.color.with_alpha_factor(opacity),
        corner_radius: border_radius + expansion,
        stroke_width: width,
        clip_rect: None,
        clip_mask: None,
    });
}

pub(super) fn centered_text_frame(
    inner: Rect,
    measured_width: f32,
    measured_height: f32,
    line_height: f32,
    center_horizontally: bool,
) -> Rect {
    let content_height = inner
        .height
        .min(measured_height.max(line_height))
        .max(Dp::new(line_height));
    let content_width = inner.width.min(measured_width).max(0.0);
    let content_x = if center_horizontally {
        inner.x + ((inner.width - content_width).max(0.0) * 0.5)
    } else {
        inner.x
    };

    Rect::new(
        content_x,
        inner.y + ((inner.height - content_height).max(0.0) * 0.5),
        content_width,
        content_height,
    )
}

pub(super) fn push_border_primitives(
    scene: &mut ScenePrimitives,
    frame: Rect,
    border_width: f32,
    border_color: Color,
    border_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    if border_color.a == 0 {
        return;
    }

    let thickness = border_width
        .min((frame.width * 0.5).get())
        .min((frame.height * 0.5).get())
        .max(0.0);
    if thickness <= 0.0 {
        return;
    }

    scene.push_shape(RenderPrimitive {
        rect: frame,
        color: border_color,
        corner_radius: border_radius,
        stroke_width: thickness,
        clip_rect,
        clip_mask,
    });
}
