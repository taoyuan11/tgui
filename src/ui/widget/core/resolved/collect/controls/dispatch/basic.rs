use super::*;

impl<VM> ResolvedElement<VM> {
    pub(super) fn collect_button_control(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) -> bool {
        let ResolvedWidgetKind::Button { label, style, .. } = &self.kind else {
            return false;
        };

        let button_style = style.clone();
        let padding = Insets::symmetric(button_style.padding_x, button_style.padding_y);
        let button_foreground = context.animations.resolve_color(
            crate::animation::AnimationKey::Widget {
                id: self.id.raw(),
                property: WidgetProperty::TextColor,
            },
            resolve_stateful_widget_color(&button_style.foreground, visual.widget_state),
            default_state_transition(context.theme, context.reduced_motion),
            context.now,
        );
        let label_text = text_with_typography(label.clone(), &button_style.text_style);
        push_text_primitives(
            &label_text,
            visual.frame,
            context.font_manager,
            context.theme,
            context.units,
            context.animations,
            context.now,
            &mut computed.scene,
            false,
            true,
            padding,
            None,
            None,
            button_foreground,
            visual.opacity,
            self.id,
            visual.primitive_clip,
            visual.primitive_clip_mask,
        );
        true
    }

    pub(super) fn collect_checkbox_control(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) -> bool {
        let ResolvedWidgetKind::Checkbox {
            checked,
            label,
            on_change,
            ..
        } = &self.kind
        else {
            return false;
        };

        let checkbox_style = visual
            .styles
            .checkbox_style
            .as_ref()
            .expect("checkbox style should be resolved for checkbox widgets");
        push_checkbox_primitives(
            visual.frame,
            checked.resolve(),
            label.as_ref(),
            checkbox_style,
            visual.opacity,
            self.id,
            visual.primitive_clip,
            visual.overflow_clip,
            visual.primitive_clip_mask,
            context.font_manager,
            context.theme,
            context.units,
            context.animations,
            context.now,
            &mut computed.scene,
            default_state_transition(context.theme, context.reduced_motion),
        );
        if !visual.disabled {
            let focus = context.build_focus_meta(self.id, &self.focus, &self.interactions, true);
            computed.hit_regions.push(HitRegion {
                rect: visual.frame,
                clip_rect: visual.primitive_clip,
                geometry: HitGeometry::Rect,
                scope_path: context.focus_scope_path(),
                focus,
                interaction: HitInteraction::Checkbox {
                    id: self.id,
                    interactions: self.interactions.clone(),
                    on_change: on_change.clone(),
                    current: checked.resolve(),
                },
            });
        }
        true
    }

    pub(super) fn collect_radio_control(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) -> bool {
        let ResolvedWidgetKind::Radio {
            checked,
            label,
            on_change,
            ..
        } = &self.kind
        else {
            return false;
        };

        let radio_style = visual
            .styles
            .radio_style
            .as_ref()
            .expect("radio style should be resolved for radio widgets");
        push_radio_primitives(
            visual.frame,
            checked.resolve(),
            label.as_ref(),
            radio_style,
            visual.opacity,
            self.id,
            visual.primitive_clip,
            visual.overflow_clip,
            visual.primitive_clip_mask,
            context.font_manager,
            context.theme,
            context.units,
            context.animations,
            context.now,
            &mut computed.scene,
            default_state_transition(context.theme, context.reduced_motion),
        );
        if !visual.disabled {
            let focus = context.build_focus_meta(self.id, &self.focus, &self.interactions, true);
            computed.hit_regions.push(HitRegion {
                rect: visual.frame,
                clip_rect: visual.primitive_clip,
                geometry: HitGeometry::Rect,
                scope_path: context.focus_scope_path(),
                focus,
                interaction: HitInteraction::Radio {
                    id: self.id,
                    interactions: self.interactions.clone(),
                    on_change: on_change.clone(),
                    current: checked.resolve(),
                },
            });
        }
        true
    }

    pub(super) fn collect_switch_control(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) -> bool {
        let ResolvedWidgetKind::Switch {
            checked,
            on_change,
            active_thumb_color,
            inactive_thumb_color,
            ..
        } = &self.kind
        else {
            return false;
        };
        let switch_style = visual
            .styles
            .switch_style
            .as_ref()
            .expect("switch style should be resolved for switch widgets");

        let padding = self
            .layout
            .padding
            .as_ref()
            .map(|padding| {
                padding.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::Padding,
                    context.now,
                )
            })
            .unwrap_or(switch_style.padding);
        push_switch_primitives(
            visual.background_frame,
            visual.background_radius.get(),
            padding,
            checked.resolve(),
            active_thumb_color.as_ref().map(Value::resolve).unwrap_or(
                resolve_stateful_widget_color(&switch_style.thumb_checked, visual.widget_state),
            ),
            inactive_thumb_color.as_ref().map(Value::resolve).unwrap_or(
                resolve_stateful_widget_color(&switch_style.thumb, visual.widget_state),
            ),
            visual.opacity,
            self.id,
            visual.primitive_clip,
            visual.primitive_clip_mask,
            context.animations,
            &mut computed.scene,
            context.now,
            default_motion_transition(context.theme, context.reduced_motion),
        );
        if !visual.disabled {
            let focus = context.build_focus_meta(self.id, &self.focus, &self.interactions, true);
            computed.hit_regions.push(HitRegion {
                rect: visual.frame,
                clip_rect: visual.primitive_clip,
                geometry: HitGeometry::Rect,
                scope_path: context.focus_scope_path(),
                focus,
                interaction: HitInteraction::Switch {
                    id: self.id,
                    interactions: self.interactions.clone(),
                    on_change: on_change.clone(),
                    current: checked.resolve(),
                },
            });
        }
        true
    }
}
