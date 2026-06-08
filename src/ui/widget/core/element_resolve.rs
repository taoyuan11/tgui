use super::element_path::resolved_child_elements_with_previous;
use super::*;
use crate::ui::theme::StyleContext;
use crate::ui::widget::r#virtual::{
    resolve_virtual_window_plan, VirtualCacheState, VirtualResolvedItemMeta, VirtualViewportHint,
};
use crate::ui::widget::StyleSheet;

impl<VM: 'static> Element<VM> {
    pub(super) fn resolve(&self, theme: &Theme) -> ResolvedElement<VM> {
        self.resolve_with_previous(theme, None)
    }

    pub(super) fn resolve_with_previous(
        &self,
        theme: &Theme,
        previous: Option<&ResolvedElement<VM>>,
    ) -> ResolvedElement<VM> {
        let empty_scroll_offsets = HashMap::new();
        let empty_virtual_states = HashMap::new();
        self.resolve_with_previous_and_runtime_state(
            theme,
            previous,
            &empty_scroll_offsets,
            &empty_virtual_states,
            VirtualViewportHint::default(),
        )
    }

    pub(super) fn resolve_with_runtime_state(
        &self,
        theme: &Theme,
        previous: Option<&ResolvedElement<VM>>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        fallback_viewport_hint: VirtualViewportHint,
    ) -> ResolvedElement<VM> {
        self.resolve_with_previous_and_runtime_state(
            theme,
            previous,
            scroll_offsets,
            virtual_states,
            fallback_viewport_hint,
        )
    }

    fn resolve_with_previous_and_runtime_state(
        &self,
        theme: &Theme,
        previous: Option<&ResolvedElement<VM>>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        fallback_viewport_hint: VirtualViewportHint,
    ) -> ResolvedElement<VM> {
        let default_style_sheet = StyleSheet::default();
        let context = StyleContext::from_theme(theme);
        self.resolve_with_previous_and_runtime_state_and_style_sheet(
            theme,
            previous,
            scroll_offsets,
            virtual_states,
            fallback_viewport_hint,
            &context,
            &default_style_sheet,
        )
    }

    pub(super) fn resolve_with_runtime_state_and_style_sheet(
        &self,
        theme: &Theme,
        previous: Option<&ResolvedElement<VM>>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        fallback_viewport_hint: VirtualViewportHint,
        context: &StyleContext<'_>,
        style_sheet: &StyleSheet,
    ) -> ResolvedElement<VM> {
        self.resolve_with_previous_and_runtime_state_and_style_sheet(
            theme,
            previous,
            scroll_offsets,
            virtual_states,
            fallback_viewport_hint,
            context,
            style_sheet,
        )
    }

    fn resolve_with_previous_and_runtime_state_and_style_sheet(
        &self,
        theme: &Theme,
        previous: Option<&ResolvedElement<VM>>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        fallback_viewport_hint: VirtualViewportHint,
        context: &StyleContext<'_>,
        style_sheet: &StyleSheet,
    ) -> ResolvedElement<VM> {
        super::tree::with_widget_stack_frame(|| {
            self.resolve_with_previous_inner(
                theme,
                previous,
                scroll_offsets,
                virtual_states,
                fallback_viewport_hint,
                context,
                style_sheet,
            )
        })
    }

    fn resolve_with_previous_inner(
        &self,
        theme: &Theme,
        previous: Option<&ResolvedElement<VM>>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        virtual_states: &HashMap<WidgetId, VirtualCacheState>,
        fallback_viewport_hint: VirtualViewportHint,
        context: &StyleContext<'_>,
        style_sheet: &StyleSheet,
    ) -> ResolvedElement<VM> {
        // `id` may be carried over from the previously resolved element so widget
        // identity stays stable across rebuilds.
        let id = previous.map(|previous| previous.id).unwrap_or(self.id);

        // Only the flat per-node fields are needed for the output; `kind` (which
        // holds the recursive child subtree) is borrowed and rebuilt, never cloned.
        // Cloning `self` here would deep-copy the entire subtree at every level of
        // the recursion, which dominates full scene rebuilds.
        let layout = self.layout.clone();
        let mut visual = self.visual.clone();
        let mut background = self.background.clone();
        let mut child_source_spans = Vec::new();
        let kind = match &self.kind {
            WidgetKind::Container {
                layout: container_layout,
                children,
                style,
            } => {
                let base_style = container_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                let mut layout = container_layout.clone();
                layout.scrollbar_style = resolved_style.scrollbar;
                let previous_children = previous
                    .and_then(|previous| match &previous.kind {
                        ResolvedWidgetKind::Container { children, .. } => Some(children.as_slice()),
                        _ => None,
                    })
                    .unwrap_or(&[]);
                let resolved_children = resolved_child_elements_with_previous(
                    id,
                    children,
                    previous_children,
                    Some(&mut child_source_spans),
                )
                .into_iter()
                .map(|(child, previous_child)| {
                    child.resolve_with_runtime_state_and_style_sheet(
                        theme,
                        previous_child,
                        scroll_offsets,
                        virtual_states,
                        fallback_viewport_hint.clone(),
                        context,
                        style_sheet,
                    )
                })
                .collect();
                ResolvedWidgetKind::Container {
                    layout,
                    children: resolved_children,
                    runtime_style: ResolvedRuntimeSurfaceStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                        explicit_visual: self.visual.clone(),
                        explicit_background: self.background.clone(),
                    },
                }
            }
            WidgetKind::Virtual {
                arrangement,
                item_layout,
                source,
                content_cross_extent,
                overflow_x,
                overflow_y,
                style,
                runtime_state,
            } => {
                let base_style = container_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                let mut runtime_state = runtime_state.clone();
                if let Some(cache) = virtual_states.get(&id) {
                    runtime_state.viewport_hint = cache.viewport_hint.clone();
                    runtime_state.measured_extents = cache.measured_extents.clone();
                    runtime_state.widget_ids_by_key = cache.widget_ids_by_key.clone();
                    runtime_state.bootstrap = runtime_state.viewport_hint.is_none();
                } else {
                    runtime_state.viewport_hint = None;
                    runtime_state.measured_extents.clear();
                    runtime_state.widget_ids_by_key.clear();
                    runtime_state.bootstrap = true;
                }
                runtime_state.fallback_viewport_hint = fallback_viewport_hint.clone();
                runtime_state.scroll_offset =
                    scroll_offsets.get(&id).copied().unwrap_or(Point::ZERO);
                if matches!(
                    arrangement.direction(),
                    crate::ui::widget::VirtualDirection::Vertical
                ) && content_cross_extent.is_none()
                {
                    runtime_state.scroll_offset.x = Dp::ZERO;
                } else if matches!(
                    arrangement.direction(),
                    crate::ui::widget::VirtualDirection::Horizontal
                ) && content_cross_extent.is_none()
                {
                    runtime_state.scroll_offset.y = Dp::ZERO;
                }
                let window_plan = resolve_virtual_window_plan(
                    *arrangement,
                    *item_layout,
                    &runtime_state,
                    source.len(),
                    runtime_state.fallback_viewport_hint.clone(),
                    content_cross_extent.as_ref().map(|value| value.resolve()),
                );
                let previous_children = previous
                    .and_then(|previous| match &previous.kind {
                        ResolvedWidgetKind::Virtual { children, .. } => Some(children.as_slice()),
                        _ => None,
                    })
                    .unwrap_or(&[]);
                let previous_by_index: HashMap<usize, &ResolvedElement<VM>> = previous
                    .and_then(|previous| match &previous.kind {
                        ResolvedWidgetKind::Virtual {
                            child_meta,
                            children,
                            ..
                        } => Some(
                            child_meta
                                .iter()
                                .zip(children.iter())
                                .map(|(meta, child)| (meta.item_index, child))
                                .collect(),
                        ),
                        _ => None,
                    })
                    .unwrap_or_default();
                let mut child_meta = Vec::with_capacity(window_plan.placements.len());
                let mut children = Vec::with_capacity(window_plan.placements.len());
                for placement in &window_plan.placements {
                    let Some(mut child) = source.build(placement.item_index, *context, style_sheet)
                    else {
                        continue;
                    };
                    child.layout.position_type = crate::ui::layout::PositionType::Absolute;
                    child.layout.left = Some(Value::Static(crate::ui::layout::Length::Px(
                        placement.cross_offset,
                    )));
                    child.layout.top = Some(Value::Static(crate::ui::layout::Length::Px(
                        placement.main_offset,
                    )));
                    let item_key = source
                        .key(placement.item_index)
                        .unwrap_or_else(|| WidgetKey::from(placement.item_index));
                    let use_live_cross_extent =
                        content_cross_extent.is_none() && arrangement.lanes() == 1;
                    match arrangement.direction() {
                        crate::ui::widget::VirtualDirection::Vertical => {
                            child.layout.width = Some(Value::Static(if use_live_cross_extent {
                                crate::ui::layout::Length::Percent(1.0)
                            } else {
                                crate::ui::layout::Length::Px(placement.cross_extent)
                            }));
                        }
                        crate::ui::widget::VirtualDirection::Horizontal => {
                            child.layout.height = Some(Value::Static(if use_live_cross_extent {
                                crate::ui::layout::Length::Percent(1.0)
                            } else {
                                crate::ui::layout::Length::Px(placement.cross_extent)
                            }));
                        }
                    }
                    let previous_id = runtime_state.widget_ids_by_key.get(&item_key).copied();
                    let previous_child = previous_by_index
                        .get(&placement.item_index)
                        .copied()
                        .or_else(|| {
                            previous_id.and_then(|id| {
                                previous_children.iter().find(|child| child.id == id)
                            })
                        })
                        .or_else(|| previous_children.get(children.len()));
                    if child.key.is_none() {
                        child.key = Some(item_key.clone());
                    }
                    if let Some(previous_child) = previous_child {
                        child.id = previous_child.id;
                    } else if let Some(previous_id) = previous_id {
                        child.id = previous_id;
                    }
                    child_meta.push(VirtualResolvedItemMeta {
                        item_index: placement.item_index,
                    });
                    children.push(child.resolve_with_runtime_state_and_style_sheet(
                        theme,
                        previous_child,
                        scroll_offsets,
                        virtual_states,
                        fallback_viewport_hint.clone(),
                        context,
                        style_sheet,
                    ));
                }
                ResolvedWidgetKind::Virtual {
                    arrangement: *arrangement,
                    item_layout: *item_layout,
                    content_cross_extent: content_cross_extent.clone(),
                    overflow_x: *overflow_x,
                    overflow_y: *overflow_y,
                    style: resolved_style,
                    runtime_style: ResolvedRuntimeSurfaceStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                        explicit_visual: self.visual.clone(),
                        explicit_background: self.background.clone(),
                    },
                    runtime_state,
                    window_plan,
                    children,
                    child_meta,
                }
            }
            WidgetKind::Text { text } => {
                let mut text = text.clone();
                let local_style = text.style.as_ref().cloned();
                let base_style = text_widget_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    text.style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                apply_text_widget_style(&mut text, &resolved_style);
                ResolvedWidgetKind::Text {
                    text,
                    runtime_style: ResolvedRuntimeSurfaceStyle {
                        base: base_style,
                        local: local_style,
                        explicit_visual: self.visual.clone(),
                        explicit_background: self.background.clone(),
                    },
                }
            }
            #[cfg(feature = "audio")]
            WidgetKind::Audio { audio } => ResolvedWidgetKind::Audio {
                audio: audio.clone(),
            },
            WidgetKind::Image { image } => {
                let mut image = image.clone();
                let local_style = image.style.as_ref().cloned();
                let base_style = image_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    image.style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                image.background = resolved_style.surface.background.clone();
                image.fit = resolved_style.fit;
                ResolvedWidgetKind::Image {
                    image,
                    runtime_style: ResolvedRuntimeSurfaceStyle {
                        base: base_style,
                        local: local_style,
                        explicit_visual: self.visual.clone(),
                        explicit_background: self.background.clone(),
                    },
                }
            }
            WidgetKind::Canvas {
                scene,
                item_interactions,
                style,
            } => {
                let base_style = canvas_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                ResolvedWidgetKind::Canvas {
                    scene: scene.clone(),
                    item_interactions: item_interactions.clone(),
                    runtime_style: ResolvedRuntimeSurfaceStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                        explicit_visual: self.visual.clone(),
                        explicit_background: self.background.clone(),
                    },
                }
            }
            #[cfg(feature = "video")]
            WidgetKind::VideoSurface { video, style } => {
                let mut video = video.clone();
                let local_style = style.as_ref().cloned();
                let base_style = video_surface_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                video.background = resolved_style.surface.background.clone();
                video.fit = resolved_style.fit;
                ResolvedWidgetKind::VideoSurface {
                    video,
                    style: resolved_style,
                    runtime_style: ResolvedRuntimeSurfaceStyle {
                        base: base_style,
                        local: local_style,
                        explicit_visual: self.visual.clone(),
                        explicit_background: self.background.clone(),
                    },
                }
            }
            WidgetKind::Button {
                label,
                disabled,
                variant,
                style,
            } => {
                let base_style = button_style_base(context, style_sheet, &self.visual, *variant);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                ResolvedWidgetKind::Button {
                    label: label.clone(),
                    disabled: disabled.clone(),
                    variant: *variant,
                    style: resolved_style,
                    runtime_style: ResolvedRuntimeStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                    },
                }
            }
            WidgetKind::Checkbox {
                checked,
                label,
                on_change,
                disabled,
                validation,
                style,
            } => {
                let base_style = checkbox_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                ResolvedWidgetKind::Checkbox {
                    checked: checked.clone(),
                    label: label.clone(),
                    on_change: on_change.clone(),
                    disabled: disabled.clone(),
                    validation: validation.clone(),
                    style: resolved_style,
                    runtime_style: ResolvedRuntimeStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                    },
                }
            }
            WidgetKind::Radio {
                checked,
                label,
                on_change,
                disabled,
                validation,
                style,
            } => {
                let base_style = radio_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                ResolvedWidgetKind::Radio {
                    checked: checked.clone(),
                    label: label.clone(),
                    on_change: on_change.clone(),
                    disabled: disabled.clone(),
                    validation: validation.clone(),
                    style: resolved_style,
                    runtime_style: ResolvedRuntimeStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                    },
                }
            }
            WidgetKind::Switch {
                checked,
                on_change,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                validation,
                style,
            } => {
                let base_style = switch_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                ResolvedWidgetKind::Switch {
                    checked: checked.clone(),
                    on_change: on_change.clone(),
                    active_background: active_background.clone(),
                    inactive_background: inactive_background.clone(),
                    active_thumb_color: active_thumb_color.clone(),
                    inactive_thumb_color: inactive_thumb_color.clone(),
                    disabled: disabled.clone(),
                    validation: validation.clone(),
                    style: resolved_style,
                    runtime_style: ResolvedRuntimeStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                    },
                }
            }
            WidgetKind::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                validation,
                style,
            } => {
                let base_style = select_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                ResolvedWidgetKind::Select {
                    selected_label: selected_label.clone(),
                    placeholder: placeholder.clone(),
                    options: options.clone(),
                    open: open.clone(),
                    on_open_change: on_open_change.clone(),
                    disabled: disabled.clone(),
                    validation: validation.clone(),
                    style: resolved_style,
                    runtime_style: ResolvedRuntimeStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                    },
                }
            }
            WidgetKind::SelectOptionRow {
                owner_id,
                option_index,
                option,
                on_open_change,
                style,
            } => ResolvedWidgetKind::SelectOptionRow {
                owner_id: *owner_id,
                option_index: *option_index,
                option: option.clone(),
                on_open_change: on_open_change.clone(),
                style: style.clone(),
            },
            WidgetKind::Slider {
                value,
                min,
                max,
                step,
                show_ticks,
                show_value_label,
                tick_count,
                value_formatter,
                on_change,
                disabled,
                validation,
                style,
            } => {
                let base_style = slider_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                ResolvedWidgetKind::Slider {
                    value: value.clone(),
                    min: *min,
                    max: *max,
                    step: *step,
                    show_ticks: *show_ticks,
                    show_value_label: *show_value_label,
                    tick_count: *tick_count,
                    value_formatter: value_formatter.clone(),
                    on_change: on_change.clone(),
                    disabled: disabled.clone(),
                    validation: validation.clone(),
                    style: resolved_style,
                    runtime_style: ResolvedRuntimeStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                    },
                }
            }
            WidgetKind::ProgressBar {
                value,
                indeterminate,
                show_label,
                label,
                style,
            } => {
                let base_style = progress_bar_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                ResolvedWidgetKind::ProgressBar {
                    value: value.clone(),
                    indeterminate: indeterminate.clone(),
                    show_label: *show_label,
                    label: label.clone(),
                    style: resolved_style,
                    runtime_style: ResolvedRuntimeStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                    },
                }
            }
            WidgetKind::Spinner {
                style,
                size_override,
                thickness_override,
                track_override,
            } => {
                let base_style = spinner_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                ResolvedWidgetKind::Spinner {
                    style: resolved_style,
                    runtime_style: ResolvedRuntimeStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                    },
                    size_override: size_override.clone(),
                    thickness_override: thickness_override.clone(),
                    track_override: *track_override,
                }
            }
            WidgetKind::Divider {
                orientation,
                dashed,
                color_override,
                thickness_override,
                inset_override,
                label,
                style,
            } => {
                let base_style = divider_style_base(context, style_sheet, &self.visual);
                let resolved_style = apply_local_style(
                    style.as_ref(),
                    base_style.clone(),
                    context,
                    style_sheet,
                    &self.visual,
                );
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                ResolvedWidgetKind::Divider {
                    orientation: *orientation,
                    dashed: dashed.clone(),
                    color_override: color_override.clone(),
                    thickness_override: thickness_override.clone(),
                    inset_override: inset_override.clone(),
                    label: label.clone(),
                    style: resolved_style,
                    runtime_style: ResolvedRuntimeStyle {
                        base: base_style,
                        local: style.as_ref().cloned(),
                    },
                }
            }
            WidgetKind::TextEditor {
                controller,
                placeholder,
                on_change,
                on_change_set,
                disabled,
                input_style,
                textarea_style,
                multiline,
                show_scrollbar,
                auto_wrap,
                validation,
            } => {
                let (resolved_style, runtime_style) = if *multiline {
                    let base_style = textarea_style_base(context, style_sheet, &self.visual);
                    let resolved_textarea = apply_local_style(
                        textarea_style.as_ref(),
                        base_style.clone(),
                        context,
                        style_sheet,
                        &self.visual,
                    );
                    (
                        input_style_from_textarea_style(resolved_textarea),
                        ResolvedTextEditorRuntimeStyle::Textarea(ResolvedRuntimeStyle {
                            base: base_style,
                            local: textarea_style.as_ref().cloned(),
                        }),
                    )
                } else {
                    let base_style = input_style_base(context, style_sheet, &self.visual);
                    let resolved_input = apply_local_style(
                        input_style.as_ref(),
                        base_style.clone(),
                        context,
                        style_sheet,
                        &self.visual,
                    );
                    (
                        resolved_input,
                        ResolvedTextEditorRuntimeStyle::Input(ResolvedRuntimeStyle {
                            base: base_style,
                            local: input_style.as_ref().cloned(),
                        }),
                    )
                };
                ResolvedWidgetKind::TextEditor {
                    controller: controller.clone(),
                    placeholder: placeholder.clone(),
                    on_change: on_change.clone(),
                    on_change_set: on_change_set.clone(),
                    disabled: disabled.clone(),
                    style: resolved_style,
                    runtime_style,
                    multiline: *multiline,
                    show_scrollbar: show_scrollbar.clone(),
                    auto_wrap: auto_wrap.clone(),
                    validation: validation.clone(),
                }
            }
            WidgetKind::ToastHost {
                queue,
                placement,
                max_visible,
                style,
            } => {
                let mut resolved_style =
                    crate::ui::widget::style::ToastStyle::default_for_theme(theme);
                context
                    .theme
                    .components
                    .toast
                    .apply(&mut resolved_style, context);
                style_sheet.apply_toast(&mut resolved_style, context, &self.visual);
                style_sheet.apply_toast_state(
                    &mut resolved_style,
                    context,
                    &self.visual,
                    crate::ui::theme::WidgetState::default(),
                );
                ResolvedWidgetKind::ToastHost {
                    queue: queue.clone(),
                    placement: *placement,
                    max_visible: *max_visible,
                    style: style
                        .as_ref()
                        .map(|resolver| resolver.resolve_from(resolved_style.clone(), context))
                        .unwrap_or(resolved_style),
                }
            }
            WidgetKind::Portal {
                content,
                open,
                target,
                anchor,
                options,
                layer,
                on_open_change,
                return_focus_to,
                close_on_outside_click,
                close_on_escape,
                focus_scope,
            } => ResolvedWidgetKind::Portal {
                content: content.clone(),
                open: open.clone(),
                target: target.clone(),
                anchor: anchor.clone(),
                options: options.clone(),
                layer: *layer,
                on_open_change: on_open_change.clone(),
                return_focus_to: *return_focus_to,
                close_on_outside_click: *close_on_outside_click,
                close_on_escape: *close_on_escape,
                focus_scope: focus_scope.clone(),
            },
        };

        ResolvedElement {
            id,
            key: self.key.clone(),
            layout,
            focus: self.focus.clone(),
            visual,
            interactions: self.interactions.clone(),
            lifecycle_events: self.lifecycle_events.clone(),
            media_events: self.media_events.clone(),
            background,
            tooltip: self.tooltip.clone(),
            popover: self.popover.clone(),
            menu: self.menu.clone(),
            context_menu: self.context_menu.clone(),
            modal: self.modal.clone(),
            drawer: self.drawer.clone(),
            tab_trigger: self.tab_trigger.clone(),
            list_item: self.list_item.clone(),
            tree_root: self.tree_root.clone(),
            tree_node: self.tree_node.clone(),
            data_grid_root: self.data_grid_root.clone(),
            data_grid_cell: self.data_grid_cell.clone(),
            data_grid_header: self.data_grid_header.clone(),
            data_grid_resize_handle: self.data_grid_resize_handle.clone(),
            splitter_handle: self.splitter_handle.clone(),
            carousel_auto_play: self.carousel_auto_play.clone(),
            child_source_spans,
            kind,
        }
    }
}
