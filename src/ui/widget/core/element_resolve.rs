use super::element_path::resolved_child_elements_with_previous;
use super::*;

impl<VM> Element<VM> {
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
            #[cfg(feature = "audio")]
            WidgetKind::Audio { audio } => ResolvedWidgetKind::Audio {
                audio: audio.clone(),
            },
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
