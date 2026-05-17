use super::*;

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
            #[cfg(feature = "audio")]
            WidgetKind::Audio { audio } => WidgetKind::Audio { audio },
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

}
