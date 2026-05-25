use super::*;
use crate::ui::widget::r#virtual::{
    ItemLayout, VirtualArrangement, VirtualResolvedItemMeta, VirtualWindowPlan,
};
use crate::ui::widget::style::ContainerStyle;

pub(crate) struct LifecycleSnapshot {
    pub(crate) id: WidgetId,
    pub(crate) key: Option<WidgetKey>,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) kind: LifecycleWidgetKind,
}

pub(crate) struct LifecycleSelectOption {
    pub(crate) label: Value<String>,
    pub(crate) selected: Value<bool>,
    pub(crate) disabled: Value<bool>,
}

pub(crate) enum LifecycleWidgetKind {
    Container {
        layout: ContainerLayout,
        child_ids: Vec<WidgetId>,
    },
    Virtual {
        arrangement: VirtualArrangement,
        item_layout: ItemLayout,
        overflow_x: Overflow,
        overflow_y: Overflow,
        style: ContainerStyle,
        window_plan: VirtualWindowPlan,
        child_ids: Vec<WidgetId>,
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
        disabled: Value<bool>,
        style: WidgetCheckboxStyle,
    },
    Radio {
        checked: Value<bool>,
        label: Option<Value<String>>,
        disabled: Value<bool>,
        style: WidgetRadioStyle,
    },
    Switch {
        checked: Value<bool>,
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
        options: Vec<LifecycleSelectOption>,
        open: Option<Value<bool>>,
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
        disabled: Value<bool>,
        style: WidgetSliderStyle,
    },
    TextEditor {
        placeholder: Value<String>,
        disabled: Value<bool>,
        style: WidgetInputStyle,
        multiline: bool,
        show_scrollbar: Value<bool>,
        auto_wrap: Value<bool>,
    },
}

impl Clone for LifecycleSnapshot {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            key: self.key.clone(),
            layout: self.layout.clone(),
            visual: self.visual.clone(),
            background: self.background.clone(),
            kind: self.kind.clone(),
        }
    }
}

impl Clone for LifecycleSelectOption {
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            selected: self.selected.clone(),
            disabled: self.disabled.clone(),
        }
    }
}

impl Clone for LifecycleWidgetKind {
    fn clone(&self) -> Self {
        match self {
            Self::Container { layout, child_ids } => Self::Container {
                layout: layout.clone(),
                child_ids: child_ids.clone(),
            },
            Self::Virtual {
                arrangement,
                item_layout,
                overflow_x,
                overflow_y,
                style,
                window_plan,
                child_ids,
                child_meta,
            } => Self::Virtual {
                arrangement: *arrangement,
                item_layout: *item_layout,
                overflow_x: *overflow_x,
                overflow_y: *overflow_y,
                style: style.clone(),
                window_plan: window_plan.clone(),
                child_ids: child_ids.clone(),
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
            Self::Canvas { scene } => Self::Canvas {
                scene: scene.clone(),
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
                disabled,
                style,
            } => Self::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Radio {
                checked,
                label,
                disabled,
                style,
            } => Self::Radio {
                checked: checked.clone(),
                label: label.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Switch {
                checked,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                style,
            } => Self::Switch {
                checked: checked.clone(),
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
                disabled,
                style,
            } => Self::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
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
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::TextEditor {
                placeholder,
                disabled,
                style,
                multiline,
                show_scrollbar,
                auto_wrap,
            } => Self::TextEditor {
                placeholder: placeholder.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
                multiline: *multiline,
                show_scrollbar: show_scrollbar.clone(),
                auto_wrap: auto_wrap.clone(),
            },
        }
    }
}
