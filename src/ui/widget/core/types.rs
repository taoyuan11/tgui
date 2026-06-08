mod lifecycle;

pub(crate) use self::lifecycle::{LifecycleSelectOption, LifecycleSnapshot, LifecycleWidgetKind};
use super::*;
use crate::foundation::form::ValidationVisualState;
use crate::ui::widget::r#virtual::{
    ItemLayout, VirtualArrangement, VirtualResolvedItemMeta, VirtualRuntimeState, VirtualWindowPlan,
};
use crate::ui::widget::style::ContainerStyle;
use crate::ui::widget::{common, image};

pub struct Element<VM> {
    pub(crate) id: WidgetId,
    pub(crate) key: Option<WidgetKey>,
    pub(crate) layout: LayoutStyle,
    pub(crate) focus: FocusState,
    pub(crate) visual: VisualStyle,
    pub(crate) interactions: InteractionHandlers<VM>,
    pub(crate) lifecycle_events: LifecycleEventHandlers<VM>,
    pub(crate) media_events: MediaEventHandlers<VM>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) tooltip: Option<Box<crate::ui::widget::tooltip::Tooltip<VM>>>,
    pub(crate) popover: Option<Box<crate::ui::widget::popover::PopoverDescriptor<VM>>>,
    pub(crate) menu: Option<Box<crate::ui::widget::menu::MenuDescriptor<VM>>>,
    pub(crate) context_menu: Option<Box<crate::ui::widget::menu::ContextMenuDescriptor<VM>>>,
    pub(crate) modal: Option<Box<crate::ui::widget::modal::ModalDescriptor<VM>>>,
    pub(crate) drawer: Option<Box<crate::ui::widget::drawer::DrawerDescriptor<VM>>>,
    pub(crate) tab_trigger: Option<common::TabTriggerState<VM>>,
    pub(crate) list_item: Option<common::ListItemState<VM>>,
    pub(crate) tree_root: Option<common::TreeRootState>,
    pub(crate) tree_node: Option<common::TreeNodeState<VM>>,
    pub(crate) data_grid_root: Option<common::DataGridRootState>,
    pub(crate) data_grid_cell: Option<common::DataGridCellState<VM>>,
    pub(crate) data_grid_header: Option<common::DataGridHeaderState<VM>>,
    pub(crate) data_grid_resize_handle: Option<common::DataGridResizeHandleState<VM>>,
    pub(crate) splitter_handle: Option<common::SplitterHandleState<VM>>,
    pub(crate) carousel_auto_play: Option<common::CarouselAutoPlayState<VM>>,
    pub(crate) kind: WidgetKind<VM>,
}

impl<VM> Clone for Element<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            key: self.key.clone(),
            layout: self.layout.clone(),
            focus: self.focus.clone(),
            visual: self.visual.clone(),
            interactions: self.interactions.clone(),
            lifecycle_events: self.lifecycle_events.clone(),
            media_events: self.media_events.clone(),
            background: self.background.clone(),
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
            kind: self.kind.clone(),
        }
    }
}

pub(crate) struct ResolvedElement<VM> {
    pub(crate) id: WidgetId,
    pub(crate) key: Option<WidgetKey>,
    pub(crate) layout: LayoutStyle,
    pub(crate) focus: FocusState,
    pub(crate) visual: VisualStyle,
    pub(crate) interactions: InteractionHandlers<VM>,
    pub(crate) lifecycle_events: LifecycleEventHandlers<VM>,
    pub(crate) media_events: MediaEventHandlers<VM>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) tooltip: Option<Box<crate::ui::widget::tooltip::Tooltip<VM>>>,
    pub(crate) popover: Option<Box<crate::ui::widget::popover::PopoverDescriptor<VM>>>,
    pub(crate) menu: Option<Box<crate::ui::widget::menu::MenuDescriptor<VM>>>,
    pub(crate) context_menu: Option<Box<crate::ui::widget::menu::ContextMenuDescriptor<VM>>>,
    pub(crate) modal: Option<Box<crate::ui::widget::modal::ModalDescriptor<VM>>>,
    pub(crate) drawer: Option<Box<crate::ui::widget::drawer::DrawerDescriptor<VM>>>,
    pub(crate) tab_trigger: Option<common::TabTriggerState<VM>>,
    pub(crate) list_item: Option<common::ListItemState<VM>>,
    pub(crate) tree_root: Option<common::TreeRootState>,
    pub(crate) tree_node: Option<common::TreeNodeState<VM>>,
    pub(crate) data_grid_root: Option<common::DataGridRootState>,
    pub(crate) data_grid_cell: Option<common::DataGridCellState<VM>>,
    pub(crate) data_grid_header: Option<common::DataGridHeaderState<VM>>,
    pub(crate) data_grid_resize_handle: Option<common::DataGridResizeHandleState<VM>>,
    pub(crate) splitter_handle: Option<common::SplitterHandleState<VM>>,
    pub(crate) carousel_auto_play: Option<common::CarouselAutoPlayState<VM>>,
    pub(crate) child_source_spans: Vec<usize>,
    pub(crate) kind: ResolvedWidgetKind<VM>,
}

