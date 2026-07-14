use super::*;

pub trait WidgetStyleExt<VM>: Into<Element<VM>> + Sized {
    fn class(self, class: impl Into<String>) -> Element<VM> {
        self.into().class(class)
    }

    fn classes<I, S>(self, classes: I) -> Element<VM>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.into().classes(classes)
    }

    fn style_id(self, style_id: impl Into<String>) -> Element<VM> {
        self.into().style_id(style_id)
    }
}

impl<VM, T> WidgetStyleExt<VM> for T where T: Into<Element<VM>> {}

impl<VM> Element<VM> {
    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.visual.classes.push(class.into());
        self
    }

    pub fn classes<I, S>(mut self, classes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.visual
            .classes
            .extend(classes.into_iter().map(Into::into));
        self
    }

    pub fn style_id(mut self, style_id: impl Into<String>) -> Self {
        self.visual.style_id = Some(style_id.into());
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
                runtime_layout,
            } => WidgetKind::Container {
                layout,
                children: children
                    .into_iter()
                    .map(|child| child.scope(selector.clone()))
                    .collect(),
                style,
                runtime_layout,
            },
            WidgetKind::Virtual {
                arrangement,
                item_layout,
                source,
                content_cross_extent,
                overflow_x,
                overflow_y,
                style,
                runtime_layout,
                runtime_state,
            } => WidgetKind::Virtual {
                arrangement,
                item_layout,
                source: source.scope(selector.clone()),
                content_cross_extent,
                overflow_x,
                overflow_y,
                style,
                runtime_layout,
                runtime_state,
            },
            WidgetKind::Text { text } => WidgetKind::Text { text },
            #[cfg(feature = "audio")]
            WidgetKind::Audio { audio } => WidgetKind::Audio { audio },
            WidgetKind::Image { image } => WidgetKind::Image { image },
            WidgetKind::Icon { icon } => WidgetKind::Icon { icon },
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
                runtime_layout,
            } => WidgetKind::Button {
                label,
                disabled,
                variant,
                style,
                runtime_layout,
            },
            WidgetKind::Checkbox {
                checked,
                label,
                on_change,
                disabled,
                validation,
                style,
            } => WidgetKind::Checkbox {
                checked,
                label,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                disabled,
                validation,
                style,
            },
            WidgetKind::Radio {
                checked,
                label,
                on_change,
                disabled,
                validation,
                style,
            } => WidgetKind::Radio {
                checked,
                label,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                disabled,
                validation,
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
                validation,
                style,
            } => WidgetKind::Switch {
                checked,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                validation,
                style,
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
                validation,
                style,
            },
            WidgetKind::SelectOptionRow {
                owner_id,
                option_index,
                option,
                on_open_change,
                style,
            } => WidgetKind::SelectOptionRow {
                owner_id,
                option_index,
                option: SelectOptionState {
                    label: option.label,
                    selected: option.selected,
                    disabled: option.disabled,
                    on_select: option
                        .on_select
                        .map(|command| command.scope(selector.clone())),
                },
                on_open_change: on_open_change.map(|command| command.scope(selector.clone())),
                style,
            },
            WidgetKind::Slider {
                value,
                min,
                max,
                step,
                orientation,
                show_ticks,
                show_value_label,
                tick_count,
                value_formatter,
                on_change,
                on_change_end,
                disabled,
                validation,
                style,
                runtime_layout,
            } => WidgetKind::Slider {
                value,
                min,
                max,
                step,
                orientation,
                show_ticks,
                show_value_label,
                tick_count,
                value_formatter,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                on_change_end: on_change_end.map(|command| command.scope(selector.clone())),
                disabled,
                validation,
                style,
                runtime_layout,
            },
            WidgetKind::ProgressBar {
                value,
                indeterminate,
                show_label,
                label,
                style,
            } => WidgetKind::ProgressBar {
                value,
                indeterminate,
                show_label,
                label,
                style,
            },
            WidgetKind::Divider {
                orientation,
                dashed,
                color_override,
                thickness_override,
                inset_override,
                label,
                style,
            } => WidgetKind::Divider {
                orientation,
                dashed,
                color_override,
                thickness_override,
                inset_override,
                label,
                style,
            },
            WidgetKind::Spinner {
                style,
                size_override,
                thickness_override,
                track_override,
            } => WidgetKind::Spinner {
                style,
                size_override,
                thickness_override,
                track_override,
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
                validation,
                runtime_layout,
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
                validation,
                runtime_layout,
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
            } => WidgetKind::Portal {
                content: Box::new(content.scope_with_selector(selector.clone())),
                open,
                target,
                anchor,
                options,
                layer,
                on_open_change: on_open_change.map(|command| command.scope(selector.clone())),
                return_focus_to,
                close_on_outside_click,
                close_on_escape,
                focus_scope,
            },
            WidgetKind::ToastHost { .. } => WidgetKind::Container {
                layout: ContainerLayout::flow(),
                children: Vec::new(),
                style: None,
                runtime_layout: None,
            },
        };

        Element {
            id: self.id,
            key: self.key,
            layout: self.layout,
            focus: self.focus,
            visual: self.visual,
            interactions: self.interactions.scope(selector.clone()),
            lifecycle_events: self.lifecycle_events.scope(selector.clone()),
            media_events: self.media_events.scope(selector.clone()),
            background: self.background,
            tooltip: self
                .tooltip
                .map(|tooltip| Box::new((*tooltip).scope(selector.clone()))),
            popover: self
                .popover
                .map(|popover| Box::new((*popover).scope(selector.clone()))),
            menu: self
                .menu
                .map(|menu| Box::new((*menu).scope(selector.clone()))),
            context_menu: self
                .context_menu
                .map(|menu| Box::new((*menu).scope(selector.clone()))),
            modal: self
                .modal
                .map(|modal| Box::new((*modal).scope(selector.clone()))),
            drawer: self
                .drawer
                .map(|drawer| Box::new((*drawer).scope(selector.clone()))),
            tab_trigger: self
                .tab_trigger
                .map(|trigger| trigger.scope(selector.clone())),
            list_item: self
                .list_item
                .map(|list_item| list_item.scope(selector.clone())),
            tree_root: self.tree_root,
            tree_node: self.tree_node.map(|state| state.scope(selector.clone())),
            data_grid_root: self.data_grid_root,
            data_grid_cell: self
                .data_grid_cell
                .map(|state| state.scope(selector.clone())),
            data_grid_header: self
                .data_grid_header
                .map(|state| state.scope(selector.clone())),
            data_grid_resize_handle: self
                .data_grid_resize_handle
                .map(|state| state.scope(selector.clone())),
            splitter_handle: self
                .splitter_handle
                .map(|state| state.scope(selector.clone())),
            carousel_auto_play: self.carousel_auto_play.map(|state| state.scope(selector)),
            kind,
        }
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.interactions.on_click = Some(command);
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focus.focusable = Some(focusable);
        self
    }

    pub fn tab_index(mut self, tab_index: i32) -> Self {
        self.focus.tab_index = Some(tab_index);
        self
    }

    pub fn focus_scope(mut self, options: crate::ui::widget::FocusScopeOptions) -> Self {
        self.focus.scope = Some(options);
        self
    }

    pub fn auto_focus_first(mut self, auto_focus_first: bool) -> Self {
        self.focus.scope = Some(
            self.focus
                .scope
                .take()
                .unwrap_or_default()
                .auto_focus_first(auto_focus_first),
        );
        self
    }

    /// 给 Element 挂上 Tooltip。任何 widget 通过 `.into()` 转 Element 后都可以链式调用。
    /// 各 widget 的 builder 通常也会暴露同名 `.tooltip()` 方法，调用本方法的简写形式。
    pub fn with_tooltip(mut self, tooltip: crate::ui::widget::Tooltip<VM>) -> Self {
        self.tooltip = Some(Box::new(tooltip));
        self
    }

    pub(crate) fn with_tab_trigger_state(
        mut self,
        trigger: crate::ui::widget::common::TabTriggerState<VM>,
    ) -> Self {
        self.tab_trigger = Some(trigger);
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

    pub fn on_file_drop(mut self, command: ValueCommand<VM, FileDropEvent>) -> Self {
        self.interactions.on_file_drop = Some(command);
        self
    }

    pub fn gesture(mut self, recognizer: crate::ui::widget::GestureRecognizer<VM>) -> Self {
        self.interactions.gesture = recognizer.has_any().then_some(recognizer);
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
