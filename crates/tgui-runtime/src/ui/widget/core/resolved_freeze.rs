use super::*;
#[cfg(feature = "video")]
use crate::ui::widget::style::VideoSurfaceStyle;
use crate::ui::widget::style::{
    ButtonStyle as WidgetButtonStyle, CheckboxStyle as WidgetCheckboxStyle, ContainerStyle,
    DividerStyle as WidgetDividerStyle, InputStyle as WidgetInputStyle,
    ProgressBarStyle as WidgetProgressBarStyle, RadioStyle as WidgetRadioStyle,
    SelectStyle as WidgetSelectStyle, SliderStyle as WidgetSliderStyle,
    SpinnerStyle as WidgetSpinnerStyle, SwitchStyle as WidgetSwitchStyle, WidgetSurfaceStyle,
};
use crate::ui::widget::{Image, Text};

fn freeze_value<T: Clone>(value: &mut Value<T>) {
    let resolved = value.resolve_untracked();
    *value = Value::Static(resolved);
}

fn freeze_option_value<T: Clone>(value: &mut Option<Value<T>>) {
    if let Some(inner) = value.as_mut() {
        freeze_value(inner);
    }
}

fn freeze_stateful_value<T: Clone>(value: &mut crate::ui::theme::StateValue<Value<T>>) {
    freeze_value(&mut value.normal);
    freeze_value(&mut value.hovered);
    freeze_value(&mut value.pressed);
    freeze_value(&mut value.disabled);
    freeze_option_value(&mut value.focused);
    freeze_option_value(&mut value.focus_visible);
    freeze_option_value(&mut value.selected);
    freeze_option_value(&mut value.checked);
    freeze_option_value(&mut value.open);
    freeze_option_value(&mut value.invalid);
}

fn freeze_widget_surface_style(style: &mut WidgetSurfaceStyle) {
    freeze_option_value(&mut style.background);
    freeze_option_value(&mut style.background_brush);
    freeze_option_value(&mut style.background_image);
    freeze_value(&mut style.background_blur);
    freeze_option_value(&mut style.shadow);
    freeze_option_value(&mut style.border_color);
    freeze_option_value(&mut style.border_radius);
    freeze_option_value(&mut style.border_width);
    freeze_value(&mut style.opacity);
    freeze_value(&mut style.offset);
}

#[cfg(feature = "video")]
fn freeze_video_surface_style(style: &mut VideoSurfaceStyle) {
    freeze_widget_surface_style(&mut style.surface);
}

fn freeze_layout_style(style: &mut LayoutStyle) {
    freeze_option_value(&mut style.width);
    freeze_option_value(&mut style.height);
    freeze_option_value(&mut style.min_width);
    freeze_option_value(&mut style.min_height);
    freeze_option_value(&mut style.max_width);
    freeze_option_value(&mut style.max_height);
    freeze_option_value(&mut style.aspect_ratio);
    freeze_option_value(&mut style.padding);
    freeze_value(&mut style.margin);
    freeze_value(&mut style.grow);
    freeze_value(&mut style.shrink);
    freeze_option_value(&mut style.basis);
    freeze_option_value(&mut style.left);
    freeze_option_value(&mut style.top);
    freeze_option_value(&mut style.right);
    freeze_option_value(&mut style.bottom);
    freeze_option_value(&mut style.column_start);
    freeze_option_value(&mut style.row_start);
}

fn freeze_visual_style(style: &mut VisualStyle) {
    freeze_option_value(&mut style.border_color);
    freeze_option_value(&mut style.border_radius);
    freeze_option_value(&mut style.border_width);
    freeze_option_value(&mut style.background_brush);
    freeze_option_value(&mut style.background_image);
    freeze_value(&mut style.background_blur);
    freeze_option_value(&mut style.shadow);
    freeze_value(&mut style.opacity);
    freeze_value(&mut style.offset);
    freeze_value(&mut style.scale);
}

fn freeze_container_layout(layout: &mut ContainerLayout) {
    freeze_option_value(&mut layout.padding);
    freeze_value(&mut layout.gap);
}

fn freeze_container_style(style: &mut ContainerStyle) {
    freeze_widget_surface_style(&mut style.surface);
}

