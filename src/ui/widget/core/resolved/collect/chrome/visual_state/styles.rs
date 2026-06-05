use super::*;

impl<VM> ResolvedElement<VM> {
    pub(super) fn resolve_collect_styles(
        &self,
        widget_state: WidgetState,
        theme: &Theme,
    ) -> CollectResolvedStyles {
        CollectResolvedStyles {
            button_style: match &self.kind {
                ResolvedWidgetKind::Button { style, .. } => {
                    Some(resolve_button_style(style, widget_state, theme))
                }
                _ => None,
            },
            select_style: match &self.kind {
                ResolvedWidgetKind::Select {
                    style, validation, ..
                } => {
                    let mut style = resolve_select_style(style, widget_state, theme);
                    if let Some(color) = validation_state_color(&validation.resolve(), theme) {
                        style.border = color;
                        apply_validation_focus_ring(&mut style.focus_ring, color);
                    }
                    Some(style)
                }
                _ => None,
            },
            slider_style: match &self.kind {
                ResolvedWidgetKind::Slider {
                    style, validation, ..
                } => {
                    let mut style = resolve_slider_style(style, widget_state, theme);
                    if let Some(color) = validation_state_color(&validation.resolve(), theme) {
                        style.active_track = color;
                        style.tick = color.with_alpha_factor(0.55);
                        apply_validation_focus_ring(&mut style.focus_ring, color);
                    }
                    Some(style)
                }
                _ => None,
            },
            progress_bar_style: match &self.kind {
                ResolvedWidgetKind::ProgressBar { style, .. } => Some(style.clone()),
                _ => None,
            },
            spinner_style: match &self.kind {
                ResolvedWidgetKind::Spinner { style, .. } => Some(style.clone()),
                _ => None,
            },
            divider_style: match &self.kind {
                ResolvedWidgetKind::Divider { style, .. } => Some(style.clone()),
                _ => None,
            },
            input_style: match &self.kind {
                ResolvedWidgetKind::TextEditor {
                    style, validation, ..
                } => {
                    let mut style = resolve_input_style(style, widget_state);
                    if let Some(color) = validation_state_color(&validation.resolve(), theme) {
                        style.border = color;
                    }
                    Some(style)
                }
                _ => None,
            },
            checkbox_style: match &self.kind {
                ResolvedWidgetKind::Checkbox {
                    checked,
                    style,
                    validation,
                    ..
                } => {
                    let mut style =
                        resolve_checkbox_style(style, widget_state, checked.resolve(), theme);
                    if let Some(color) = validation_state_color(&validation.resolve(), theme) {
                        style.border = color;
                        if checked.resolve() {
                            style.background = color;
                            style.checkmark = theme.colors.on_error;
                        }
                        apply_validation_focus_ring(&mut style.focus_ring, color);
                    }
                    Some(style)
                }
                _ => None,
            },
            radio_style: match &self.kind {
                ResolvedWidgetKind::Radio {
                    checked,
                    style,
                    validation,
                    ..
                } => {
                    let mut style =
                        resolve_radio_style(style, widget_state, checked.resolve(), theme);
                    if let Some(color) = validation_state_color(&validation.resolve(), theme) {
                        style.border = color;
                        if checked.resolve() {
                            style.indicator = color;
                        }
                        apply_validation_focus_ring(&mut style.focus_ring, color);
                    }
                    Some(style)
                }
                _ => None,
            },
        }
    }

    pub(super) fn resolve_collect_border_width(
        &self,
        styles: &CollectResolvedStyles,
        context: &mut CollectContext<'_, '_>,
    ) -> f32 {
        self.visual
            .border_width
            .as_ref()
            .map(|width| {
                width.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderWidth,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or_else(|| self.default_collect_border_width(styles, context))
    }

    pub(super) fn resolve_collect_border_radius(
        &self,
        styles: &CollectResolvedStyles,
        context: &mut CollectContext<'_, '_>,
    ) -> f32 {
        self.visual
            .border_radius
            .as_ref()
            .map(|radius| {
                radius.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderRadius,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or_else(|| self.default_collect_border_radius(styles, context))
    }

    fn default_collect_border_width(
        &self,
        styles: &CollectResolvedStyles,
        context: &mut CollectContext<'_, '_>,
    ) -> f32 {
        match &self.kind {
            ResolvedWidgetKind::Button { .. } => context.units.resolve_dp(
                styles
                    .button_style
                    .as_ref()
                    .expect("button style should be resolved for button widgets")
                    .border_width,
            ),
            ResolvedWidgetKind::Select { .. } => context.units.resolve_dp(
                styles
                    .select_style
                    .as_ref()
                    .expect("select style should be resolved for select widgets")
                    .border_width,
            ),
            ResolvedWidgetKind::TextEditor { .. } => context.units.resolve_dp(
                styles
                    .input_style
                    .as_ref()
                    .expect("input style should be resolved for input widgets")
                    .border_width,
            ),
            ResolvedWidgetKind::Checkbox { .. } => context.units.resolve_dp(
                styles
                    .checkbox_style
                    .as_ref()
                    .expect("checkbox style should be resolved for checkbox widgets")
                    .border_width,
            ),
            ResolvedWidgetKind::Radio { .. } => context.units.resolve_dp(
                styles
                    .radio_style
                    .as_ref()
                    .expect("radio style should be resolved for radio widgets")
                    .border_width,
            ),
            ResolvedWidgetKind::Switch { style, .. } => {
                context.units.resolve_dp(style.border_width.resolve())
            }
            ResolvedWidgetKind::Slider { .. } => context.units.resolve_dp(
                styles
                    .slider_style
                    .as_ref()
                    .expect("slider style should be resolved for slider widgets")
                    .border_width,
            ),
            _ => 0.0,
        }
    }

    fn default_collect_border_radius(
        &self,
        styles: &CollectResolvedStyles,
        context: &mut CollectContext<'_, '_>,
    ) -> f32 {
        match &self.kind {
            ResolvedWidgetKind::Button { .. } => context.units.resolve_dp(
                styles
                    .button_style
                    .as_ref()
                    .expect("button style should be resolved for button widgets")
                    .radius,
            ),
            ResolvedWidgetKind::Select { .. } => context.units.resolve_dp(
                styles
                    .select_style
                    .as_ref()
                    .expect("select style should be resolved for select widgets")
                    .radius,
            ),
            ResolvedWidgetKind::TextEditor { .. } => context.units.resolve_dp(
                styles
                    .input_style
                    .as_ref()
                    .expect("input style should be resolved for input widgets")
                    .radius,
            ),
            ResolvedWidgetKind::Checkbox { .. } => context.units.resolve_dp(
                styles
                    .checkbox_style
                    .as_ref()
                    .expect("checkbox style should be resolved for checkbox widgets")
                    .radius,
            ),
            ResolvedWidgetKind::Radio { .. } => context.units.resolve_dp(
                styles
                    .radio_style
                    .as_ref()
                    .expect("radio style should be resolved for radio widgets")
                    .radius,
            ),
            ResolvedWidgetKind::Switch { style, .. } => {
                context.units.resolve_dp(style.radius.resolve())
            }
            ResolvedWidgetKind::Slider { .. } => context.units.resolve_dp(
                styles
                    .slider_style
                    .as_ref()
                    .expect("slider style should be resolved for slider widgets")
                    .radius,
            ),
            _ => 0.0,
        }
    }
}
