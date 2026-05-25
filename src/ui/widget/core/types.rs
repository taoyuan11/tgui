mod lifecycle;

pub(crate) use self::lifecycle::{LifecycleSelectOption, LifecycleSnapshot, LifecycleWidgetKind};
use super::*;
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
    pub(crate) tooltip: Option<crate::ui::widget::tooltip::Tooltip>,
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
    pub(crate) tooltip: Option<crate::ui::widget::tooltip::Tooltip>,
    pub(crate) child_source_spans: Vec<usize>,
    pub(crate) kind: ResolvedWidgetKind<VM>,
}

pub(crate) enum ResolvedWidgetKind<VM> {
    Container {
        layout: ContainerLayout,
        children: Vec<ResolvedElement<VM>>,
    },
    Virtual {
        arrangement: VirtualArrangement,
        item_layout: ItemLayout,
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
        style: WidgetButtonStyle,
    },
    Checkbox {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: WidgetCheckboxStyle,
    },
    Radio {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: WidgetRadioStyle,
    },
    Switch {
        checked: Value<bool>,
        on_change: Option<ValueCommand<VM, bool>>,
        active_background: Option<Value<Color>>,
        inactive_background: Option<Value<Color>>,
        active_thumb_color: Option<Value<Color>>,
        inactive_thumb_color: Option<Value<Color>>,
        disabled: Value<bool>,
        style: WidgetSwitchStyle,
    },
    Select {
        selected_label: Value<Option<String>>,
        placeholder: Value<String>,
        options: Vec<SelectOptionState<VM>>,
        open: Option<Value<bool>>,
        on_open_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: WidgetSelectStyle,
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
        style: WidgetSliderStyle,
    },
    TextEditor {
        controller: TextController,
        placeholder: Value<String>,
        on_change: Option<Command<VM>>,
        on_change_set: Option<ValueCommand<VM, TextChangeSet>>,
        disabled: Value<bool>,
        style: WidgetInputStyle,
        multiline: bool,
        show_scrollbar: Value<bool>,
        auto_wrap: Value<bool>,
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
                style,
            } => Self::Button {
                label: label.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Checkbox {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => Self::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Radio {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => Self::Radio {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Switch {
                checked,
                on_change,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                style,
            } => Self::Switch {
                checked: checked.clone(),
                on_change: on_change.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                style,
            } => Self::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
                on_open_change: on_open_change.clone(),
                disabled: disabled.clone(),
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
                style,
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
                style: style.clone(),
            },
            Self::TextEditor {
                controller,
                placeholder,
                on_change,
                on_change_set,
                disabled,
                style,
                multiline,
                show_scrollbar,
                auto_wrap,
            } => Self::TextEditor {
                controller: controller.clone(),
                placeholder: placeholder.clone(),
                on_change: on_change.clone(),
                on_change_set: on_change_set.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
                multiline: *multiline,
                show_scrollbar: show_scrollbar.clone(),
                auto_wrap: auto_wrap.clone(),
            },
        }
    }
}
