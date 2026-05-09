use super::*;

impl<VM> Element<VM> {
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
                items,
                item_interactions,
                style,
            } => WidgetKind::Canvas {
                items,
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
            layout: self.layout,
            visual: self.visual,
            interactions: self.interactions.scope(selector.clone()),
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
        let layout = self.layout.clone();
        let mut visual = self.visual.clone();
        let mut background = self.background.clone();
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
                ResolvedWidgetKind::Container {
                    layout,
                    children: children
                        .iter()
                        .flat_map(|child| child.resolve(Some(self.id)))
                        .map(|child| child.resolve(theme))
                        .collect(),
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
                items,
                item_interactions,
                style,
            } => {
                let resolved_style = resolved_canvas_style(style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                ResolvedWidgetKind::Canvas {
                    items: items.clone(),
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
            id: self.id,
            layout,
            visual,
            interactions: self.interactions.clone(),
            media_events: self.media_events.clone(),
            background,
            kind,
        }
    }
}
