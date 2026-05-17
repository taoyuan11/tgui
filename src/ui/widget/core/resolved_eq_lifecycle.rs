use super::*;

impl PartialEq for LifecycleWidgetKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Container {
                    layout: left_layout,
                    child_ids: left_child_ids,
                },
                Self::Container {
                    layout: right_layout,
                    child_ids: right_child_ids,
                },
            ) => left_layout == right_layout && left_child_ids == right_child_ids,
            (Self::Text { text: left }, Self::Text { text: right }) => {
                left.content == right.content
                    && left.font_family == right.font_family
                    && left.background == right.background
                    && left.color == right.color
                    && left.font_size == right.font_size
                    && left.line_height == right.line_height
                    && left.font_weight == right.font_weight
                    && left.letter_spacing == right.letter_spacing
                    && left.cursor_style == right.cursor_style
                    && left.user_select == right.user_select
            }
            #[cfg(feature = "audio")]
            (Self::Audio { audio: left }, Self::Audio { audio: right }) => {
                left.controller == right.controller
                    && left.autoplay == right.autoplay
                    && left.looping == right.looping
            }
            (Self::Image { image: left }, Self::Image { image: right }) => {
                left.source == right.source
                    && left.background == right.background
                    && left.fit == right.fit
                    && left.cursor_style == right.cursor_style
            }
            (Self::Canvas { scene: left }, Self::Canvas { scene: right }) => left == right,
            #[cfg(feature = "video")]
            (
                Self::VideoSurface {
                    video: left_video,
                    style: left_style,
                },
                Self::VideoSurface {
                    video: right_video,
                    style: right_style,
                },
            ) => {
                left_video.background == right_video.background
                    && left_video.fit == right_video.fit
                    && left_video.cursor_style == right_video.cursor_style
                    && left_style == right_style
            }
            (
                Self::Button {
                    label: left_label,
                    disabled: left_disabled,
                    style: left_style,
                },
                Self::Button {
                    label: right_label,
                    disabled: right_disabled,
                    style: right_style,
                },
            ) => {
                left_label == right_label
                    && left_disabled == right_disabled
                    && left_style == right_style
            }
            (
                Self::Checkbox {
                    checked: left_checked,
                    label: left_label,
                    disabled: left_disabled,
                    style: left_style,
                },
                Self::Checkbox {
                    checked: right_checked,
                    label: right_label,
                    disabled: right_disabled,
                    style: right_style,
                },
            ) => {
                left_checked == right_checked
                    && left_label == right_label
                    && left_disabled == right_disabled
                    && left_style == right_style
            }
            (
                Self::Radio {
                    checked: left_checked,
                    label: left_label,
                    disabled: left_disabled,
                    style: left_style,
                },
                Self::Radio {
                    checked: right_checked,
                    label: right_label,
                    disabled: right_disabled,
                    style: right_style,
                },
            ) => {
                left_checked == right_checked
                    && left_label == right_label
                    && left_disabled == right_disabled
                    && left_style == right_style
            }
            (
                Self::Switch {
                    checked: left_checked,
                    active_background: left_active_background,
                    inactive_background: left_inactive_background,
                    active_thumb_color: left_active_thumb_color,
                    inactive_thumb_color: left_inactive_thumb_color,
                    disabled: left_disabled,
                    style: left_style,
                },
                Self::Switch {
                    checked: right_checked,
                    active_background: right_active_background,
                    inactive_background: right_inactive_background,
                    active_thumb_color: right_active_thumb_color,
                    inactive_thumb_color: right_inactive_thumb_color,
                    disabled: right_disabled,
                    style: right_style,
                },
            ) => {
                left_checked == right_checked
                    && left_active_background == right_active_background
                    && left_inactive_background == right_inactive_background
                    && left_active_thumb_color == right_active_thumb_color
                    && left_inactive_thumb_color == right_inactive_thumb_color
                    && left_disabled == right_disabled
                    && left_style == right_style
            }
            (
                Self::Select {
                    selected_label: left_selected_label,
                    placeholder: left_placeholder,
                    options: left_options,
                    open: left_open,
                    disabled: left_disabled,
                    style: left_style,
                },
                Self::Select {
                    selected_label: right_selected_label,
                    placeholder: right_placeholder,
                    options: right_options,
                    open: right_open,
                    disabled: right_disabled,
                    style: right_style,
                },
            ) => {
                left_selected_label == right_selected_label
                    && left_placeholder == right_placeholder
                    && left_open == right_open
                    && left_disabled == right_disabled
                    && left_style == right_style
                    && left_options.len() == right_options.len()
                    && left_options.iter().zip(right_options.iter()).all(
                        |(left_option, right_option)| {
                            left_option.label == right_option.label
                                && left_option.selected == right_option.selected
                                && left_option.disabled == right_option.disabled
                        },
                    )
            }
            (
                Self::Slider {
                    value: left_value,
                    min: left_min,
                    max: left_max,
                    step: left_step,
                    show_ticks: left_show_ticks,
                    show_value_label: left_show_value_label,
                    tick_count: left_tick_count,
                    value_formatter: left_value_formatter,
                    disabled: left_disabled,
                    style: left_style,
                },
                Self::Slider {
                    value: right_value,
                    min: right_min,
                    max: right_max,
                    step: right_step,
                    show_ticks: right_show_ticks,
                    show_value_label: right_show_value_label,
                    tick_count: right_tick_count,
                    value_formatter: right_value_formatter,
                    disabled: right_disabled,
                    style: right_style,
                },
            ) => {
                left_value == right_value
                    && left_min == right_min
                    && left_max == right_max
                    && left_step == right_step
                    && left_show_ticks == right_show_ticks
                    && left_show_value_label == right_show_value_label
                    && left_tick_count == right_tick_count
                    && left_disabled == right_disabled
                    && left_style == right_style
                    && left_value_formatter.is_some() == right_value_formatter.is_some()
            }
            (
                Self::TextEditor {
                    placeholder: left_placeholder,
                    disabled: left_disabled,
                    style: left_style,
                    multiline: left_multiline,
                    show_scrollbar: left_show_scrollbar,
                    auto_wrap: left_auto_wrap,
                },
                Self::TextEditor {
                    placeholder: right_placeholder,
                    disabled: right_disabled,
                    style: right_style,
                    multiline: right_multiline,
                    show_scrollbar: right_show_scrollbar,
                    auto_wrap: right_auto_wrap,
                },
            ) => {
                left_placeholder == right_placeholder
                    && left_disabled == right_disabled
                    && left_style == right_style
                    && left_multiline == right_multiline
                    && left_show_scrollbar == right_show_scrollbar
                    && left_auto_wrap == right_auto_wrap
            }
            _ => false,
        }
    }
}
