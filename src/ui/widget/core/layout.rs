use super::*;

pub(super) fn apply_container_style(
    style: &mut TaffyStyle,
    layout: &ContainerLayout,
    animations: &mut AnimationEngine,
    widget_id: WidgetId,
    units: UnitContext,
    now: std::time::Instant,
) {
    style.padding = to_taffy_rect(
        layout
            .padding
            .as_ref()
            .map(|padding| {
                padding.resolve_widget(animations, widget_id, WidgetProperty::Padding, now)
            })
            .unwrap_or(Insets::ZERO),
        units,
    );
    let gap = layout
        .gap
        .resolve_widget(animations, widget_id, WidgetProperty::Gap, now);
    style.gap = TaffySize {
        width: resolve_length_percentage(&gap, units).unwrap_or(LengthPercentage::ZERO),
        height: resolve_length_percentage(&gap, units).unwrap_or(LengthPercentage::ZERO),
    };

    match &layout.kind {
        ContainerKind::Flow => {
            style.display = Display::Flex;
            style.flex_direction = FlexDirection::Column;
            style.justify_content = Some(map_justify_content(layout.justify));
            style.align_items = map_align_items(layout.align);
        }
        ContainerKind::Flex { direction, wrap } => {
            style.display = Display::Flex;
            style.flex_direction = match direction {
                Axis::Horizontal => FlexDirection::Row,
                Axis::Vertical => FlexDirection::Column,
            };
            style.flex_wrap = match wrap {
                Wrap::NoWrap => FlexWrap::NoWrap,
                Wrap::Wrap => FlexWrap::Wrap,
            };
            style.justify_content = Some(map_justify_content(layout.justify));
            style.align_items = map_align_items(layout.align);
            style.align_content = Some(map_align_content(layout.align));
        }
        ContainerKind::Grid { columns, rows } => {
            style.display = Display::Grid;
            style.grid_template_columns = if columns.is_empty() {
                vec![GridTemplateComponent::Single(TrackSizingFunction::AUTO)]
            } else {
                columns
                    .iter()
                    .copied()
                    .map(map_track)
                    .map(GridTemplateComponent::Single)
                    .collect()
            };
            style.grid_template_rows = if rows.is_empty() {
                vec![GridTemplateComponent::Single(TrackSizingFunction::AUTO)]
            } else {
                rows.iter()
                    .copied()
                    .map(map_track)
                    .map(GridTemplateComponent::Single)
                    .collect()
            };
            style.justify_content = Some(map_justify_content(layout.justify));
            style.align_content = Some(map_align_content(layout.align));
            style.justify_items = map_justify_items(layout.justify);
            style.align_items = map_align_items(layout.align);
        }
        ContainerKind::Stack => {
            style.display = Display::Grid;
            style.grid_template_columns =
                vec![GridTemplateComponent::Single(TrackSizingFunction::AUTO)];
            style.grid_template_rows =
                vec![GridTemplateComponent::Single(TrackSizingFunction::AUTO)];
            style.justify_items = map_justify_items(layout.justify);
            style.align_items = map_align_items(layout.align);
        }
    }
}

pub(super) fn compute_container_content_bounds<VM>(
    element: &ResolvedElement<VM>,
    children: &[ResolvedElement<VM>],
    layout_node: &LayoutNode,
    frame: Rect,
    context: &mut CollectContext<'_, '_>,
) -> Rect {
    let padding = match &element.kind {
        ResolvedWidgetKind::Container { layout, .. } => layout
            .padding
            .as_ref()
            .map(|padding| {
                padding.resolve_widget(
                    context.animations,
                    element.id,
                    WidgetProperty::Padding,
                    context.now,
                )
            })
            .unwrap_or(Insets::ZERO),
        _ => Insets::ZERO,
    };
    let mut bounds: Option<Rect> = None;

    for (child, child_layout) in children.iter().zip(layout_node.children.iter()) {
        let child_layout = context
            .taffy
            .layout(child_layout.node)
            .expect("child layout node should exist");
        let offset = child.visual.offset.resolve_widget(
            context.animations,
            child.id,
            WidgetProperty::Offset,
            context.now,
        );
        let child_frame = Rect::new(
            frame.x + child_layout.location.x + offset.x,
            frame.y + child_layout.location.y + offset.y,
            child_layout.size.width,
            child_layout.size.height,
        );
        bounds = Some(match bounds {
            Some(existing) => existing.union(child_frame),
            None => child_frame,
        });
    }

    bounds
        .map(|bounds| {
            Rect::new(
                bounds.x,
                bounds.y,
                bounds.width + padding.right,
                bounds.height + padding.bottom,
            )
        })
        .unwrap_or(Rect::new(frame.x, frame.y, 0.0, 0.0))
}

