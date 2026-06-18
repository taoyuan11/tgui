use super::*;

impl<VM> ResolvedElement<VM> {
    pub(super) fn resolve_collect_styles(
        &self,
        widget_state: WidgetState,
        context: &CollectContext<'_, '_>,
    ) -> CollectResolvedStyles {
        let theme = context.theme;
        CollectResolvedStyles {
            button_style: match &self.kind {
                ResolvedWidgetKind::Button {
                    runtime_style,
                    variant,
                    ..
                } => {
                    let mut style = runtime_style.base.clone();
                    context.style_sheet.apply_button_state(
                        &mut style,
                        &context.style_context,
                        *variant,
                        &self.visual,
                        widget_state,
                    );
                    let style = apply_local_style_with_state(
                        runtime_style.local.as_ref(),
                        style,
                        &context.style_context,
                        context.style_sheet,
                        &self.visual,
                        widget_state,
                    );
                    Some(resolve_button_style(&style, widget_state, theme))
                }
                _ => None,
            },
            select_style: match &self.kind {
                ResolvedWidgetKind::Select {
                    runtime_style,
                    validation,
                    ..
                } => {
                    let mut source_style = runtime_style.base.clone();
                    context.style_sheet.apply_select_state(
                        &mut source_style,
                        &context.style_context,
                        &self.visual,
                        widget_state,
                    );
                    let source_style = apply_local_style_with_state(
                        runtime_style.local.as_ref(),
                        source_style,
                        &context.style_context,
                        context.style_sheet,
                        &self.visual,
                        widget_state,
                    );
                    let mut style = resolve_select_style(&source_style, widget_state, theme);
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
                    runtime_style,
                    validation,
                    ..
                } => {
                    let mut source_style = runtime_style.base.clone();
                    context.style_sheet.apply_slider_state(
                        &mut source_style,
                        &context.style_context,
                        &self.visual,
                        widget_state,
                    );
                    let source_style = apply_local_style_with_state(
                        runtime_style.local.as_ref(),
                        source_style,
                        &context.style_context,
                        context.style_sheet,
                        &self.visual,
                        widget_state,
                    );
                    let mut style = resolve_slider_style(&source_style, widget_state, theme);
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
                ResolvedWidgetKind::ProgressBar { runtime_style, .. } => {
                    let mut style = runtime_style.base.clone();
                    context.style_sheet.apply_progress_bar_state(
                        &mut style,
                        &context.style_context,
                        &self.visual,
                        widget_state,
                    );
                    let style = apply_local_style_with_state(
                        runtime_style.local.as_ref(),
                        style,
                        &context.style_context,
                        context.style_sheet,
                        &self.visual,
                        widget_state,
                    );
                    Some(style)
                }
                _ => None,
            },
            spinner_style: match &self.kind {
                ResolvedWidgetKind::Spinner { runtime_style, .. } => {
                    let mut style = runtime_style.base.clone();
                    context.style_sheet.apply_spinner_state(
                        &mut style,
                        &context.style_context,
                        &self.visual,
                        widget_state,
                    );
                    let style = apply_local_style_with_state(
                        runtime_style.local.as_ref(),
                        style,
                        &context.style_context,
                        context.style_sheet,
                        &self.visual,
                        widget_state,
                    );
                    Some(style)
                }
                _ => None,
            },
            divider_style: match &self.kind {
                ResolvedWidgetKind::Divider { runtime_style, .. } => {
                    let mut style = runtime_style.base.clone();
                    context.style_sheet.apply_divider_state(
                        &mut style,
                        &context.style_context,
                        &self.visual,
                        widget_state,
                    );
                    let style = apply_local_style_with_state(
                        runtime_style.local.as_ref(),
                        style,
                        &context.style_context,
                        context.style_sheet,
                        &self.visual,
                        widget_state,
                    );
                    Some(style)
                }
                _ => None,
            },
            switch_style: match &self.kind {
                ResolvedWidgetKind::Switch {
                    runtime_style,
                    validation,
                    ..
                } => {
                    let mut style = runtime_style.base.clone();
                    context.style_sheet.apply_switch_state(
                        &mut style,
                        &context.style_context,
                        &self.visual,
                        widget_state,
                    );
                    let mut style = apply_local_style_with_state(
                        runtime_style.local.as_ref(),
                        style,
                        &context.style_context,
                        context.style_sheet,
                        &self.visual,
                        widget_state,
                    );
                    if let Some(color) = validation_state_color(&validation.resolve(), theme) {
                        style.border.normal = Value::Static(color);
                        style.border.hovered = Value::Static(color);
                        style.border.pressed = Value::Static(color);
                        style.border_checked.normal = Value::Static(color);
                        style.border_checked.hovered = Value::Static(color);
                        style.border_checked.pressed = Value::Static(color);
                    }
                    Some(style)
                }
                _ => None,
            },
            input_style: match &self.kind {
                ResolvedWidgetKind::TextEditor {
                    runtime_style,
                    validation,
                    ..
                } => {
                    let source_style = match runtime_style {
                        ResolvedTextEditorRuntimeStyle::Textarea(runtime_style) => {
                            let mut textarea_style = runtime_style.base.clone();
                            context.style_sheet.apply_textarea_state(
                                &mut textarea_style,
                                &context.style_context,
                                &self.visual,
                                widget_state,
                            );
                            let textarea_style = apply_local_style_with_state(
                                runtime_style.local.as_ref(),
                                textarea_style,
                                &context.style_context,
                                context.style_sheet,
                                &self.visual,
                                widget_state,
                            );
                            input_style_from_textarea_style(textarea_style)
                        }
                        ResolvedTextEditorRuntimeStyle::Input(runtime_style) => {
                            let mut input_style = runtime_style.base.clone();
                            context.style_sheet.apply_input_state(
                                &mut input_style,
                                &context.style_context,
                                &self.visual,
                                widget_state,
                            );
                            apply_local_style_with_state(
                                runtime_style.local.as_ref(),
                                input_style,
                                &context.style_context,
                                context.style_sheet,
                                &self.visual,
                                widget_state,
                            )
                        }
                    };
                    let mut style = resolve_input_style(&source_style, widget_state);
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
                    runtime_style,
                    validation,
                    ..
                } => {
                    let mut source_style = runtime_style.base.clone();
                    context.style_sheet.apply_checkbox_state(
                        &mut source_style,
                        &context.style_context,
                        &self.visual,
                        widget_state,
                    );
                    let source_style = apply_local_style_with_state(
                        runtime_style.local.as_ref(),
                        source_style,
                        &context.style_context,
                        context.style_sheet,
                        &self.visual,
                        widget_state,
                    );
                    let mut style = resolve_checkbox_style(
                        &source_style,
                        widget_state,
                        checked.resolve(),
                        theme,
                    );
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
                    runtime_style,
                    validation,
                    ..
                } => {
                    let mut source_style = runtime_style.base.clone();
                    context.style_sheet.apply_radio_state(
                        &mut source_style,
                        &context.style_context,
                        &self.visual,
                        widget_state,
                    );
                    let source_style = apply_local_style_with_state(
                        runtime_style.local.as_ref(),
                        source_style,
                        &context.style_context,
                        context.style_sheet,
                        &self.visual,
                        widget_state,
                    );
                    let mut style =
                        resolve_radio_style(&source_style, widget_state, checked.resolve(), theme);
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
        visual: &VisualStyle,
        styles: &CollectResolvedStyles,
        context: &mut CollectContext<'_, '_>,
    ) -> f32 {
        visual
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
        visual: &VisualStyle,
        styles: &CollectResolvedStyles,
        context: &mut CollectContext<'_, '_>,
    ) -> f32 {
        visual
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
            ResolvedWidgetKind::Switch { .. } => context.units.resolve_dp(
                styles
                    .switch_style
                    .as_ref()
                    .expect("switch style should be resolved for switch widgets")
                    .border_width
                    .resolve(),
            ),
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
            ResolvedWidgetKind::Switch { .. } => context.units.resolve_dp(
                styles
                    .switch_style
                    .as_ref()
                    .expect("switch style should be resolved for switch widgets")
                    .radius
                    .resolve(),
            ),
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
