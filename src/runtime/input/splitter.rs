use super::*;
use crate::ui::widget::{
    splitter_adjusted_sizes, splitter_reset_sizes, SplitterHandleState, SplitterResize,
};

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn focused_splitter_handle_hit(&mut self) -> Option<HitInteraction<VM>> {
        let widget_id = self.focused_widget_id()?;
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| match &region.interaction {
                HitInteraction::SplitterHandle { id, .. } if *id == widget_id => {
                    Some(region.interaction.clone())
                }
                _ => None,
            })
    }

    pub(super) fn begin_splitter_resize(
        &mut self,
        state: &SplitterHandleState<VM>,
        pair_extent: Dp,
        button: CanvasMouseButton,
    ) -> bool {
        if button != CanvasMouseButton::Left {
            return false;
        }
        let Some(start_position) = self.cursor_position else {
            return false;
        };
        self.active_splitter_resize = Some(super::super::ActiveSplitterResize {
            axis: state.axis,
            index: state.index,
            start_position,
            pair_extent: pair_extent.max(Dp::new(1.0)),
            start_sizes: state.sizes.clone(),
            constraints: state.constraints.clone(),
            current_sizes: state.sizes.clone(),
            moved: false,
            on_resize: state.on_resize.clone(),
        });
        self.invalidate_scene_with_reason("begin_splitter_resize");
        true
    }

    pub(super) fn handle_splitter_resize(&mut self) -> bool {
        let Some(position) = self.cursor_position else {
            return false;
        };
        let Some(active) = self.active_splitter_resize.as_ref() else {
            return false;
        };
        let axis = active.axis;
        let index = active.index;
        let start_position = active.start_position;
        let pair_extent = active.pair_extent;
        let start_sizes = active.start_sizes.clone();
        let constraints = active.constraints.clone();
        let current_sizes = active.current_sizes.clone();
        let on_resize = active.on_resize.clone();
        let delta_px = match axis {
            crate::ui::layout::Axis::Horizontal => (position.x - start_position.x).get(),
            crate::ui::layout::Axis::Vertical => (position.y - start_position.y).get(),
        };
        if delta_px.abs() < 1.0 {
            return false;
        }
        let delta = delta_px / pair_extent.get().max(1.0);
        let next_sizes = splitter_adjusted_sizes(&start_sizes, &constraints, index, delta);
        if sizes_close(&next_sizes, &current_sizes) {
            return false;
        }
        if let Some(command) = on_resize.as_ref() {
            self.execute_value_command(
                command,
                SplitterResize {
                    index,
                    sizes: next_sizes.clone(),
                },
            );
        }
        if let Some(active) = self.active_splitter_resize.as_mut() {
            active.current_sizes = next_sizes;
            active.moved = true;
        }
        self.clear_pending_splitter_click(axis, index, start_sizes.len());
        self.invalidate_scene_with_reason("splitter_resize");
        true
    }

    pub(super) fn end_splitter_resize(&mut self) -> bool {
        if self.active_splitter_resize.take().is_none() {
            return false;
        };
        self.invalidate_scene_with_reason("end_splitter_resize");
        true
    }

    pub(super) fn cancel_splitter_resize(&mut self) -> bool {
        if self.active_splitter_resize.take().is_none() {
            return false;
        }
        self.invalidate_scene_with_reason("cancel_splitter_resize");
        true
    }

    fn clear_pending_splitter_click(
        &mut self,
        axis: crate::ui::layout::Axis,
        index: usize,
        pane_count: usize,
    ) {
        let should_clear = self
            .pending_click
            .as_ref()
            .and_then(|pending| pending.splitter)
            .map(|pending| {
                pending.axis == axis && pending.index == index && pending.pane_count == pane_count
            })
            .unwrap_or(false);
        if should_clear {
            self.pending_click = None;
        }
    }

    pub(in crate::runtime) fn reset_splitter_from_hit(
        &mut self,
        state: &SplitterHandleState<VM>,
    ) -> bool {
        let Some(command) = state.on_resize.as_ref() else {
            return false;
        };
        self.execute_value_command(
            command,
            SplitterResize {
                index: state.index,
                sizes: splitter_reset_sizes(state.sizes.len()),
            },
        );
        true
    }

    pub(super) fn adjust_focused_splitter(
        &mut self,
        axis: crate::ui::layout::Axis,
        direction: i32,
    ) -> bool {
        let Some(HitInteraction::SplitterHandle { state, .. }) = self.focused_splitter_handle_hit()
        else {
            return false;
        };
        if state.axis != axis {
            return false;
        }
        let Some(command) = state.on_resize.as_ref() else {
            return false;
        };
        let next_sizes = splitter_adjusted_sizes(
            &state.sizes,
            &state.constraints,
            state.index,
            state.step * direction as f32,
        );
        self.execute_value_command(
            command,
            SplitterResize {
                index: state.index,
                sizes: next_sizes,
            },
        );
        true
    }
}

fn sizes_close(left: &[f32], right: &[f32]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(left, right)| (left - right).abs() <= 0.0005)
}