fn freeze_text(text: &mut Text) {
    freeze_value(&mut text.content);
    freeze_option_value(&mut text.background);
    freeze_option_value(&mut text.color);
    freeze_option_value(&mut text.cursor_style);
}

fn freeze_image(image: &mut Image) {
    freeze_value(&mut image.source);
    freeze_option_value(&mut image.background);
    freeze_option_value(&mut image.cursor_style);
}

#[cfg(feature = "audio")]
fn freeze_audio(audio: &mut crate::audio::Audio) {
    freeze_value(&mut audio.autoplay);
    freeze_value(&mut audio.looping);
}

fn freeze_button_style(style: &mut WidgetButtonStyle) {
    freeze_widget_surface_style(&mut style.surface);
    freeze_stateful_value(&mut style.background);
    freeze_stateful_value(&mut style.foreground);
    freeze_stateful_value(&mut style.border);
    freeze_value(&mut style.border_width);
    freeze_value(&mut style.radius);
}

fn freeze_checkbox_style(style: &mut WidgetCheckboxStyle) {
    freeze_widget_surface_style(&mut style.surface);
    freeze_stateful_value(&mut style.background);
    freeze_stateful_value(&mut style.background_checked);
    freeze_stateful_value(&mut style.border);
    freeze_stateful_value(&mut style.border_checked);
    freeze_stateful_value(&mut style.checkmark);
    freeze_stateful_value(&mut style.label);
    freeze_value(&mut style.border_width);
    freeze_value(&mut style.radius);
}

fn freeze_radio_style(style: &mut WidgetRadioStyle) {
    freeze_widget_surface_style(&mut style.surface);
    freeze_stateful_value(&mut style.background);
    freeze_stateful_value(&mut style.background_checked);
    freeze_stateful_value(&mut style.border);
    freeze_stateful_value(&mut style.border_checked);
    freeze_stateful_value(&mut style.indicator);
    freeze_stateful_value(&mut style.label);
    freeze_value(&mut style.border_width);
    freeze_value(&mut style.radius);
}

fn freeze_switch_style(style: &mut WidgetSwitchStyle) {
    freeze_widget_surface_style(&mut style.surface);
    freeze_stateful_value(&mut style.track);
    freeze_stateful_value(&mut style.track_checked);
    freeze_stateful_value(&mut style.thumb);
    freeze_stateful_value(&mut style.thumb_checked);
    freeze_stateful_value(&mut style.border);
    freeze_stateful_value(&mut style.border_checked);
    freeze_value(&mut style.border_width);
    freeze_value(&mut style.radius);
}

fn freeze_select_style(style: &mut WidgetSelectStyle) {
    freeze_widget_surface_style(&mut style.surface);
    freeze_stateful_value(&mut style.background);
    freeze_stateful_value(&mut style.text);
    freeze_stateful_value(&mut style.placeholder);
    freeze_stateful_value(&mut style.border);
    freeze_stateful_value(&mut style.arrow);
    freeze_value(&mut style.menu_background);
    freeze_value(&mut style.menu_border);
    freeze_value(&mut style.menu_border_width);
    freeze_value(&mut style.menu_radius);
    freeze_stateful_value(&mut style.option_background);
    freeze_value(&mut style.selected_option_background);
    freeze_value(&mut style.border_width);
    freeze_value(&mut style.radius);
}

fn freeze_slider_style(style: &mut WidgetSliderStyle) {
    freeze_widget_surface_style(&mut style.surface);
    freeze_stateful_value(&mut style.track);
    freeze_stateful_value(&mut style.active_track);
    freeze_stateful_value(&mut style.thumb);
    freeze_stateful_value(&mut style.tick);
    freeze_stateful_value(&mut style.label);
    if let Some(shadow) = style.thumb_shadow.as_mut() {
        *shadow = shadow.clone();
    }
    freeze_value(&mut style.radius);
    freeze_value(&mut style.border_width);
}

