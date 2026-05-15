use super::*;
use crate::log::{log_text_profile, text_profile_enabled};
use crate::ui::widget::common::ChildSource;
use std::time::Instant;

impl<VM> Element<VM> {
    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Adapts an element tree built for a child view model so it can be mounted
    /// inside a root view model tree.
    ///
    /// Commands stored anywhere inside the scoped subtree are executed against
    /// the child view model returned by `selector`.
    pub fn scope<RootVm: 'static>(
        self,
        selector: impl for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync + 'static,
    ) -> Element<RootVm>
    where
        VM: 'static,
    {
        self.scope_with_selector(Arc::new(selector))
    }

    pub(crate) fn scope_with_selector<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> Element<RootVm>
    where
        VM: 'static,
    {
        let kind = match self.kind {
            WidgetKind::Container {
                layout,
                children,
                style,
            } => WidgetKind::Container {
                layout,
                children: children
                    .into_iter()
                    .map(|child| child.scope(selector.clone()))
                    .collect(),
                style,
            },
            WidgetKind::Text { text } => WidgetKind::Text { text },
            WidgetKind::Image { image } => WidgetKind::Image { image },
            WidgetKind::Canvas {
                scene,
                item_interactions,
                style,
            } => WidgetKind::Canvas {
                scene,
                item_interactions: item_interactions.scope(selector.clone()),
                style,
            },
            #[cfg(feature = "video")]
            WidgetKind::VideoSurface { video, style } => WidgetKind::VideoSurface { video, style },
            WidgetKind::Button {
                label,
                disabled,
                variant,
                style,
            } => WidgetKind::Button {
                label,
                disabled,
                variant,
                style,
            },
            WidgetKind::Checkbox {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => WidgetKind::Checkbox {
                checked,
                label,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                disabled,
                style,
            },
            WidgetKind::Radio {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => WidgetKind::Radio {
                checked,
                label,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                disabled,
                style,
            },
            WidgetKind::Switch {
                checked,
                on_change,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                style,
            } => WidgetKind::Switch {
                checked,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                style,
            },
            WidgetKind::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                style,
            } => WidgetKind::Select {
                selected_label,
                placeholder,
                options: options
                    .into_iter()
                    .map(|option| SelectOptionState {
                        label: option.label,
                        selected: option.selected,
                        disabled: option.disabled,
                        on_select: option
                            .on_select
                            .map(|command| command.scope(selector.clone())),
                    })
                    .collect(),
                open,
                on_open_change: on_open_change.map(|command| command.scope(selector.clone())),
                disabled,
                style,
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
                style,
            } => WidgetKind::Slider {
                value,
                min,
                max,
                step,
                show_ticks,
                show_value_label,
                tick_count,
                value_formatter,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                disabled,
                style,
            },
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
            } => WidgetKind::TextEditor {
                controller,
                placeholder,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                on_change_set: on_change_set.map(|command| command.scope(selector.clone())),
                disabled,
                input_style,
                textarea_style,
                multiline,
                show_scrollbar,
                auto_wrap,
            },
        };

        Element {
            id: self.id,
            key: self.key,
            layout: self.layout,
            visual: self.visual,
            interactions: self.interactions.scope(selector.clone()),
            lifecycle_events: self.lifecycle_events.scope(selector.clone()),
            media_events: self.media_events.scope(selector),
            background: self.background,
            kind,
        }
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.interactions.on_click = Some(command);
        self
    }

    pub fn on_double_click(mut self, command: Command<VM>) -> Self {
        self.interactions.on_double_click = Some(command);
        self
    }

    pub fn on_mouse_enter(mut self, command: Command<VM>) -> Self {
        self.interactions.on_mouse_enter = Some(command);
        self
    }

    pub fn on_mouse_leave(mut self, command: Command<VM>) -> Self {
        self.interactions.on_mouse_leave = Some(command);
        self
    }

    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.interactions.on_mouse_move = Some(command);
        self
    }

    pub fn on_mount(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_mount = Some(command);
        self
    }

    pub fn on_unmount(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_unmount = Some(command);
        self
    }

    pub fn on_update(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_update = Some(command);
        self
    }

    pub fn on_loading(mut self, command: Command<VM>) -> Self {
        self.media_events.on_loading = Some(command);
        self
    }

    pub fn on_success(mut self, command: Command<VM>) -> Self {
        self.media_events.on_success = Some(command);
        self
    }

    pub fn on_error(mut self, command: ValueCommand<VM, String>) -> Self {
        self.media_events.on_error = Some(command);
        self
    }

    pub(super) fn resolve(&self, theme: &Theme) -> ResolvedElement<VM> {
        self.resolve_with_previous(theme, None)
    }

    pub(super) fn resolve_with_previous(
        &self,
        theme: &Theme,
        previous: Option<&ResolvedElement<VM>>,
    ) -> ResolvedElement<VM> {
        let mut source = self.clone();
        if let Some(previous) = previous {
            source.id = previous.id;
        }

        let layout = source.layout.clone();
        let mut visual = source.visual.clone();
        let mut background = source.background.clone();
        let mut child_source_spans = Vec::new();
        let kind = match &source.kind {
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
                    source.id,
                    children,
                    previous_children,
                    Some(&mut child_source_spans),
                )
                .into_iter()
                .map(|(child, previous_child)| child.resolve_with_previous(theme, previous_child))
                .collect();
                ResolvedWidgetKind::Container {
                    layout,
                    children: resolved_children,
                }
            }
            WidgetKind::Text { text } => {
                let mut text = text.clone();
                let resolved_style = resolved_text_widget_style(text.style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                apply_text_widget_style(&mut text, &resolved_style);
                ResolvedWidgetKind::Text { text }
            }
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
                style,
            } => ResolvedWidgetKind::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: resolved_checkbox_style(style.as_ref(), theme),
            },
            WidgetKind::Radio {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => ResolvedWidgetKind::Radio {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
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
                style,
            } => ResolvedWidgetKind::Switch {
                checked: checked.clone(),
                on_change: on_change.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
                style: resolved_switch_style(style.as_ref(), theme),
            },
            WidgetKind::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                style,
            } => ResolvedWidgetKind::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
                on_open_change: on_open_change.clone(),
                disabled: disabled.clone(),
                style: resolved_select_style(style.as_ref(), theme),
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
                style: resolved_slider_style(style.as_ref(), theme),
            },
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
            },
        };

        ResolvedElement {
            id: source.id,
            key: source.key.clone(),
            layout,
            visual,
            interactions: source.interactions.clone(),
            lifecycle_events: source.lifecycle_events.clone(),
            media_events: source.media_events.clone(),
            background,
            child_source_spans,
            kind,
        }
    }
}