impl<VM> ResolvedElement<VM> {
    pub(crate) fn contains_virtual(&self) -> bool {
        match &self.kind {
            ResolvedWidgetKind::Virtual { .. } => true,
            ResolvedWidgetKind::Container { children, .. } => {
                children.iter().any(ResolvedElement::contains_virtual)
            }
            _ => false,
        }
    }

    /// 递归估算子树中的节点总数。
    /// 在 `collect_scene_cache` 开始时调用，用于预分配 HashMap 容量，
    /// 避免 `HashMap::new()` 的多次 reallocation。
    pub(crate) fn estimated_node_count(&self) -> usize {
        match &self.kind {
            ResolvedWidgetKind::Container { children, .. }
            | ResolvedWidgetKind::Virtual { children, .. } => {
                1 + children
                    .iter()
                    .map(|c| c.estimated_node_count())
                    .sum::<usize>()
            }
            _ => 1,
        }
    }
}

pub(crate) enum ResolvedWidgetKind<VM> {
    Container {
        layout: ContainerLayout,
        children: Vec<ResolvedElement<VM>>,
    },
    Virtual {
        arrangement: VirtualArrangement,
        item_layout: ItemLayout,
        content_cross_extent: Option<Value<Dp>>,
        overflow_x: Overflow,
        overflow_y: Overflow,
        style: ContainerStyle,
        runtime_state: VirtualRuntimeState,
        window_plan: VirtualWindowPlan,
        children: Vec<ResolvedElement<VM>>,
        child_meta: Vec<VirtualResolvedItemMeta>,
    },
    Text {
        text: Text,
    },
    #[cfg(feature = "audio")]
    Audio {
        audio: PublicAudio,
    },
    Image {
        image: image::Image,
    },
    Canvas {
        scene: Value<CanvasScene>,
        item_interactions: common::CanvasItemInteractionHandlers<VM>,
    },
    #[cfg(feature = "video")]
    VideoSurface {
        video: PublicVideoSurface,
        style: WidgetVideoSurfaceStyle,
    },
    Button {
        label: Value<String>,
        disabled: Value<bool>,
        variant: common::ButtonVariantKind,
        style: WidgetButtonStyle,
        runtime_style: ResolvedRuntimeStyle<WidgetButtonStyle>,
    },
    Checkbox {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        validation: Value<ValidationVisualState>,
        style: WidgetCheckboxStyle,
        runtime_style: ResolvedRuntimeStyle<WidgetCheckboxStyle>,
    },
    Radio {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        validation: Value<ValidationVisualState>,
        style: WidgetRadioStyle,
        runtime_style: ResolvedRuntimeStyle<WidgetRadioStyle>,
    },
    Switch {
        checked: Value<bool>,
        on_change: Option<ValueCommand<VM, bool>>,
        active_background: Option<Value<Color>>,
        inactive_background: Option<Value<Color>>,
        active_thumb_color: Option<Value<Color>>,
        inactive_thumb_color: Option<Value<Color>>,
        disabled: Value<bool>,
        validation: Value<ValidationVisualState>,
        style: WidgetSwitchStyle,
        runtime_style: ResolvedRuntimeStyle<WidgetSwitchStyle>,
    },
    Select {
        selected_label: Value<Option<String>>,
        placeholder: Value<String>,
        options: Vec<SelectOptionState<VM>>,
        open: Option<Value<bool>>,
        on_open_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        validation: Value<ValidationVisualState>,
        style: WidgetSelectStyle,
        runtime_style: ResolvedRuntimeStyle<WidgetSelectStyle>,
    },
    SelectOptionRow {
        owner_id: WidgetId,
        option_index: usize,
        option: SelectOptionState<VM>,
        on_open_change: Option<ValueCommand<VM, bool>>,
        style: common::SelectOptionRowStyle,
    },
    Slider {
        value: Value<f32>,
        min: f32,
        max: f32,
        step: f32,
        show_ticks: bool,
        show_value_label: bool,
        tick_count: Option<usize>,
        value_formatter: Option<SliderValueFormatter>,
        on_change: Option<ValueCommand<VM, f32>>,
        disabled: Value<bool>,
        validation: Value<ValidationVisualState>,
        style: WidgetSliderStyle,
        runtime_style: ResolvedRuntimeStyle<WidgetSliderStyle>,
    },
    ProgressBar {
        value: Value<f32>,
        indeterminate: Value<bool>,
        show_label: bool,
        label: Option<Value<String>>,
        style: WidgetProgressBarStyle,
        runtime_style: ResolvedRuntimeStyle<WidgetProgressBarStyle>,
    },
    Spinner {
        style: WidgetSpinnerStyle,
        runtime_style: ResolvedRuntimeStyle<WidgetSpinnerStyle>,
        size_override: Option<Value<Dp>>,
        thickness_override: Option<Value<Dp>>,
        track_override: Option<bool>,
    },
    Divider {
        orientation: crate::ui::widget::common::DividerOrientation,
        dashed: Value<bool>,
        color_override: Option<Value<Color>>,
        thickness_override: Option<Value<Dp>>,
        inset_override: Option<Value<Dp>>,
        label: Option<Value<String>>,
        style: WidgetDividerStyle,
        runtime_style: ResolvedRuntimeStyle<WidgetDividerStyle>,
    },
    TextEditor {
        controller: TextController,
        placeholder: Value<String>,
        on_change: Option<Command<VM>>,
        on_change_set: Option<ValueCommand<VM, TextChangeSet>>,
        disabled: Value<bool>,
        style: WidgetInputStyle,
        runtime_style: ResolvedTextEditorRuntimeStyle,
        multiline: bool,
        show_scrollbar: Value<bool>,
        auto_wrap: Value<bool>,
        validation: Value<ValidationVisualState>,
    },
    ToastHost {
        queue: crate::foundation::binding::ToastQueue<VM>,
        placement: crate::foundation::binding::ToastPlacement,
        max_visible: Option<usize>,
        style: WidgetToastStyle,
    },
    Portal {
        content: Box<Element<VM>>,
        open: Value<bool>,
        target: crate::ui::widget::PortalTarget,
        anchor: Option<crate::ui::widget::PortalAnchor>,
        options: crate::ui::widget::OverlayPlacementOptions,
        layer: crate::ui::widget::OverlayLayer,
        on_open_change: Option<ValueCommand<VM, bool>>,
        return_focus_to: Option<WidgetId>,
        close_on_outside_click: bool,
        close_on_escape: bool,
        focus_scope: Option<crate::ui::widget::FocusScopeOptions>,
    },
}

