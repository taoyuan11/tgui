use super::*;

impl<VM> ResolvedElement<VM> {
    pub(super) fn resolve_collect_border_color(
        &self,
        visual: &VisualStyle,
        widget_state: WidgetState,
        _opacity: f32,
        validation_color: Option<Color>,
        styles: &CollectResolvedStyles,
        context: &mut CollectContext<'_, '_>,
    ) -> Color {
        visual
            .border_color
            .as_ref()
            .map(|color| {
                color.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderColor,
                    context.now,
                )
            })
            .unwrap_or_else(|| {
                validation_color.unwrap_or_else(|| {
                    self.default_collect_border_color(widget_state, styles, context)
                })
            })
    }

    pub(super) fn resolve_collect_background(
        &self,
        background: Option<&Value<Color>>,
        widget_state: WidgetState,
        _opacity: f32,
        styles: &CollectResolvedStyles,
        context: &mut CollectContext<'_, '_>,
    ) -> Color {
        background
            .map(|background| {
                background.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::Background,
                    context.now,
                )
            })
            .unwrap_or_else(|| self.default_collect_background(widget_state, styles, context))
    }

    fn default_collect_border_color(
        &self,
        widget_state: WidgetState,
        styles: &CollectResolvedStyles,
        context: &mut CollectContext<'_, '_>,
    ) -> Color {
        match &self.kind {
            ResolvedWidgetKind::Button { .. } => {
                let button_style = styles
                    .button_style
                    .as_ref()
                    .expect("button style should be resolved for button widgets");
                context.animations.resolve_color(
                    crate::animation::AnimationKey::Widget {
                        id: self.id.raw(),
                        property: WidgetProperty::BorderColor,
                    },
                    button_style.border_color,
                    default_state_transition(context.theme, context.reduced_motion),
                    context.now,
                )
            }
            ResolvedWidgetKind::Select { .. } => {
                let select_style = styles
                    .select_style
                    .as_ref()
                    .expect("select style should be resolved for select widgets");
                context.animations.resolve_color(
                    crate::animation::AnimationKey::Widget {
                        id: self.id.raw(),
                        property: WidgetProperty::BorderColor,
                    },
                    select_style.border,
                    default_state_transition(context.theme, context.reduced_motion),
                    context.now,
                )
            }
            ResolvedWidgetKind::TextEditor { .. } => {
                let input_style = styles
                    .input_style
                    .as_ref()
                    .expect("input style should be resolved for input widgets");
                context.animations.resolve_color(
                    crate::animation::AnimationKey::Widget {
                        id: self.id.raw(),
                        property: WidgetProperty::BorderColor,
                    },
                    input_style.border,
                    default_state_transition(context.theme, context.reduced_motion),
                    context.now,
                )
            }
            ResolvedWidgetKind::Switch { checked, .. } => {
                let style = styles
                    .switch_style
                    .as_ref()
                    .expect("switch style should be resolved for switch widgets");
                let visual_state = base_interaction_state(widget_state);
                let color = if checked.resolve() {
                    resolve_stateful_widget_color(&style.border_checked, visual_state)
                } else {
                    resolve_stateful_widget_color(&style.border, visual_state)
                };
                context.animations.resolve_color(
                    crate::animation::AnimationKey::Widget {
                        id: self.id.raw(),
                        property: WidgetProperty::BorderColor,
                    },
                    color,
                    default_state_transition(context.theme, context.reduced_motion),
                    context.now,
                )
            }
            ResolvedWidgetKind::Slider { .. } => Color::TRANSPARENT,
            _ => Color::TRANSPARENT,
        }
    }

    fn default_collect_background(
        &self,
        widget_state: WidgetState,
        styles: &CollectResolvedStyles,
        context: &mut CollectContext<'_, '_>,
    ) -> Color {
        if let Some(list_item) = self.list_item.as_ref() {
            let color = if list_item.disabled.resolve() {
                list_item.item_disabled_background.resolve()
            } else if list_item.selected_keys.resolve().contains(&list_item.key) {
                list_item.item_selected_background.resolve()
            } else if widget_state.hovered || widget_state.pressed || widget_state.focused {
                list_item.item_hover_background.resolve()
            } else {
                list_item.item_background.resolve()
            };
            return context.animations.resolve_color(
                crate::animation::AnimationKey::Widget {
                    id: self.id.raw(),
                    property: WidgetProperty::Background,
                },
                color,
                default_state_transition(context.theme, context.reduced_motion),
                context.now,
            );
        }
        if let Some(tree_node) = self.tree_node.as_ref() {
            let color = if tree_node.disabled.resolve() {
                tree_node.item_disabled_background.resolve()
            } else if tree_node.selected_keys.resolve().contains(&tree_node.key) {
                tree_node.item_selected_background.resolve()
            } else if widget_state.hovered || widget_state.pressed || widget_state.focused {
                tree_node.item_hover_background.resolve()
            } else {
                tree_node.item_background.resolve()
            };
            return context.animations.resolve_color(
                crate::animation::AnimationKey::Widget {
                    id: self.id.raw(),
                    property: WidgetProperty::Background,
                },
                color,
                default_state_transition(context.theme, context.reduced_motion),
                context.now,
            );
        }
        match &self.kind {
            ResolvedWidgetKind::Button { .. } => {
                let button_style = styles
                    .button_style
                    .as_ref()
                    .expect("button style should be resolved for button widgets");
                context.animations.resolve_color(
                    crate::animation::AnimationKey::Widget {
                        id: self.id.raw(),
                        property: WidgetProperty::Background,
                    },
                    button_style.background,
                    default_state_transition(context.theme, context.reduced_motion),
                    context.now,
                )
            }
            ResolvedWidgetKind::Select { .. } => {
                let background = styles
                    .select_style
                    .as_ref()
                    .expect("select style should be resolved for select widgets")
                    .background;
                context.animations.resolve_color(
                    crate::animation::AnimationKey::Widget {
                        id: self.id.raw(),
                        property: WidgetProperty::Background,
                    },
                    background,
                    default_state_transition(context.theme, context.reduced_motion),
                    context.now,
                )
            }
            ResolvedWidgetKind::TextEditor { .. } => {
                let background = styles
                    .input_style
                    .as_ref()
                    .expect("input style should be resolved for input widgets")
                    .background;
                context.animations.resolve_color(
                    crate::animation::AnimationKey::Widget {
                        id: self.id.raw(),
                        property: WidgetProperty::Background,
                    },
                    background,
                    default_state_transition(context.theme, context.reduced_motion),
                    context.now,
                )
            }
            ResolvedWidgetKind::Switch {
                checked,
                active_background,
                inactive_background,
                ..
            } => context.animations.resolve_color(
                crate::animation::AnimationKey::Widget {
                    id: self.id.raw(),
                    property: WidgetProperty::BackgroundAlt,
                },
                {
                    let visual_state = base_interaction_state(widget_state);
                    if let Some(color) = self.collect_validation_color(context.theme) {
                        return color;
                    }
                    let style = styles
                        .switch_style
                        .as_ref()
                        .expect("switch style should be resolved for switch widgets");
                    if checked.resolve() {
                        active_background.as_ref().map(Value::resolve).unwrap_or(
                            resolve_stateful_widget_color(&style.track_checked, visual_state),
                        )
                    } else {
                        inactive_background
                            .as_ref()
                            .map(Value::resolve)
                            .unwrap_or(resolve_stateful_widget_color(&style.track, visual_state))
                    }
                },
                default_motion_transition(context.theme, context.reduced_motion),
                context.now,
            ),
            ResolvedWidgetKind::Slider { .. } => Color::TRANSPARENT,
            _ => Color::TRANSPARENT,
        }
    }
}