fn freeze_input_style(style: &mut WidgetInputStyle) {
    freeze_widget_surface_style(&mut style.surface);
    freeze_stateful_value(&mut style.background);
    freeze_stateful_value(&mut style.text);
    freeze_stateful_value(&mut style.placeholder);
    freeze_stateful_value(&mut style.border);
    freeze_option_value(&mut style.selection);
    freeze_option_value(&mut style.caret);
    freeze_value(&mut style.border_width);
    freeze_value(&mut style.radius);
}

fn freeze_progress_bar_style(style: &mut WidgetProgressBarStyle) {
    freeze_widget_surface_style(&mut style.surface);
    freeze_value(&mut style.track_color);
    freeze_value(&mut style.fill_color);
    freeze_value(&mut style.label_color);
    freeze_value(&mut style.radius);
}

fn freeze_spinner_style(style: &mut WidgetSpinnerStyle) {
    freeze_widget_surface_style(&mut style.surface);
    freeze_value(&mut style.track_color);
    freeze_value(&mut style.indicator_color);
}

fn freeze_divider_style(style: &mut WidgetDividerStyle) {
    freeze_widget_surface_style(&mut style.surface);
    freeze_value(&mut style.color);
    freeze_value(&mut style.thickness);
    freeze_value(&mut style.inset);
    freeze_value(&mut style.label_color);
}

pub(super) fn lifecycle_snapshot<VM>(element: &ResolvedElement<VM>) -> LifecycleSnapshot {
    let mut layout = element.layout.clone();
    freeze_layout_style(&mut layout);
    let mut visual = element.visual.clone();
    freeze_visual_style(&mut visual);
    let mut background = element.background.clone();
    freeze_option_value(&mut background);

    LifecycleSnapshot {
        id: element.id,
        key: element.key.clone(),
        layout,
        visual,
        background,
        kind: lifecycle_widget_kind(&element.kind),
    }
}