impl<VM> Clone for ResolvedElement<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            key: self.key.clone(),
            layout: self.layout.clone(),
            focus: self.focus.clone(),
            visual: self.visual.clone(),
            interactions: self.interactions.clone(),
            lifecycle_events: self.lifecycle_events.clone(),
            media_events: self.media_events.clone(),
            background: self.background.clone(),
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
            child_source_spans: self.child_source_spans.clone(),
            kind: self.kind.clone(),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct FocusState {
    pub(crate) focusable: Option<bool>,
    pub(crate) tab_index: Option<i32>,
    pub(crate) scope: Option<crate::ui::widget::FocusScopeOptions>,
}

pub(crate) struct ResolvedRuntimeStyle<T> {
    pub(crate) base: T,
    pub(crate) local: Option<StyleResolver<T>>,
}

impl<T: Clone> Clone for ResolvedRuntimeStyle<T> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            local: self.local.clone(),
        }
    }
}

pub(crate) enum ResolvedTextEditorRuntimeStyle {
    Input(ResolvedRuntimeStyle<WidgetInputStyle>),
    Textarea(ResolvedRuntimeStyle<WidgetTextareaStyle>),
}

impl Clone for ResolvedTextEditorRuntimeStyle {
    fn clone(&self) -> Self {
        match self {
            Self::Input(style) => Self::Input(style.clone()),
            Self::Textarea(style) => Self::Textarea(style.clone()),
        }
    }
}