pub(super) fn apply_overflow_clip(
    parent_clip: Rect,
    frame: Rect,
    overflow_x: Overflow,
    overflow_y: Overflow,
) -> Rect {
    let x = if matches!(overflow_x, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.x.max(frame.x)
    } else {
        parent_clip.x
    };
    let y = if matches!(overflow_y, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.y.max(frame.y)
    } else {
        parent_clip.y
    };
    let right = if matches!(overflow_x, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.right().min(frame.right())
    } else {
        parent_clip.right()
    };
    let bottom = if matches!(overflow_y, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.bottom().min(frame.bottom())
    } else {
        parent_clip.bottom()
    };

    Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
}

pub(super) fn apply_overflow_clip_mask(
    parent_clip_mask: Option<ClipMask>,
    frame: Rect,
    corner_radius: f32,
    overflow_x: Overflow,
    overflow_y: Overflow,
) -> Option<ClipMask> {
    if matches!(overflow_x, Overflow::Hidden | Overflow::Scroll)
        && matches!(overflow_y, Overflow::Hidden | Overflow::Scroll)
        && corner_radius > 0.0
        && frame.width > Dp::ZERO
        && frame.height > Dp::ZERO
    {
        return Some(ClipMask {
            rect: frame,
            corner_radius,
        });
    }

    parent_clip_mask
}

#[derive(Clone, Copy, Default)]
pub(super) struct ScrollbarGeometry {
    pub(super) horizontal_track: Option<Rect>,
    pub(super) horizontal_thumb: Option<Rect>,
    pub(super) vertical_track: Option<Rect>,
    pub(super) vertical_thumb: Option<Rect>,
}

pub(super) fn compute_scrollbar_geometry(
    viewport: Rect,
    content_bounds: Rect,
    scroll_offset: Point,
    layout: &ContainerLayout,
    theme: &Theme,
    units: UnitContext,
) -> ScrollbarGeometry {
    let can_scroll_x =
        layout.overflow_x == Overflow::Scroll && content_bounds.right() > viewport.right();
    let can_scroll_y =
        layout.overflow_y == Overflow::Scroll && content_bounds.bottom() > viewport.bottom();
    if !can_scroll_x && !can_scroll_y {
        return ScrollbarGeometry::default();
    }

    let defaults = resolved_container_style(None, theme).scrollbar;
    let style = layout.scrollbar_style;
    let thickness = units.resolve_dp(
        style
            .thickness
            .or(defaults.thickness)
            .unwrap_or(dp(5.0))
            .max(dp(2.0)),
    );
    let inset_bounds = viewport.inset(style.insets.unwrap_or(Insets::ZERO));
    if inset_bounds.is_empty() {
        return ScrollbarGeometry::default();
    }

    let vertical_track = can_scroll_y.then(|| {
        Rect::new(
            (inset_bounds.right() - thickness).max(inset_bounds.x),
            inset_bounds.y,
            Dp::new(thickness).min(inset_bounds.width),
            (inset_bounds.height - if can_scroll_x { thickness } else { 0.0 }).max(0.0),
        )
    });
    let horizontal_track = can_scroll_x.then(|| {
        Rect::new(
            inset_bounds.x,
            (inset_bounds.bottom() - thickness).max(inset_bounds.y),
            (inset_bounds.width - if can_scroll_y { thickness } else { 0.0 }).max(0.0),
            Dp::new(thickness).min(inset_bounds.height),
        )
    });

    ScrollbarGeometry {
        horizontal_thumb: horizontal_track
            .filter(|track| !track.is_empty())
            .map(|track| {
                scrollbar_thumb_rect(
                    track,
                    viewport.width.get(),
                    scroll_offset.x.get(),
                    (content_bounds.right() - viewport.x)
                        .max(viewport.width)
                        .get(),
                    units.resolve_dp(
                        style
                            .min_thumb_length
                            .or(defaults.min_thumb_length)
                            .unwrap_or(dp(12.0))
                            .max(Dp::new(thickness)),
                    ),
                    Axis::Horizontal,
                )
            }),
        vertical_thumb: vertical_track
            .filter(|track| !track.is_empty())
            .map(|track| {
                scrollbar_thumb_rect(
                    track,
                    viewport.height.get(),
                    scroll_offset.y.get(),
                    (content_bounds.bottom() - viewport.y)
                        .max(viewport.height)
                        .get(),
                    units.resolve_dp(
                        style
                            .min_thumb_length
                            .or(defaults.min_thumb_length)
                            .unwrap_or(dp(12.0))
                            .max(Dp::new(thickness)),
                    ),
                    Axis::Vertical,
                )
            }),
        horizontal_track: horizontal_track.filter(|track| !track.is_empty()),
        vertical_track: vertical_track.filter(|track| !track.is_empty()),
    }
}

