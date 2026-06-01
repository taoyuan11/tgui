use super::*;

mod measure;
mod scroll;

pub(crate) use self::measure::*;
pub(crate) use self::scroll::*;

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
        ResolvedWidgetKind::Virtual { .. } => Insets::ZERO,
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
    let owner = match node_context.as_ref() {
        Some(MeasureContext::Text { id, .. })
        | Some(MeasureContext::Image { id, .. })
        | Some(MeasureContext::Canvas { id, .. })
        | Some(MeasureContext::Button { id, .. })
        | Some(MeasureContext::Checkbox { id, .. })
        | Some(MeasureContext::Radio { id, .. })
        | Some(MeasureContext::Switch { id, .. })
        | Some(MeasureContext::Select { id, .. })
        | Some(MeasureContext::Slider { id, .. })
        | Some(MeasureContext::ProgressBar { id, .. })
        | Some(MeasureContext::Spinner { id, .. })
        | Some(MeasureContext::Divider { id, .. })
        | Some(MeasureContext::TextEditor { id, .. }) => {
            Some(id.dependency_owner(DependencyPhase::Layout))
        }
        #[cfg(feature = "audio")]
        Some(MeasureContext::Audio { id, .. }) => {
            Some(id.dependency_owner(DependencyPhase::Layout))
        }
        #[cfg(feature = "video")]
        Some(MeasureContext::VideoSurface { id, .. }) => {
            Some(id.dependency_owner(DependencyPhase::Layout))
        }
        Some(MeasureContext::None) | None => None,
    };

    if let Some(owner) = owner {
        return track_dependency_scope(owner, || {
            measure_node_tracked(
                node_context,
                known_dimensions,
                font_manager,
                theme,
                media,
                units,
            )
        });
    }

    measure_node_tracked(
        node_context,
        known_dimensions,
        font_manager,
        theme,
        media,
        units,
    )
}

fn measure_node_tracked(
    node_context: Option<&mut MeasureContext>,
    known_dimensions: TaffySize<Option<f32>>,
    font_manager: &FontManager,
    theme: &Theme,
    media: &MediaManager,
    units: UnitContext,
) -> TaffySize<f32> {
    let measured = match node_context {
        Some(MeasureContext::Text { text, .. }) => {
            measure_text_content(text, font_manager, theme, units)
        }
        #[cfg(feature = "audio")]
        Some(MeasureContext::Audio { .. }) => (0.0, 0.0),
        Some(MeasureContext::Image { image, .. }) => {
            let snapshot = media.image_snapshot(&image.source.resolve(), None);
            measure_media_content(
                known_dimensions,
                image.layout.aspect_ratio.as_ref().map(Value::resolve),
                snapshot.intrinsic_size,
            )
        }
        Some(MeasureContext::Canvas { scene, .. }) => canvas_scene_bounds(&scene.resolve())
            .map(|bounds| (bounds.width(), bounds.height()))
            .unwrap_or((0.0, 0.0)),
        #[cfg(feature = "video")]
        Some(MeasureContext::VideoSurface { video, .. }) => {
            let snapshot = video.controller.surface_metadata();
            measure_media_content(
                known_dimensions,
                video.layout.aspect_ratio.as_ref().map(Value::resolve),
                snapshot.intrinsic_size,
            )
        }
        Some(MeasureContext::Button { label, style, .. }) => {
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
        Some(MeasureContext::Switch { style, .. }) => {
            let switch_style = style;
            (
                units.resolve_dp(switch_style.width),
                units.resolve_dp(switch_style.height),
            )
        }
        Some(MeasureContext::Checkbox { label, style, .. }) => {
            let checkbox_style = resolve_checkbox_style(style, Default::default(), false, theme);
            measure_checkbox_content(label.as_ref(), &checkbox_style, font_manager, theme, units)
        }
        Some(MeasureContext::Radio { label, style, .. }) => {
            let radio_style = resolve_radio_style(style, Default::default(), false, theme);
            measure_radio_content(label.as_ref(), &radio_style, font_manager, theme, units)
        }
        Some(MeasureContext::Select {
            selected_label,
            placeholder,
            style,
            ..
        }) => measure_select_content(
            selected_label.resolve().as_deref(),
            placeholder,
            &resolve_select_style(style, Default::default(), theme),
            font_manager,
            theme,
            units,
        ),
        Some(MeasureContext::Slider { style, .. }) => {
            let style = resolve_slider_style(style, Default::default(), theme);
            (
                units.resolve_dp(style.min_width),
                units.resolve_dp(style.min_height),
            )
        }
        Some(MeasureContext::ProgressBar {
            value: _,
            indeterminate: _,
            show_label,
            label,
            style,
            ..
        }) => {
            let base_width = units.resolve_dp(style.min_width);
            let track_height = units.resolve_dp(style.height);
            if *show_label {
                let content = label
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| Value::Static(String::from("0%")));
                let label_text = progress_bar_label_with_theme(&content, style);
                let label_size = measure_text_content(&label_text, font_manager, theme, units);
                (
                    base_width.max(label_size.0),
                    track_height + units.resolve_dp(style.gap) + label_size.1,
                )
            } else {
                (base_width, track_height)
            }
        }
        Some(MeasureContext::Spinner {
            style,
            size_override,
            ..
        }) => {
            let size = size_override
                .as_ref()
                .map(Value::resolve)
                .unwrap_or(style.size);
            let resolved = units.resolve_dp(size);
            (resolved, resolved)
        }
        Some(MeasureContext::Divider {
            orientation,
            thickness_override,
            label,
            style,
            ..
        }) => {
            let thickness = units
                .resolve_dp(
                    thickness_override
                        .as_ref()
                        .map(Value::resolve)
                        .unwrap_or_else(|| style.thickness.resolve()),
                )
                .max(1.0);
            if orientation.is_horizontal() {
                // 主轴（宽度）交由父级 / grow 决定，intrinsic 给 0；
                // cross（高度）= 线宽，带标签时取标签高度与线宽的较大者。
                let cross = if let Some(label) = label {
                    let label_text = text_with_typography(label.clone(), &style.text_style);
                    let label_size = measure_text_content(&label_text, font_manager, theme, units);
                    thickness.max(label_size.1)
                } else {
                    thickness
                };
                (0.0, cross)
            } else {
                // 垂直：cross（宽度）= 线宽；主轴（高度）交由父级决定。
                (thickness, 0.0)
            }
        }
        Some(MeasureContext::TextEditor {
            controller,
            placeholder,
            style,
            multiline,
            ..
        }) => {
            let text_size = if *multiline {
                let text = text_with_typography(Value::Static(String::new()), &style.text_style);
                let (_, line_height, _) = resolved_text_metrics(&text, theme, units);
                (
                    SELECT_DEFAULT_WIDTH,
                    line_height.max(units.resolve_dp(style.min_height)),
                )
            } else {
                let value = controller.text();
                let content = if value.is_empty() {
                    placeholder.resolve()
                } else {
                    value
                };
                let text = text_with_typography(Value::Static(content.clone()), &style.text_style);
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
