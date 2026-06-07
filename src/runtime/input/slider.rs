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
        min: f32,
        max: f32,
        step: f32,
    ) -> f32 {
        if track_rect.width <= Dp::ZERO {
            return min;
        }
        let normalized =
            ((position.x - track_rect.x).get() / track_rect.width.get()).clamp(0.0, 1.0);
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

    pub(super) fn begin_slider_drag(
        &mut self,
        widget_id: WidgetId,
        on_change: Option<ValueCommand<VM, f32>>,
        min: f32,
        max: f32,
        step: f32,
        track_rect: Rect,
        current_value: f32,
    ) -> bool {
        self.active_slider_drag = Some(SliderDrag {
            widget_id,
            on_change,
            min,
            max,
            step,
            track_rect,
            current_value,
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
        let current_value = drag.current_value;
        let on_change = drag.on_change.clone();
        let value = self.slider_value_for_position(position, track_rect, min, max, step);
        if (value - current_value).abs() <= f32::EPSILON {
            return false;
        }
        let changed = self.apply_slider_value(on_change.as_ref(), value, min, max, step, false);
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
        if self.active_slider_drag.take().is_none() {
            return false;
        }
        self.invalidate_scene_with_reason("end_slider_drag");
        true
    }

    pub(super) fn adjust_focused_slider(
        &mut self,
        direction: i32,
        set_to_edge: Option<bool>,
    ) -> bool {
        let Some(HitInteraction::Slider {
            on_change,
            value,
            min,
            max,
            step,
            ..
        }) = self.focused_slider_hit()
        else {
            return false;
        };
        let next_value = if let Some(max_edge) = set_to_edge {
            if max_edge {
                max
            } else {
                min
            }
        } else if let Some(step) = crate::ui::widget::slider_effective_step(min, max, step) {
            value + (step * direction as f32)
        } else {
            let span = (max - min).abs();
            if span <= f32::EPSILON {
                value
            } else {
                value + ((span / 100.0) * direction as f32)
            }
        };
        self.apply_slider_value(on_change.as_ref(), next_value, min, max, step, true)
    }
}
