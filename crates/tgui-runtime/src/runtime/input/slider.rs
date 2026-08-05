use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn focused_slider_hit(&mut self) -> Option<HitInteraction<VM>> {
        let widget_id = self.focused_widget_id()?;
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::Slider { id, .. } if *id == widget_id => {
                    Some(region.interaction.clone())
                }
                _ => None,
            })
    }

    pub(super) fn slider_value_for_position(
        &self,
        position: Point,
        track_rect: Rect,
        orientation: crate::ui::widget::SliderOrientation,
        min: f32,
        max: f32,
        step: f32,
    ) -> f32 {
        let normalized = if orientation.is_horizontal() {
            if track_rect.width <= Dp::ZERO {
                return min;
            }
            ((position.x - track_rect.x).get() / track_rect.width.get()).clamp(0.0, 1.0)
        } else {
            if track_rect.height <= Dp::ZERO {
                return min;
            }
            ((track_rect.bottom() - position.y).get() / track_rect.height.get()).clamp(0.0, 1.0)
        };
        crate::ui::widget::slider_value_from_normalized(normalized, min, max, step)
    }

    pub(in crate::runtime) fn apply_slider_value(
        &mut self,
        command: Option<&ValueCommand<VM, f32>>,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        invalidate_scene: bool,
    ) -> bool {
        let value = crate::ui::widget::slider_resolve_value(value, min, max, step);
        if let Some(command) = command {
            if invalidate_scene {
                self.execute_value_command(command, value);
            } else {
                self.execute_value_command_without_invalidation(command, value);
            }
            true
        } else {
            false
        }
    }

    pub(in crate::runtime) fn complete_slider_value(
        &mut self,
        on_change: Option<&ValueCommand<VM, f32>>,
        on_change_end: Option<&ValueCommand<VM, f32>>,
        current_value: f32,
        next_value: f32,
        min: f32,
        max: f32,
        step: f32,
    ) -> bool {
        let current_value = crate::ui::widget::slider_resolve_value(current_value, min, max, step);
        let next_value = crate::ui::widget::slider_resolve_value(next_value, min, max, step);
        if (next_value - current_value).abs() <= f32::EPSILON {
            return false;
        }

        if let Some(command) = on_change {
            if on_change_end.is_some() {
                self.execute_value_command_without_invalidation(command, next_value);
            } else {
                self.execute_value_command(command, next_value);
            }
        }
        if let Some(command) = on_change_end {
            self.execute_value_command(command, next_value);
        }
        true
    }

    pub(super) fn begin_slider_drag(
        &mut self,
        widget_id: WidgetId,
        on_change: Option<ValueCommand<VM, f32>>,
        on_change_end: Option<ValueCommand<VM, f32>>,
        min: f32,
        max: f32,
        step: f32,
        orientation: crate::ui::widget::SliderOrientation,
        track_rect: Rect,
        current_value: f32,
    ) -> bool {
        self.active_slider_drag = Some(SliderDrag {
            widget_id,
            on_change,
            on_change_end,
            min,
            max,
            step,
            orientation,
            track_rect,
            current_value,
            committed_value: None,
        });
        true
    }

    pub(super) fn handle_slider_drag(&mut self) -> bool {
        let Some(drag) = self.active_slider_drag.as_ref() else {
            return false;
        };
        let Some(position) = self.cursor_position else {
            return false;
        };
        let track_rect = drag.track_rect;
        let min = drag.min;
        let max = drag.max;
        let step = drag.step;
        let orientation = drag.orientation;
        let current_value = drag.current_value;
        let on_change = drag.on_change.clone();
        let on_change_end = drag.on_change_end.clone();
        let value =
            self.slider_value_for_position(position, track_rect, orientation, min, max, step);
        if (value - current_value).abs() <= f32::EPSILON {
            return false;
        }
        let changed = if on_change_end.is_some() {
            true
        } else {
            self.apply_slider_value(on_change.as_ref(), value, min, max, step, false)
        };
        if changed {
            if let Some(active) = self.active_slider_drag.as_mut() {
                active.current_value = value;
            }
            if !self.patch_active_slider_scene(Instant::now()) {
                self.invalidate_computed_scene();
            }
            return true;
        }
        false
    }

    pub(super) fn end_slider_drag(&mut self) -> bool {
        let Some(drag) = self.active_slider_drag.take() else {
            return false;
        };
        let value = crate::ui::widget::slider_resolve_value(
            drag.current_value,
            drag.min,
            drag.max,
            drag.step,
        );
        let already_committed = drag
            .committed_value
            .map(|committed| (committed - value).abs() <= f32::EPSILON)
            .unwrap_or(false);
        if let Some(command) = drag.on_change_end.as_ref() {
            if already_committed {
                self.invalidate_scene_with_reason("end_slider_drag");
                return true;
            }
            self.execute_value_command(command, value);
        }
        self.invalidate_scene_with_reason("end_slider_drag");
        true
    }

    pub(super) fn adjust_focused_slider(
        &mut self,
        direction: i32,
        set_to_edge: Option<bool>,
        required_horizontal: Option<bool>,
    ) -> bool {
        let Some(HitInteraction::Slider {
            on_change,
            on_change_end,
            value,
            min,
            max,
            step,
            orientation,
            ..
        }) = self.focused_slider_hit()
        else {
            return false;
        };
        if required_horizontal.is_some_and(|required| orientation.is_horizontal() != required) {
            return false;
        }
        let next_value = if let Some(max_edge) = set_to_edge {
            if max_edge {
                max
            } else {
                min
            }
        } else if let Some(step) = crate::ui::widget::slider_interaction_step(min, max, step) {
            value + (step * direction as f32)
        } else {
            value
        };
        let _ = self.complete_slider_value(
            on_change.as_ref(),
            on_change_end.as_ref(),
            value,
            next_value,
            min,
            max,
            step,
        );
        true
    }

    pub(super) fn page_focused_slider(&mut self, direction: i32) -> bool {
        let Some(HitInteraction::Slider {
            on_change,
            on_change_end,
            value,
            min,
            max,
            step,
            ..
        }) = self.focused_slider_hit()
        else {
            return false;
        };
        let span = max - min;
        if !span.is_finite() || span <= f32::EPSILON {
            return true;
        }
        let page = crate::ui::widget::slider_interaction_step(min, max, step)
            .unwrap_or(0.0)
            .max(span / 10.0);
        let _ = self.complete_slider_value(
            on_change.as_ref(),
            on_change_end.as_ref(),
            value,
            value + page * direction as f32,
            min,
            max,
            step,
        );
        true
    }
}