impl<VM> Clone for ResolvedWidgetKind<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Container { layout, children } => Self::Container {
                layout: layout.clone(),
                children: children.clone(),
            },
            Self::Virtual {
                arrangement,
                item_layout,
                content_cross_extent,
                overflow_x,
                overflow_y,
                style,
                runtime_state,
                window_plan,
                children,
                child_meta,
            } => Self::Virtual {
                arrangement: *arrangement,
                item_layout: *item_layout,
                content_cross_extent: content_cross_extent.clone(),
                overflow_x: *overflow_x,
                overflow_y: *overflow_y,
                style: style.clone(),
                runtime_state: runtime_state.clone(),
                window_plan: window_plan.clone(),
                children: children.clone(),
                child_meta: child_meta.clone(),
            },
            Self::Text { text } => Self::Text { text: text.clone() },
            #[cfg(feature = "audio")]
            Self::Audio { audio } => Self::Audio {
                audio: audio.clone(),
            },
            Self::Image { image } => Self::Image {
                image: image.clone(),
            },
            Self::Canvas {
                scene,
                item_interactions,
            } => Self::Canvas {
                scene: scene.clone(),
                item_interactions: item_interactions.clone(),
            },
            #[cfg(feature = "video")]
            Self::VideoSurface { video, style } => Self::VideoSurface {
                video: video.clone(),
                style: style.clone(),
            },
            Self::Button {
                label,
                disabled,
                variant,
                style,
                runtime_style,
            } => Self::Button {
                label: label.clone(),
                disabled: disabled.clone(),
                variant: *variant,
                style: style.clone(),
                runtime_style: runtime_style.clone(),
            },
            Self::Checkbox {
                checked,
                label,
                on_change,
                disabled,
                validation,
                style,
                runtime_style,
            } => Self::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
                runtime_style: runtime_style.clone(),
            },
            Self::Radio {
                checked,
                label,
                on_change,
                disabled,
                validation,
                style,
                runtime_style,
            } => Self::Radio {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
                runtime_style: runtime_style.clone(),
            },
            Self::Switch {
                checked,
                on_change,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                validation,
                style,
                runtime_style,
            } => Self::Switch {
                checked: checked.clone(),
                on_change: on_change.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
                runtime_style: runtime_style.clone(),
            },
            Self::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                validation,
                style,
                runtime_style,
            } => Self::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
                on_open_change: on_open_change.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
                runtime_style: runtime_style.clone(),
            },
            Self::SelectOptionRow {
                owner_id,
                option_index,
                option,
                on_open_change,
                style,
            } => Self::SelectOptionRow {
                owner_id: *owner_id,
                option_index: *option_index,
                option: option.clone(),
                on_open_change: on_open_change.clone(),
                style: style.clone(),
            },
            Self::Slider {
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
                runtime_style,
            } => Self::Slider {
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
                style: style.clone(),
                runtime_style: runtime_style.clone(),
            },
            Self::ProgressBar {
                value,
                indeterminate,
                show_label,
                label,
                style,
                runtime_style,
            } => Self::ProgressBar {
                value: value.clone(),
                indeterminate: indeterminate.clone(),
                show_label: *show_label,
                label: label.clone(),
                style: style.clone(),
                runtime_style: runtime_style.clone(),
            },
            Self::Spinner {
                style,
                runtime_style,
                size_override,
                thickness_override,
                track_override,
            } => Self::Spinner {
                style: style.clone(),
                runtime_style: runtime_style.clone(),
                size_override: size_override.clone(),
                thickness_override: thickness_override.clone(),
                track_override: *track_override,
            },
            Self::Divider {
                orientation,
                dashed,
                color_override,
                thickness_override,
                inset_override,
                label,
                style,
                runtime_style,
            } => Self::Divider {
                orientation: *orientation,
                dashed: dashed.clone(),
                color_override: color_override.clone(),
                thickness_override: thickness_override.clone(),
                inset_override: inset_override.clone(),
                label: label.clone(),
                style: style.clone(),
                runtime_style: runtime_style.clone(),
            },
            Self::TextEditor {
                controller,
                placeholder,
                on_change,
                on_change_set,
                disabled,
                style,
                runtime_style,
                multiline,
                show_scrollbar,
                auto_wrap,
                validation,
            } => Self::TextEditor {
                controller: controller.clone(),
                placeholder: placeholder.clone(),
                on_change: on_change.clone(),
                on_change_set: on_change_set.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
                runtime_style: runtime_style.clone(),
                multiline: *multiline,
                show_scrollbar: show_scrollbar.clone(),
                auto_wrap: auto_wrap.clone(),
                validation: validation.clone(),
            },
            Self::ToastHost {
                queue,
                placement,
                max_visible,
                style,
            } => Self::ToastHost {
                queue: queue.clone(),
                placement: *placement,
                max_visible: *max_visible,
                style: style.clone(),
            },
            Self::Portal {
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
            } => Self::Portal {
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
        }
    }
}