pub(super) fn push_scrollbar_primitives(
    scene: &mut ScenePrimitives,
    theme: &Theme,
    clip_rect: Rect,
    opacity: f32,
    layout: &ContainerLayout,
    geometry: ScrollbarGeometry,
    widget_id: WidgetId,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar: Option<ScrollbarHandle>,
) {
    if geometry.horizontal_track.is_none() && geometry.vertical_track.is_none() {
        return;
    }

    let track_clip = Some(clip_rect);
    let defaults = resolved_container_style(None, theme).scrollbar;
    let style = layout.scrollbar_style;
    let track_color = style
        .track_color
        .or(defaults.track_color)
        .unwrap_or(Color::TRANSPARENT)
        .with_alpha_factor(opacity);
    let thumb_color_for = |axis| {
        let handle = ScrollbarHandle {
            id: widget_id,
            axis,
        };
        let mut state = crate::ui::theme::WidgetState::default();
        if active_scrollbar == Some(handle) {
            state.pressed = true;
        } else if hovered_scrollbar == Some(handle) {
            state.hovered = true;
        }
        if state.pressed {
            style
                .active_thumb_color
                .or(style.thumb_color)
                .or(defaults.active_thumb_color)
                .or(defaults.thumb_color)
                .unwrap_or(Color::TRANSPARENT)
                .with_alpha_factor(opacity)
        } else if state.hovered {
            style
                .hover_thumb_color
                .or(style.thumb_color)
                .or(defaults.hover_thumb_color)
                .or(defaults.thumb_color)
                .unwrap_or(Color::TRANSPARENT)
                .with_alpha_factor(opacity)
        } else {
            style
                .thumb_color
                .or(defaults.thumb_color)
                .unwrap_or(Color::TRANSPARENT)
                .with_alpha_factor(opacity)
        }
    };
    let thickness = style
        .thickness
        .or(defaults.thickness)
        .unwrap_or(dp(12.0))
        .max(dp(2.0))
        .get();
    let radius = style
        .radius
        .or(defaults.radius)
        .unwrap_or(dp(999.0))
        .max(Dp::ZERO)
        .min(Dp::new(thickness * 0.5))
        .get();

    if let Some(track) = geometry.vertical_track {
        scene.push_overlay_shape(RenderPrimitive {
            rect: track,
            color: track_color,
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
        let thumb = geometry
            .vertical_thumb
            .expect("vertical thumb should exist with vertical track");
        scene.push_overlay_shape(RenderPrimitive {
            rect: thumb,
            color: thumb_color_for(ScrollbarAxis::Vertical),
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
    }

    if let Some(track) = geometry.horizontal_track {
        scene.push_overlay_shape(RenderPrimitive {
            rect: track,
            color: track_color,
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
        let thumb = geometry
            .horizontal_thumb
            .expect("horizontal thumb should exist with horizontal track");
        scene.push_overlay_shape(RenderPrimitive {
            rect: thumb,
            color: thumb_color_for(ScrollbarAxis::Horizontal),
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
    }
}

pub(super) fn scrollbar_thumb_rect(
    track: Rect,
    viewport_extent: f32,
    scroll_offset: f32,
    content_extent: f32,
    min_thumb_length: f32,
    axis: Axis,
) -> Rect {
    let track_extent = match axis {
        Axis::Horizontal => track.width,
        Axis::Vertical => track.height,
    }
    .max(0.0)
    .get();
    let max_offset = (content_extent - viewport_extent).max(0.0);
    let mut thumb_extent = if content_extent <= 0.0 {
        track_extent
    } else {
        track_extent * (viewport_extent / content_extent)
    };
    thumb_extent = thumb_extent.clamp(min_thumb_length.min(track_extent), track_extent);
    let travel = (track_extent - thumb_extent).max(0.0);
    let thumb_offset = if max_offset <= 0.0 || travel <= 0.0 {
        0.0
    } else {
        (scroll_offset.clamp(0.0, max_offset) / max_offset) * travel
    };

    match axis {
        Axis::Horizontal => Rect::new(track.x + thumb_offset, track.y, thumb_extent, track.height),
        Axis::Vertical => Rect::new(track.x, track.y + thumb_offset, track.width, thumb_extent),
    }
}

pub(super) fn map_align_items(align: Align) -> Option<TaffyAlignItems> {
    Some(match align {
        Align::Start => TaffyAlignItems::Start,
        Align::Center => TaffyAlignItems::Center,
        Align::End => TaffyAlignItems::End,
        Align::Stretch => TaffyAlignItems::Stretch,
    })
}

pub(super) fn map_align_self(align: Align) -> TaffyAlignItems {
    match align {
        Align::Start => TaffyAlignItems::Start,
        Align::Center => TaffyAlignItems::Center,
        Align::End => TaffyAlignItems::End,
        Align::Stretch => TaffyAlignItems::Stretch,
    }
}

pub(super) fn map_justify_content(justify: Justify) -> TaffyJustifyContent {
    match justify {
        Justify::Start => TaffyJustifyContent::Start,
        Justify::Center => TaffyJustifyContent::Center,
        Justify::End => TaffyJustifyContent::End,
        Justify::SpaceBetween => TaffyJustifyContent::SpaceBetween,
        Justify::SpaceAround => TaffyJustifyContent::SpaceAround,
        Justify::SpaceEvenly => TaffyJustifyContent::SpaceEvenly,
    }
}

pub(super) fn map_align_content(align: Align) -> TaffyAlignContent {
    match align {
        Align::Start => TaffyAlignContent::Start,
        Align::Center => TaffyAlignContent::Center,
        Align::End => TaffyAlignContent::End,
        Align::Stretch => TaffyAlignContent::Stretch,
    }
}

pub(super) fn map_justify_items(justify: Justify) -> Option<TaffyAlignItems> {
    match justify {
        Justify::Start => Some(TaffyAlignItems::Start),
        Justify::Center => Some(TaffyAlignItems::Center),
        Justify::End => Some(TaffyAlignItems::End),
        Justify::SpaceBetween | Justify::SpaceAround | Justify::SpaceEvenly => None,
    }
}

pub(super) fn map_track(track: Track) -> TrackSizingFunction {
    match track {
        Track::Auto => TrackSizingFunction::AUTO,
        Track::Px(value) => TrackSizingFunction::from_length(value.get()),
        Track::Percent(value) => TrackSizingFunction::from_percent(value),
        Track::Fr(value) => TrackSizingFunction::from_fr(value),
    }
}

pub(super) fn resolve_dimension(
    value: &Value<Length>,
    animations: &mut AnimationEngine,
    widget_id: WidgetId,
    property: WidgetProperty,
    now: std::time::Instant,
    units: UnitContext,
) -> Dimension {
    match value.resolve_widget(animations, widget_id, property, now) {
        Length::Auto => Dimension::AUTO,
        Length::Px(value) => Dimension::from_length(units.resolve_dp(value)),
        Length::Percent(value) => Dimension::from_percent(value),
    }
}

pub(super) fn resolve_length_percentage(
    value: &Length,
    units: UnitContext,
) -> Option<LengthPercentage> {
    match value {
        Length::Auto => None,
        Length::Px(value) => Some(LengthPercentage::from_length(units.resolve_dp(*value))),
        Length::Percent(value) => Some(LengthPercentage::from_percent(*value)),
    }
}

pub(super) fn resolve_length_percentage_auto(
    value: &Value<Length>,
    animations: &mut AnimationEngine,
    widget_id: WidgetId,
    property: WidgetProperty,
    now: std::time::Instant,
    units: UnitContext,
) -> LengthPercentageAuto {
    match value.resolve_widget(animations, widget_id, property, now) {
        Length::Auto => LengthPercentageAuto::AUTO,
        Length::Px(value) => LengthPercentageAuto::from_length(units.resolve_dp(value)),
        Length::Percent(value) => LengthPercentageAuto::from_percent(value),
    }
}

pub(super) fn to_taffy_rect(
    insets: Insets,
    units: UnitContext,
) -> taffy::prelude::Rect<taffy::style::LengthPercentage> {
    taffy::prelude::Rect {
        left: length(units.resolve_dp(insets.left)),
        right: length(units.resolve_dp(insets.right)),
        top: length(units.resolve_dp(insets.top)),
        bottom: length(units.resolve_dp(insets.bottom)),
    }
}

pub(super) fn to_taffy_rect_auto(
    insets: Insets,
    units: UnitContext,
) -> taffy::prelude::Rect<taffy::style::LengthPercentageAuto> {
    taffy::prelude::Rect {
        left: length(units.resolve_dp(insets.left)),
        right: length(units.resolve_dp(insets.right)),
        top: length(units.resolve_dp(insets.top)),
        bottom: length(units.resolve_dp(insets.bottom)),
    }
}

pub(super) fn measure_node(
    node_context: Option<&mut MeasureContext>,
    known_dimensions: TaffySize<Option<f32>>,
    font_manager: &FontManager,
    theme: &Theme,
    media: &MediaManager,
    units: UnitContext,
) -> TaffySize<f32> {
    let measured = match node_context {
        Some(MeasureContext::Text(text)) => measure_text_content(text, font_manager, theme, units),
        Some(MeasureContext::Image(image)) => {
            let snapshot = media.image_snapshot(&image.source.resolve(), None);
            measure_media_content(
                known_dimensions,
                image.layout.aspect_ratio.as_ref().map(Value::resolve),
                snapshot.intrinsic_size,
            )
        }
        Some(MeasureContext::Canvas(items)) => canvas_bounds(items)
            .map(|bounds| (bounds.width(), bounds.height()))
            .unwrap_or((0.0, 0.0)),
        #[cfg(feature = "video")]
        Some(MeasureContext::VideoSurface(video)) => {
            let snapshot = video.controller.surface_snapshot();
            measure_media_content(
                known_dimensions,
                video.layout.aspect_ratio.as_ref().map(Value::resolve),
                snapshot.intrinsic_size,
            )
        }
        Some(MeasureContext::Button { label, style }) => {
            let button_style = resolve_button_style(style, Default::default(), theme);
            let label_text = text_with_typography(label.clone(), &style.text_style);
            let text_size = measure_text_content(&label_text, font_manager, theme, units);
            let horizontal = units.resolve_dp(button_style.padding_x) * 2.0;
            let vertical = units.resolve_dp(button_style.padding_y) * 2.0;
            (
                text_size.0 + horizontal,
                text_size
                    .1
                    .max(units.resolve_dp(button_style.min_height))
                    .max(text_size.1 + vertical),
            )
        }
        Some(MeasureContext::Switch { style }) => {
            let switch_style = style;
            (
                units.resolve_dp(switch_style.width),
                units.resolve_dp(switch_style.height),
            )
        }
        Some(MeasureContext::Checkbox { label, style }) => {
            let checkbox_style = resolve_checkbox_style(style, Default::default(), false, theme);
            measure_checkbox_content(label.as_ref(), &checkbox_style, font_manager, theme, units)
        }
        Some(MeasureContext::Radio { label, style }) => {
            let radio_style = resolve_radio_style(style, Default::default(), false, theme);
            measure_radio_content(label.as_ref(), &radio_style, font_manager, theme, units)
        }
        Some(MeasureContext::Select {
            selected_label,
            placeholder,
            style,
        }) => measure_select_content(
            selected_label.as_deref(),
            placeholder,
            &resolve_select_style(style, Default::default(), theme),
            font_manager,
            theme,
            units,
        ),
        Some(MeasureContext::TextEditor {
            controller,
            placeholder,
            style,
            multiline,
        }) => {
            let value = controller.text();
            let content = if value.is_empty() {
                placeholder.clone()
            } else {
                value
            };
            let text = text_with_typography(Value::Static(content.clone()), &style.text_style);
            let text_size = if *multiline {
                let (_, line_height, _) = resolved_text_metrics(&text, theme, units);
                (
                    SELECT_DEFAULT_WIDTH,
                    line_height.max(units.resolve_dp(style.min_height)),
                )
            } else {
                measure_text_content(&text, font_manager, theme, units)
            };
            let horizontal = units.resolve_dp(style.padding_x) * 2.0;
            let vertical = units.resolve_dp(style.padding_y) * 2.0;
            (
                SELECT_DEFAULT_WIDTH.max(text_size.0 + horizontal),
                text_size
                    .1
                    .max(units.resolve_dp(style.min_height))
                    .max(text_size.1 + vertical),
            )
        }
        Some(MeasureContext::None) | None => (0.0, 0.0),
    };

    TaffySize {
        width: known_dimensions.width.unwrap_or(measured.0),
        height: known_dimensions.height.unwrap_or(measured.1),
    }
}

pub(super) fn measure_text_content(
    text: &Text,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let default_style = &theme.typography.body;
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    font_manager.measure_text(
        &text.content.resolve(),
        TextFontRequest {
            preferred_font: text
                .font_family
                .as_deref()
                .or(default_style.font_family.as_deref()),
            weight: text.font_weight.unwrap_or(default_style.weight),
        },
        font_size,
        line_height,
        letter_spacing,
    )
}

pub(super) fn text_from_content(content: impl Into<Value<String>>) -> Text {
    Text::new(content)
}

pub(super) fn measure_checkbox_content(
    label: Option<&Value<String>>,
    checkbox_style: &ResolvedCheckboxStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let size = units.resolve_dp(checkbox_style.size);
    let Some(label) = label else {
        return (size, size);
    };

    let label = checkbox_label_with_theme(label, checkbox_style);
    let label_size = measure_text_content(&label, font_manager, theme, units);
    (
        size + units.resolve_dp(checkbox_style.label_gap) + label_size.0,
        size.max(label_size.1),
    )
}

pub(super) fn checkbox_label_with_theme(
    label: &Value<String>,
    checkbox_style: &ResolvedCheckboxStyle,
) -> Text {
    let mut label = text_from_content(label.clone());
    if label.font_family.is_none() {
        label.font_family = checkbox_style.text_style.font_family.clone();
    }
    if label.font_size.is_none() {
        label.font_size = Some(checkbox_style.text_style.size);
    }
    if label.line_height.is_none() {
        label.line_height = checkbox_style.text_style.line_height;
    }
    if label.font_weight.is_none() {
        label.font_weight = Some(checkbox_style.text_style.weight);
    }
    if label.letter_spacing.is_none() {
        label.letter_spacing = checkbox_style.text_style.letter_spacing;
    }
    label
}

pub(super) fn measure_radio_content(
    label: Option<&Value<String>>,
    radio_style: &ResolvedRadioStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let size = units.resolve_dp(radio_style.size);
    let Some(label) = label else {
        return (size, size);
    };

    let label = radio_label_with_theme(label, radio_style);
    let label_size = measure_text_content(&label, font_manager, theme, units);
    (
        size + units.resolve_dp(radio_style.label_gap) + label_size.0,
        size.max(label_size.1),
    )
}

pub(super) fn radio_label_with_theme(
    label: &Value<String>,
    radio_style: &ResolvedRadioStyle,
) -> Text {
    let mut label = text_from_content(label.clone());
    if label.font_family.is_none() {
        label.font_family = radio_style.text_style.font_family.clone();
    }
    if label.font_size.is_none() {
        label.font_size = Some(radio_style.text_style.size);
    }
    if label.line_height.is_none() {
        label.line_height = radio_style.text_style.line_height;
    }
    if label.font_weight.is_none() {
        label.font_weight = Some(radio_style.text_style.weight);
    }
    if label.letter_spacing.is_none() {
        label.letter_spacing = radio_style.text_style.letter_spacing;
    }
    label
}

pub(super) fn default_layout_padding<VM>(element: &ResolvedElement<VM>, _theme: &Theme) -> Insets {
    match &element.kind {
        ResolvedWidgetKind::Button { style, .. } => {
            Insets::symmetric(style.padding_x, style.padding_y)
        }
        ResolvedWidgetKind::Select { style, .. } => {
            Insets::symmetric(style.padding_x, style.padding_y)
        }
        ResolvedWidgetKind::TextEditor { style, .. } => {
            Insets::symmetric(style.padding_x, style.padding_y)
        }
        ResolvedWidgetKind::Switch { style, .. } => style.padding,
        ResolvedWidgetKind::Checkbox { .. } => Insets::ZERO,
        ResolvedWidgetKind::Radio { .. } => Insets::ZERO,
        ResolvedWidgetKind::Text { .. } => Insets::ZERO,
        ResolvedWidgetKind::Container { .. } => Insets::ZERO,
        ResolvedWidgetKind::Image { .. } => Insets::ZERO,
        ResolvedWidgetKind::Canvas { .. } => Insets::ZERO,
        #[cfg(feature = "video")]
        ResolvedWidgetKind::VideoSurface { .. } => Insets::ZERO,
    }
}

pub(super) fn resolved_text_metrics(
    text: &Text,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32, f32) {
    let default_style = &theme.typography.body;
    let default_size = default_style.size.max(sp(1.0));
    let default_line_height_sp = text
        .line_height
        .or(default_style.line_height)
        .unwrap_or(text.font_size.unwrap_or(default_style.size) * 1.25);
    let font_size = units.resolve_sp(text.font_size.unwrap_or(default_size));
    let default_line_height = units.resolve_sp(default_line_height_sp);
    let default_font_size = units.resolve_sp(default_size);
    let scaled_line_height = if default_font_size > 0.0 {
        default_line_height * (font_size / default_font_size)
    } else {
        default_line_height
    };
    let line_height = default_line_height
        .max(scaled_line_height)
        .max(font_size + 4.0);
    let letter_spacing = units.resolve_sp(
        text.letter_spacing
            .unwrap_or(default_style.letter_spacing.unwrap_or(Sp::ZERO)),
    );
    (font_size, line_height, letter_spacing)
}

pub(super) fn text_with_typography(
    content: impl Into<Value<String>>,
    style: &crate::ui::theme::TextStyle,
) -> Text {
    let mut text = text_from_content(content);
    text.font_family = style.font_family.clone();
    text.font_size = Some(style.size);
    text.line_height = style.line_height;
    text.font_weight = Some(style.weight);
    text.letter_spacing = style.letter_spacing;
    text
}

pub(super) fn measure_media_content(
    known_dimensions: TaffySize<Option<f32>>,
    aspect_ratio: Option<f32>,
    intrinsic_size: IntrinsicSize,
) -> (f32, f32) {
    let ratio = aspect_ratio
        .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
        .or_else(|| intrinsic_size.aspect_ratio());

    match (known_dimensions.width, known_dimensions.height, ratio) {
        (Some(width), Some(height), _) => (width, height),
        (Some(width), None, Some(ratio)) => (width, width / ratio),
        (None, Some(height), Some(ratio)) => (height * ratio, height),
        (Some(width), None, None) => (width, intrinsic_size.height),
        (None, Some(height), None) => (intrinsic_size.width, height),
        (None, None, _) => (intrinsic_size.width, intrinsic_size.height),
    }
}