pub(super) fn lifecycle_widget_kind<VM>(kind: &ResolvedWidgetKind<VM>) -> LifecycleWidgetKind {
    match kind {
        ResolvedWidgetKind::Container {
            layout, children, ..
        } => {
            let mut layout = layout.clone();
            freeze_container_layout(&mut layout);
            LifecycleWidgetKind::Container {
                layout,
                child_ids: children.iter().map(|child| child.id).collect(),
            }
        }
        ResolvedWidgetKind::Virtual {
            arrangement,
            item_layout,
            overflow_x,
            overflow_y,
            style,
            window_plan,
            children,
            child_meta,
            ..
        } => {
            let mut style = style.clone();
            freeze_container_style(&mut style);
            LifecycleWidgetKind::Virtual {
                arrangement: *arrangement,
                item_layout: *item_layout,
                overflow_x: *overflow_x,
                overflow_y: *overflow_y,
                style,
                window_plan: window_plan.clone(),
                child_ids: children.iter().map(|child| child.id).collect(),
                child_meta: child_meta.clone(),
            }
        }
        ResolvedWidgetKind::Text { text, .. } => {
            let mut text = text.clone();
            freeze_text(&mut text);
            LifecycleWidgetKind::Text { text }
        }
        #[cfg(feature = "audio")]
        ResolvedWidgetKind::Audio { audio } => {
            let mut audio = audio.clone();
            freeze_audio(&mut audio);
            LifecycleWidgetKind::Audio { audio }
        }
        ResolvedWidgetKind::Image { image, .. } => {
            let mut image = image.clone();
            freeze_image(&mut image);
            LifecycleWidgetKind::Image { image }
        }
        ResolvedWidgetKind::Icon { icon } => LifecycleWidgetKind::Icon {
            source: icon.source,
        },
        ResolvedWidgetKind::Canvas { scene, .. } => {
            let mut scene = scene.clone();
            freeze_value(&mut scene);
            LifecycleWidgetKind::Canvas { scene }
        }
        #[cfg(feature = "video")]
        ResolvedWidgetKind::VideoSurface { video, style, .. } => {
            let mut video = video.clone();
            let mut style = style.clone();
            freeze_option_value(&mut video.background);
            freeze_option_value(&mut video.cursor_style);
            freeze_video_surface_style(&mut style);
            LifecycleWidgetKind::VideoSurface { video, style }
        }
        ResolvedWidgetKind::Button {
            label,
            disabled,
            variant: _,
            style,
            ..
        } => {
            let mut label = label.clone();
            let mut disabled = disabled.clone();
            let mut style = style.clone();
            freeze_value(&mut label);
            freeze_value(&mut disabled);
            freeze_button_style(&mut style);
            LifecycleWidgetKind::Button {
                label,
                disabled,
                style,
            }
        }
        ResolvedWidgetKind::Checkbox {
            checked,
            label,
            disabled,
            validation,
            style,
            ..
        } => {
            let mut checked = checked.clone();
            let mut label = label.clone();
            let mut disabled = disabled.clone();
            let mut validation = validation.clone();
            let mut style = style.clone();
            freeze_value(&mut checked);
            freeze_option_value(&mut label);
            freeze_value(&mut disabled);
            freeze_value(&mut validation);
            freeze_checkbox_style(&mut style);
            LifecycleWidgetKind::Checkbox {
                checked,
                label,
                disabled,
                validation,
                style,
            }
        }
        ResolvedWidgetKind::Radio {
            checked,
            label,
            disabled,
            validation,
            style,
            ..
        } => {
            let mut checked = checked.clone();
            let mut label = label.clone();
            let mut disabled = disabled.clone();
            let mut validation = validation.clone();
            let mut style = style.clone();
            freeze_value(&mut checked);
            freeze_option_value(&mut label);
            freeze_value(&mut disabled);
            freeze_value(&mut validation);
            freeze_radio_style(&mut style);
            LifecycleWidgetKind::Radio {
                checked,
                label,
                disabled,
                validation,
                style,
            }
        }
        ResolvedWidgetKind::Switch {
            checked,
            active_background,
            inactive_background,
            active_thumb_color,
            inactive_thumb_color,
            disabled,
            validation,
            style,
            ..
        } => {
            let mut checked = checked.clone();
            let mut active_background = active_background.clone();
            let mut inactive_background = inactive_background.clone();
            let mut active_thumb_color = active_thumb_color.clone();
            let mut inactive_thumb_color = inactive_thumb_color.clone();
            let mut disabled = disabled.clone();
            let mut validation = validation.clone();
            let mut style = style.clone();
            freeze_value(&mut checked);
            freeze_option_value(&mut active_background);
            freeze_option_value(&mut inactive_background);
            freeze_option_value(&mut active_thumb_color);
            freeze_option_value(&mut inactive_thumb_color);
            freeze_value(&mut disabled);
            freeze_value(&mut validation);
            freeze_switch_style(&mut style);
            LifecycleWidgetKind::Switch {
                checked,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                validation,
                style,
            }
        }
        ResolvedWidgetKind::Select {
            selected_label,
            placeholder,
            options,
            open,
            disabled,
            validation,
            style,
            ..
        } => {
            let mut selected_label = selected_label.clone();
            let mut placeholder = placeholder.clone();
            let mut options = options
                .iter()
                .map(|option| LifecycleSelectOption {
                    label: option.label.clone(),
                    selected: option.selected.clone(),
                    disabled: option.disabled.clone(),
                })
                .collect::<Vec<_>>();
            let mut open = open.clone();
            let mut disabled = disabled.clone();
            let mut validation = validation.clone();
            let mut style = style.clone();
            selected_label.freeze();
            freeze_value(&mut placeholder);
            for option in &mut options {
                freeze_value(&mut option.label);
                freeze_value(&mut option.selected);
                freeze_value(&mut option.disabled);
            }
            freeze_option_value(&mut open);
            freeze_value(&mut disabled);
            freeze_value(&mut validation);
            freeze_select_style(&mut style);
            LifecycleWidgetKind::Select {
                selected_label,
                placeholder,
                options,
                open,
                disabled,
                validation,
                style,
            }
        }
        ResolvedWidgetKind::SelectOptionRow {
            owner_id,
            option_index,
            option,
            style,
            ..
        } => {
            let mut option = LifecycleSelectOption {
                label: option.label.clone(),
                selected: option.selected.clone(),
                disabled: option.disabled.clone(),
            };
            freeze_value(&mut option.label);
            freeze_value(&mut option.selected);
            freeze_value(&mut option.disabled);
            LifecycleWidgetKind::SelectOptionRow {
                owner_id: *owner_id,
                option_index: *option_index,
                option,
                style: style.clone(),
            }
        }
        ResolvedWidgetKind::Slider {
            value,
            label,
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
            ..
        } => {
            let mut value = value.clone();
            let mut label = label.clone();
            let mut disabled = disabled.clone();
            let mut validation = validation.clone();
            let mut style = style.clone();
            freeze_value(&mut value);
            freeze_option_value(&mut label);
            freeze_value(&mut disabled);
            freeze_value(&mut validation);
            freeze_slider_style(&mut style);
            LifecycleWidgetKind::Slider {
                value,
                label,
                min: *min,
                max: *max,
                step: *step,
                orientation: *orientation,
                show_ticks: *show_ticks,
                show_value_label: *show_value_label,
                tick_count: *tick_count,
                value_formatter: value_formatter.clone(),
                disabled,
                validation,
                style,
            }
        }
        ResolvedWidgetKind::ProgressBar {
            value,
            indeterminate,
            show_label,
            label,
            style,
            ..
        } => {
            let mut value = value.clone();
            let mut indeterminate = indeterminate.clone();
            let mut label = label.clone();
            let mut style = style.clone();
            freeze_value(&mut value);
            freeze_value(&mut indeterminate);
            freeze_option_value(&mut label);
            freeze_progress_bar_style(&mut style);
            LifecycleWidgetKind::ProgressBar {
                value,
                indeterminate,
                show_label: *show_label,
                label,
                style,
            }
        }
        ResolvedWidgetKind::Spinner {
            style,
            size_override,
            thickness_override,
            track_override,
            ..
        } => {
            let mut style = style.clone();
            let mut size_override = size_override.clone();
            let mut thickness_override = thickness_override.clone();
            freeze_spinner_style(&mut style);
            freeze_option_value(&mut size_override);
            freeze_option_value(&mut thickness_override);
            LifecycleWidgetKind::Spinner {
                style,
                size_override,
                thickness_override,
                track_override: *track_override,
            }
        }
        ResolvedWidgetKind::Divider {
            orientation,
            dashed,
            color_override,
            thickness_override,
            inset_override,
            label,
            style,
            ..
        } => {
            let mut dashed = dashed.clone();
            let mut color_override = color_override.clone();
            let mut thickness_override = thickness_override.clone();
            let mut inset_override = inset_override.clone();
            let mut label = label.clone();
            let mut style = style.clone();
            freeze_value(&mut dashed);
            freeze_option_value(&mut color_override);
            freeze_option_value(&mut thickness_override);
            freeze_option_value(&mut inset_override);
            freeze_option_value(&mut label);
            freeze_divider_style(&mut style);
            LifecycleWidgetKind::Divider {
                orientation: *orientation,
                dashed,
                color_override,
                thickness_override,
                inset_override,
                label,
                style,
            }
        }
        ResolvedWidgetKind::TextEditor {
            controller: _,
            placeholder,
            disabled,
            style,
            multiline,
            show_scrollbar,
            auto_wrap,
            validation,
            ..
        } => {
            let mut placeholder = placeholder.clone();
            let mut disabled = disabled.clone();
            let mut style = style.clone();
            let mut show_scrollbar = show_scrollbar.clone();
            let mut auto_wrap = auto_wrap.clone();
            let mut validation = validation.clone();
            freeze_value(&mut placeholder);
            freeze_value(&mut disabled);
            freeze_input_style(&mut style);
            freeze_value(&mut show_scrollbar);
            freeze_value(&mut auto_wrap);
            freeze_value(&mut validation);
            LifecycleWidgetKind::TextEditor {
                placeholder,
                disabled,
                style,
                multiline: *multiline,
                show_scrollbar,
                auto_wrap,
                validation,
            }
        }
        ResolvedWidgetKind::ToastHost { .. } => LifecycleWidgetKind::ToastHost,
        ResolvedWidgetKind::Portal { .. } => LifecycleWidgetKind::Portal,
    }
}