fn resolved_child_elements_with_previous<'a, VM>(
    owner_id: WidgetId,
    child_sources: &[ChildSource<VM>],
    previous_children: &'a [ResolvedElement<VM>],
    child_source_spans: Option<&mut Vec<usize>>,
) -> Vec<(Element<VM>, Option<&'a ResolvedElement<VM>>)> {
    let previous_by_key: HashMap<_, _> = previous_children
        .iter()
        .filter_map(|child| child.key.as_ref().map(|key| (key.clone(), child)))
        .collect();
    let previous_by_id: HashMap<_, _> = previous_children
        .iter()
        .map(|child| (child.id, child))
        .collect();

    let mut resolved = Vec::new();
    let mut spans = child_source_spans;
    for child_source in child_sources {
        let source_children = child_source.resolve(Some(owner_id));
        if let Some(spans) = spans.as_mut() {
            spans.push(source_children.len());
        }
        resolved.extend(source_children.into_iter().map(|mut child| {
            let previous_child = child
                .key
                .as_ref()
                .and_then(|key| previous_by_key.get(key).copied())
                .or_else(|| previous_by_id.get(&child.id).copied());
            if let Some(previous_child) = previous_child {
                child.id = previous_child.id;
            }
            (child, previous_child)
        }));
    }
    resolved
}

pub(super) fn resolve_subtree_from_source_path<'a, VM>(
    source: &Element<VM>,
    previous: Option<&'a ResolvedElement<VM>>,
    theme: &Theme,
    path: &[usize],
) -> Option<ResolvedElement<VM>> {
    let started_at = text_profile_enabled().then_some(Instant::now());
    if path.is_empty() {
        let resolved = source.resolve_with_previous(theme, previous);
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_patch_scene_resolve_roots",
                started_at.elapsed(),
                format!("path={:?} terminal=true widget_id={:?}", path, resolved.id),
            );
        }
        return Some(resolved);
    }

    let WidgetKind::Container { children, .. } = &source.kind else {
        return None;
    };
    let previous_children = previous
        .and_then(|previous| match &previous.kind {
            ResolvedWidgetKind::Container { children, .. } => Some(children.as_slice()),
            _ => None,
        })
        .unwrap_or(&[]);
    let owner_id = previous.map(|previous| previous.id).unwrap_or(source.id);
    let (source_index, local_index) = previous
        .and_then(|previous| previous.child_source_spans.get(..children.len()))
        .and_then(|spans| child_source_position(spans, path[0]))
        .or_else(|| child_source_position_from_source(children, owner_id, path[0]))?;
    let source_children = children.get(source_index)?.resolve(Some(owner_id));
    let resolved_children_len = source_children.len();
    let mut child = source_children.into_iter().nth(local_index)?;
    let previous_child = previous_children.get(path[0]);
    if let Some(previous_child) = previous_child {
        child.id = previous_child.id;
    }
    let resolved = resolve_subtree_from_source_path(&child, previous_child, theme, &path[1..]);
    if let Some(started_at) = started_at {
        log_text_profile(
            "textarea_patch_scene_resolve_roots",
            started_at.elapsed(),
            format!(
                "path={:?} owner_id={:?} source_index={} local_index={} resolved_children={} previous_children={}",
                path,
                owner_id,
                source_index,
                local_index,
                resolved_children_len,
                previous_children.len(),
            ),
        );
    }
    resolved
}

fn child_source_position(spans: &[usize], child_index: usize) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    for (source_index, span) in spans.iter().copied().enumerate() {
        if child_index < offset + span {
            return Some((source_index, child_index - offset));
        }
        offset += span;
    }
    None
}

fn child_source_position_from_source<VM>(
    child_sources: &[ChildSource<VM>],
    owner_id: WidgetId,
    child_index: usize,
) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    for (source_index, child_source) in child_sources.iter().enumerate() {
        let span = child_source.resolve(Some(owner_id)).len();
        if child_index < offset + span {
            return Some((source_index, child_index - offset));
        }
        offset += span;
    }
    None
}
