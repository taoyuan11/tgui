use super::element_path::resolved_child_elements_with_previous;
use super::*;
use crate::ui::widget::r#virtual::{
    resolve_virtual_window_plan, VirtualCacheState, VirtualResolvedItemMeta, VirtualViewportHint,
};

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
        super::tree::with_widget_stack_frame(|| {
            self.resolve_with_previous_inner(
                theme,
                previous,
                scroll_offsets,
                virtual_states,
                fallback_viewport_hint,
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
                let resolved_style = resolved_container_style(style.as_ref(), theme);
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
                    child.resolve_with_previous_and_runtime_state(
                        theme,
                        previous_child,
                        scroll_offsets,
                        virtual_states,
                        fallback_viewport_hint.clone(),
                    )
                })
                .collect();
                ResolvedWidgetKind::Container {
                    layout,
                    children: resolved_children,
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
                let resolved_style = resolved_container_style(style.as_ref(), theme);
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
                    let Some(mut child) = source.build(placement.item_index) else {
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
                    children.push(child.resolve_with_previous_and_runtime_state(
                        theme,
                        previous_child,
                        scroll_offsets,
                        virtual_states,
                        fallback_viewport_hint.clone(),
                    ));
                }
                ResolvedWidgetKind::Virtual {
                    arrangement: *arrangement,
                    item_layout: *item_layout,
                    content_cross_extent: content_cross_extent.clone(),
                    overflow_x: *overflow_x,
                    overflow_y: *overflow_y,
                    style: resolved_style,
                    runtime_state,
                    window_plan,
                    children,
                    child_meta,
                }
            }
            WidgetKind::Text { text } => {
                let mut text = text.clone();
                let resolved_style = resolved_text_widget_style(text.style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                apply_text_widget_style(&mut text, &resolved_style);
                ResolvedWidgetKind::Text { text }
            }
            #[cfg(feature = "audio")]
            WidgetKind::Audio { audio } => ResolvedWidgetKind::Audio {
                audio: audio.clone(),
            },
            WidgetKind::Image { image } => {
                let mut image = image.clone();
                let resolved_style = resolved_image_style(image.style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                image.background = resolved_style.surface.background.clone();
                image.fit = resolved_style.fit;
                ResolvedWidgetKind::Image { image }
            }
            WidgetKind::Canvas {
                scene,
                item_interactions,
                style,
            } => {
                let resolved_style = resolved_canvas_style(style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                ResolvedWidgetKind::Canvas {
                    scene: scene.clone(),
                    item_interactions: item_interactions.clone(),
                }
            }
            #[cfg(feature = "video")]
            WidgetKind::VideoSurface { video, style } => {
                let mut video = video.clone();
                let resolved_style = resolved_video_surface_style(style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                video.background = resolved_style.surface.background.clone();
                video.fit = resolved_style.fit;
                ResolvedWidgetKind::VideoSurface {
                    video,
                    style: resolved_style,
                }
            }
            WidgetKind::Button {
                label,
                disabled,
                variant,
                style,
            } => ResolvedWidgetKind::Button {
                label: label.clone(),
                disabled: disabled.clone(),
                style: resolved_button_style(style.as_ref(), theme, *variant),
            },
            WidgetKind::Checkbox {
                checked,
                label,
                on_change,
                disabled,
                validation,
                style,
            } => ResolvedWidgetKind::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: resolved_checkbox_style(style.as_ref(), theme),
            },
            WidgetKind::Radio {
                checked,
                label,
                on_change,
                disabled,
                validation,
                style,
            } => ResolvedWidgetKind::Radio {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: resolved_radio_style(style.as_ref(), theme),
            },
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
            } => ResolvedWidgetKind::Switch {
                checked: checked.clone(),
                on_change: on_change.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: resolved_switch_style(style.as_ref(), theme),
            },
            WidgetKind::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                validation,
                style,
            } => ResolvedWidgetKind::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
                on_open_change: on_open_change.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: resolved_select_style(style.as_ref(), theme),
            },
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
            } => ResolvedWidgetKind::Slider {
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
                style: resolved_slider_style(style.as_ref(), theme),
            },
            WidgetKind::ProgressBar {
                value,
                indeterminate,
                show_label,
                label,
                style,
            } => {
                let resolved_style = resolved_progress_bar_style(style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                ResolvedWidgetKind::ProgressBar {
                    value: value.clone(),
                    indeterminate: indeterminate.clone(),
                    show_label: *show_label,
                    label: label.clone(),
                    style: resolved_style,
                }
            }
            WidgetKind::Spinner {
                style,
                size_override,
                thickness_override,
                track_override,
            } => {
                let resolved_style = resolved_spinner_style(style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                ResolvedWidgetKind::Spinner {
                    style: resolved_style,
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
                let resolved_style = resolved_divider_style(style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                ResolvedWidgetKind::Divider {
                    orientation: *orientation,
                    dashed: dashed.clone(),
                    color_override: color_override.clone(),
                    thickness_override: thickness_override.clone(),
                    inset_override: inset_override.clone(),
                    label: label.clone(),
                    style: resolved_style,
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
            } => ResolvedWidgetKind::TextEditor {
                controller: controller.clone(),
                placeholder: placeholder.clone(),
                on_change: on_change.clone(),
                on_change_set: on_change_set.clone(),
                disabled: disabled.clone(),
                style: if *multiline {
                    let style = resolved_textarea_style(textarea_style.as_ref(), theme);
                    crate::ui::widget::InputStyle {
                        surface: style.surface,
                        background: style.background,
                        text: style.text,
                        placeholder: style.placeholder,
                        border: style.border,
                        selection: style.selection,
                        caret: style.caret,
                        border_width: style.border_width,
                        radius: style.radius,
                        padding_x: style.padding_x,
                        padding_y: style.padding_y,
                        min_height: style.min_height,
                        text_style: style.text_style,
                    }
                } else {
                    resolved_input_style(input_style.as_ref(), theme)
                },
                multiline: *multiline,
                show_scrollbar: show_scrollbar.clone(),
                auto_wrap: auto_wrap.clone(),
                validation: validation.clone(),
            },
            WidgetKind::ToastHost {
                queue,
                placement,
                max_visible,
                style,
            } => ResolvedWidgetKind::ToastHost {
                queue: queue.clone(),
                placement: *placement,
                max_visible: *max_visible,
                style: style
                    .as_ref()
                    .map(|resolver| {
                        resolver.resolve(crate::ui::widget::style::infer_theme_mode(theme))
                    })
                    .unwrap_or_else(|| {
                        crate::ui::widget::style::ToastStyle::default_for(
                            crate::ui::widget::style::infer_theme_mode(theme),
                        )
                    }),
            },
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
            child_source_spans,
            kind,
        }
    }
}
