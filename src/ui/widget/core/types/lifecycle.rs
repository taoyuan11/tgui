use super::*;
use crate::ui::widget::r#virtual::{
    ItemLayout, VirtualArrangement, VirtualResolvedItemMeta, VirtualWindowPlan,
};
use crate::ui::widget::style::ContainerStyle;
use crate::ui::widget::SliderOrientation;

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
    Icon {
        source: crate::ui::widget::icon::SvgIconId,
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
        validation: Value<ValidationVisualState>,
        style: WidgetCheckboxStyle,
    },
    Radio {
        checked: Value<bool>,
        label: Option<Value<String>>,
        disabled: Value<bool>,
        validation: Value<ValidationVisualState>,
        style: WidgetRadioStyle,
    },
    Switch {
        checked: Value<bool>,
        active_background: Option<Value<Color>>,
        inactive_background: Option<Value<Color>>,
        active_thumb_color: Option<Value<Color>>,
        inactive_thumb_color: Option<Value<Color>>,
        disabled: Value<bool>,
        validation: Value<ValidationVisualState>,
        style: WidgetSwitchStyle,
    },
    Select {
        selected_label: Value<Option<String>>,
        placeholder: Value<String>,
        options: Vec<LifecycleSelectOption>,
        open: Option<Value<bool>>,
        disabled: Value<bool>,
        validation: Value<ValidationVisualState>,
        style: WidgetSelectStyle,
    },
    SelectOptionRow {
        owner_id: WidgetId,
        option_index: usize,
        option: LifecycleSelectOption,
        style: common::SelectOptionRowStyle,
    },
    Slider {
        value: Value<f32>,
        min: f32,
        max: f32,
        step: f32,
        orientation: SliderOrientation,
        show_ticks: bool,
        show_value_label: bool,
        tick_count: Option<usize>,
        value_formatter: Option<SliderValueFormatter>,
        disabled: Value<bool>,
        validation: Value<ValidationVisualState>,
        style: WidgetSliderStyle,
    },
    ProgressBar {
        value: Value<f32>,
        indeterminate: Value<bool>,
        show_label: bool,
        label: Option<Value<String>>,
        style: crate::ui::widget::style::ProgressBarStyle,
    },
    Spinner {
        style: crate::ui::widget::style::SpinnerStyle,
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
        style: crate::ui::widget::style::DividerStyle,
    },
    TextEditor {
        placeholder: Value<String>,
        disabled: Value<bool>,
        style: WidgetInputStyle,
        multiline: bool,
        show_scrollbar: Value<bool>,
        auto_wrap: Value<bool>,
        validation: Value<ValidationVisualState>,
    },
    ToastHost,
    Portal,
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
            Self::Icon { source } => Self::Icon { source: *source },
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
                validation,
                style,
            } => Self::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
            },
            Self::Radio {
                checked,
                label,
                disabled,
                validation,
                style,
            } => Self::Radio {
                checked: checked.clone(),
                label: label.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
            },
            Self::Switch {
                checked,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                validation,
                style,
            } => Self::Switch {
                checked: checked.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
            },
            Self::Select {
                selected_label,
                placeholder,
                options,
                open,
                disabled,
                validation,
                style,
            } => Self::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
            },
            Self::SelectOptionRow {
                owner_id,
                option_index,
                option,
                style,
            } => Self::SelectOptionRow {
                owner_id: *owner_id,
                option_index: *option_index,
                option: option.clone(),
                style: style.clone(),
            },
            Self::Slider {
                value,
                min,
                max,
                step,
                orientation,
                show_ticks,
                show_value_label,
                tick_count,
                value_formatter,
                disabled,
                validation,
                style,
            } => Self::Slider {
                value: value.clone(),
                min: *min,
                max: *max,
                step: *step,
                orientation: *orientation,
                show_ticks: *show_ticks,
                show_value_label: *show_value_label,
                tick_count: *tick_count,
                value_formatter: value_formatter.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
            },
            Self::ProgressBar {
                value,
                indeterminate,
                show_label,
                label,
                style,
            } => Self::ProgressBar {
                value: value.clone(),
                indeterminate: indeterminate.clone(),
                show_label: *show_label,
                label: label.clone(),
                style: style.clone(),
            },
            Self::Spinner {
                style,
                size_override,
                thickness_override,
                track_override,
            } => Self::Spinner {
                style: style.clone(),
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
            } => Self::Divider {
                orientation: *orientation,
                dashed: dashed.clone(),
                color_override: color_override.clone(),
                thickness_override: thickness_override.clone(),
                inset_override: inset_override.clone(),
                label: label.clone(),
                style: style.clone(),
            },
            Self::TextEditor {
                placeholder,
                disabled,
                style,
                multiline,
                show_scrollbar,
                auto_wrap,
                validation,
            } => Self::TextEditor {
                placeholder: placeholder.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
                multiline: *multiline,
                show_scrollbar: show_scrollbar.clone(),
                auto_wrap: auto_wrap.clone(),
                validation: validation.clone(),
            },
            Self::ToastHost => Self::ToastHost,
            Self::Portal => Self::Portal,
        }
    }
}
