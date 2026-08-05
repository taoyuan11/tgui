use super::*;
use crate::ui::widget::ScrollRegion;

fn scrollbar_region_axis_hit(
    region: &ScrollRegion,
    axis: ScrollbarAxis,
    cursor_position: Point,
) -> bool {
    match axis {
        ScrollbarAxis::Horizontal => region
            .horizontal_thumb
            .map(|thumb| thumb.contains(cursor_position))
            .unwrap_or(false),
        ScrollbarAxis::Vertical => region
            .vertical_thumb
            .map(|thumb| thumb.contains(cursor_position))
            .unwrap_or(false),
    }
}

fn scrollbar_axis_thumb_area(region: &ScrollRegion, axis: ScrollbarAxis) -> Option<f32> {
    let thumb = match axis {
        ScrollbarAxis::Horizontal => region.horizontal_thumb?,
        ScrollbarAxis::Vertical => region.vertical_thumb?,
    };
    Some(thumb.width.get() * thumb.height.get())
}

#[cfg(test)]
pub(crate) mod scroll_region_lookup_probe {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static WHEEL_CANDIDATE_VISITS: AtomicUsize = AtomicUsize::new(0);
    static SCROLLBAR_CANDIDATE_VISITS: AtomicUsize = AtomicUsize::new(0);
    static TOUCH_CANDIDATE_VISITS: AtomicUsize = AtomicUsize::new(0);
    static DRAG_ID_FALLBACK_VISITS: AtomicUsize = AtomicUsize::new(0);

    pub(crate) fn reset() {
        WHEEL_CANDIDATE_VISITS.store(0, Ordering::Relaxed);
        SCROLLBAR_CANDIDATE_VISITS.store(0, Ordering::Relaxed);
        TOUCH_CANDIDATE_VISITS.store(0, Ordering::Relaxed);
        DRAG_ID_FALLBACK_VISITS.store(0, Ordering::Relaxed);
    }

    pub(super) fn record_wheel_candidate() {
        WHEEL_CANDIDATE_VISITS.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_scrollbar_candidate() {
        SCROLLBAR_CANDIDATE_VISITS.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_touch_candidate() {
        TOUCH_CANDIDATE_VISITS.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_drag_id_fallback_candidate() {
        DRAG_ID_FALLBACK_VISITS.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn wheel_candidate_visits() -> usize {
        WHEEL_CANDIDATE_VISITS.load(Ordering::Relaxed)
    }

    pub(crate) fn scrollbar_candidate_visits() -> usize {
        SCROLLBAR_CANDIDATE_VISITS.load(Ordering::Relaxed)
    }

    pub(crate) fn touch_candidate_visits() -> usize {
        TOUCH_CANDIDATE_VISITS.load(Ordering::Relaxed)
    }

    pub(crate) fn drag_id_fallback_visits() -> usize {
        DRAG_ID_FALLBACK_VISITS.load(Ordering::Relaxed)
    }
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn mouse_wheel_scroll_target_from_indices(
        &self,
        scroll_regions: &[ScrollRegion],
        indices: impl DoubleEndedIterator<Item = usize>,
        cursor_position: Point,
        scroll_delta: Point,
    ) -> Option<(WidgetId, Point)> {
        for index in indices.rev() {
            #[cfg(test)]
            scroll_region_lookup_probe::record_wheel_candidate();
            let Some(region) = scroll_regions.get(index).copied() else {
                continue;
            };
            if region.visible_frame.is_empty() || !region.visible_frame.contains(cursor_position) {
                continue;
            }

            let max_offset = region.max_offset();
            let current_offset = self.effective_scroll_offset(region.id, region.scroll_offset);
            let mut next_offset = current_offset;
            if region.can_scroll_x() {
                next_offset.x = (next_offset.x - scroll_delta.x).clamp(0.0, max_offset.x);
            }
            if region.can_scroll_y() {
                next_offset.y = (next_offset.y - scroll_delta.y).clamp(0.0, max_offset.y);
            }

            if (next_offset.x - current_offset.x).abs() > 0.01
                || (next_offset.y - current_offset.y).abs() > 0.01
            {
                return Some((region.id, next_offset));
            }
        }
        None
    }

    fn mouse_wheel_scroll_target(
        &self,
        cursor_position: Point,
        scroll_delta: Point,
    ) -> Option<(WidgetId, Point)> {
        let cached = self.cached_scene.as_ref()?;
        let computed = &cached.computed;
        let scroll_regions = computed.scroll_regions.as_slice();
        if let Some(index) = computed.scroll_region_lookup_index() {
            return self.mouse_wheel_scroll_target_from_indices(
                scroll_regions,
                index.scrollable_indices().iter().copied(),
                cursor_position,
                scroll_delta,
            );
        }
        self.mouse_wheel_scroll_target_from_indices(
            scroll_regions,
            0..scroll_regions.len(),
            cursor_position,
            scroll_delta,
        )
    }

    pub(in crate::runtime) fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) -> bool {
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };

        let mut scroll_delta = mouse_scroll_delta(delta);
        if scroll_delta.x.abs() <= f32::EPSILON && self.modifiers.shift_key() {
            scroll_delta.x = scroll_delta.y;
            scroll_delta.y = Dp::ZERO;
        }
        if scroll_delta.x.abs() <= f32::EPSILON && scroll_delta.y.abs() <= f32::EPSILON {
            return false;
        }

        for interaction in self.hit_path(self.viewport_rect()).into_iter().rev() {
            if let HitInteraction::CanvasItem {
                item_id,
                ref item_interactions,
                canvas_origin,
                item_origin,
                inverse_transform,
                ref text_hits,
                ..
            } = interaction
            {
                if let Some(command) = &item_interactions.on_wheel {
                    self.execute_canvas_wheel_command(
                        command,
                        CanvasPointerContext {
                            item_id,
                            canvas_origin,
                            item_origin,
                            inverse_transform,
                            text_hits: Arc::clone(text_hits),
                        },
                        cursor_position,
                        scroll_delta,
                    );
                    return true;
                }
            }

            if let HitInteraction::TextInput {
                id,
                controller,
                frame,
                padding,
                text_style,
                multiline: true,
                auto_wrap,
                show_scrollbar,
                ..
            } = interaction
            {
                if !frame.contains(cursor_position) {
                    continue;
                }
                let value = self.text_input_current_value(id, &controller);
                let input = TextInputContext {
                    frame,
                    padding,
                    text_style: &text_style,
                    text: &value,
                    multiline: true,
                    auto_wrap,
                    show_scrollbar,
                };
                let scroll =
                    ScrollContext::new(self.scroll_states.get(&id).copied().unwrap_or(Point::ZERO));
                if self.scroll_multiline_text_input(id, input, scroll, scroll_delta) {
                    return true;
                }
            }
        }

        // CRITICAL: Use cached scroll_regions to avoid triggering computed_scene()
        // during mouse wheel handling, which causes stack overflow on Windows.
        // Copy only the target region instead of cloning the whole region table on
        // every wheel event.
        let target = self.mouse_wheel_scroll_target(cursor_position, scroll_delta);

        let Some((widget_id, next_offset)) = target else {
            return false;
        };

        self.touch_scroll_inertia_states.remove(&widget_id);
        self.set_smooth_scroll_target(widget_id, next_offset);
        true
    }

    pub(in crate::runtime) fn sync_scrollbar_hover(&mut self) -> bool {
        let next_hovered = if let Some(drag) = self.active_scrollbar_drag {
            Some(drag.handle)
        } else {
            self.scrollbar_thumb_hit()
        };

        if self.hovered_scrollbar != next_hovered {
            self.hovered_scrollbar = next_hovered;
            return true;
        }

        false
    }

    fn scrollbar_thumb_hit_from_indices(
        scroll_regions: &[ScrollRegion],
        indices: impl Iterator<Item = usize>,
        cursor_position: Point,
    ) -> Option<(ScrollbarHandle, usize)> {
        indices
            .filter_map(|index| {
                #[cfg(test)]
                scroll_region_lookup_probe::record_scrollbar_candidate();
                let region = scroll_regions.get(index)?;
                if region.visible_frame.is_empty()
                    || !region.visible_frame.contains(cursor_position)
                {
                    return None;
                }
                if let Some(thumb) = region
                    .vertical_thumb
                    .filter(|thumb| thumb.contains(cursor_position))
                {
                    return Some((
                        ScrollbarHandle {
                            id: region.id,
                            axis: ScrollbarAxis::Vertical,
                        },
                        index,
                        thumb.width.get() * thumb.height.get(),
                    ));
                }
                if let Some(thumb) = region
                    .horizontal_thumb
                    .filter(|thumb| thumb.contains(cursor_position))
                {
                    return Some((
                        ScrollbarHandle {
                            id: region.id,
                            axis: ScrollbarAxis::Horizontal,
                        },
                        index,
                        thumb.width.get() * thumb.height.get(),
                    ));
                }
                None
            })
            .min_by(|(_, _, a), (_, _, b)| a.total_cmp(b))
            .map(|(handle, index, _)| (handle, index))
    }

    fn scrollbar_thumb_hit_with_index(&mut self) -> Option<(ScrollbarHandle, usize)> {
        let cursor_position = self.cursor_position?;
        // CRITICAL: Use cached scroll_regions to avoid stack overflow
        let computed = &self.cached_scene.as_ref()?.computed;
        let scroll_regions = computed.scroll_regions.as_slice();
        if let Some(index) = computed.scroll_region_lookup_index() {
            return Self::scrollbar_thumb_hit_from_indices(
                scroll_regions,
                index.scrollbar_indices().iter().copied(),
                cursor_position,
            );
        }
        Self::scrollbar_thumb_hit_from_indices(
            scroll_regions,
            0..scroll_regions.len(),
            cursor_position,
        )
    }

    pub(super) fn scrollbar_thumb_hit(&mut self) -> Option<ScrollbarHandle> {
        self.scrollbar_thumb_hit_with_index()
            .map(|(handle, _)| handle)
    }

    fn topmost_scrollable_region_from_indices(
        scroll_regions: &[ScrollRegion],
        indices: impl DoubleEndedIterator<Item = usize>,
        cursor_position: Point,
    ) -> Option<ScrollRegion> {
        for index in indices.rev() {
            #[cfg(test)]
            scroll_region_lookup_probe::record_touch_candidate();
            let Some(region) = scroll_regions.get(index).copied() else {
                continue;
            };
            if !region.visible_frame.is_empty()
                && region.visible_frame.contains(cursor_position)
                && (region.can_scroll_x() || region.can_scroll_y())
            {
                return Some(region);
            }
        }
        None
    }

    pub(super) fn topmost_scrollable_region(&self, cursor_position: Point) -> Option<ScrollRegion> {
        let computed = &self.cached_scene.as_ref()?.computed;
        let scroll_regions = computed.scroll_regions.as_slice();
        if let Some(index) = computed.scroll_region_lookup_index() {
            return Self::topmost_scrollable_region_from_indices(
                scroll_regions,
                index.scrollable_indices().iter().copied(),
                cursor_position,
            );
        }
        Self::topmost_scrollable_region_from_indices(
            scroll_regions,
            0..scroll_regions.len(),
            cursor_position,
        )
    }

    pub(in crate::runtime) fn begin_scrollbar_drag(&mut self) -> bool {
        let Some((handle, scroll_region_index)) = self.scrollbar_thumb_hit_with_index() else {
            return false;
        };
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };
        // CRITICAL: Use cached scroll_regions to avoid stack overflow
        let scroll_regions = if let Some(cached) = self.cached_scene.as_ref() {
            &cached.computed.scroll_regions
        } else {
            return false;
        };
        let Some(region) = scroll_regions.get(scroll_region_index).copied() else {
            return false;
        };
        debug_assert_eq!(region.id, handle.id);

        let (track, thumb, max_offset) = match handle.axis {
            ScrollbarAxis::Horizontal => (
                region.horizontal_track,
                region.horizontal_thumb,
                region.max_offset().x,
            ),
            ScrollbarAxis::Vertical => (
                region.vertical_track,
                region.vertical_thumb,
                region.max_offset().y,
            ),
        };
        let (Some(track), Some(thumb)) = (track, thumb) else {
            return false;
        };

        self.cancel_scroll_motion(handle.id);
        self.active_scrollbar_drag = Some(ScrollbarDrag {
            handle,
            scroll_region_index,
            start_cursor: cursor_position,
            start_scroll_offset: region.scroll_offset,
            track,
            thumb,
            max_offset,
        });
        self.hovered_scrollbar = Some(handle);
        self.invalidate_computed_scene();
        true
    }

    pub(in crate::runtime) fn handle_scrollbar_drag(&mut self) -> bool {
        let Some(drag) = self.active_scrollbar_drag else {
            return false;
        };
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };

        let (travel, delta) = match drag.handle.axis {
            ScrollbarAxis::Horizontal => (
                (drag.track.width - drag.thumb.width).max(0.0),
                cursor_position.x - drag.start_cursor.x,
            ),
            ScrollbarAxis::Vertical => (
                (drag.track.height - drag.thumb.height).max(0.0),
                cursor_position.y - drag.start_cursor.y,
            ),
        };

        let mut next_offset = drag.start_scroll_offset;
        let axis_offset = if travel <= 0.0 || drag.max_offset <= 0.0 {
            Dp::ZERO
        } else {
            (delta / travel) * drag.max_offset
        };

        match drag.handle.axis {
            ScrollbarAxis::Horizontal => {
                next_offset.x =
                    (drag.start_scroll_offset.x + axis_offset).clamp(0.0, drag.max_offset)
            }
            ScrollbarAxis::Vertical => {
                next_offset.y =
                    (drag.start_scroll_offset.y + axis_offset).clamp(0.0, drag.max_offset)
            }
        }

        let previous = self
            .scroll_states
            .get(&drag.handle.id)
            .copied()
            .unwrap_or_else(|| {
                if drag.start_scroll_offset.x.abs() <= 0.01
                    && drag.start_scroll_offset.y.abs() <= 0.01
                {
                    Point::ZERO
                } else {
                    drag.start_scroll_offset
                }
            });
        if (previous.x - next_offset.x).abs() > 0.01 || (previous.y - next_offset.y).abs() > 0.01 {
            self.touch_scroll_inertia_states.remove(&drag.handle.id);
            self.set_scroll_offset(drag.handle.id, next_offset);
            self.rebind_active_scrollbar_drag_if_needed(drag, next_offset);
            return true;
        }

        false
    }

    fn rebind_active_scrollbar_drag_if_needed(&mut self, drag: ScrollbarDrag, next_offset: Point) {
        // CRITICAL: Do NOT call computed_scene() here - it causes stack overflow on Windows
        // during scroll drag events. Use cached scene if available, otherwise skip rebind.
        let Some((scroll_region_index, region)) = ({
            let scroll_regions = if let Some(cached) = self.cached_scene.as_ref() {
                &cached.computed.scroll_regions
            } else {
                return; // No cached scene available, skip rebind
            };
            if scroll_regions
                .get(drag.scroll_region_index)
                .is_some_and(|region| region.id == drag.handle.id)
            {
                return;
            }

            if let Some(scroll_region_index) = scroll_regions.iter().position(|region| {
                #[cfg(test)]
                scroll_region_lookup_probe::record_drag_id_fallback_candidate();
                region.id == drag.handle.id
            }) {
                self.active_scrollbar_drag = Some(ScrollbarDrag {
                    scroll_region_index,
                    ..drag
                });
                return;
            }

            let candidate_indices = self
                .cached_scene
                .as_ref()
                .and_then(|cached| cached.computed.scroll_region_lookup_index())
                .map(|index| index.scrollbar_indices());
            if let Some(indices) = candidate_indices {
                indices
                    .iter()
                    .copied()
                    .filter_map(|index| scroll_regions.get(index).map(|region| (index, region)))
                    .filter(|(_, region)| {
                        !region.visible_frame.is_empty()
                            && region.visible_frame.contains(drag.start_cursor)
                            && scrollbar_region_axis_hit(
                                region,
                                drag.handle.axis,
                                drag.start_cursor,
                            )
                    })
                    .min_by(|(_, a), (_, b)| {
                        scrollbar_axis_thumb_area(a, drag.handle.axis)
                            .unwrap_or(f32::MAX)
                            .total_cmp(
                                &scrollbar_axis_thumb_area(b, drag.handle.axis).unwrap_or(f32::MAX),
                            )
                    })
                    .map(|(index, region)| (index, *region))
            } else {
                scroll_regions
                    .iter()
                    .enumerate()
                    .filter(|(_, region)| {
                        !region.visible_frame.is_empty()
                            && region.visible_frame.contains(drag.start_cursor)
                            && scrollbar_region_axis_hit(
                                region,
                                drag.handle.axis,
                                drag.start_cursor,
                            )
                    })
                    .min_by(|(_, a), (_, b)| {
                        scrollbar_axis_thumb_area(a, drag.handle.axis)
                            .unwrap_or(f32::MAX)
                            .total_cmp(
                                &scrollbar_axis_thumb_area(b, drag.handle.axis).unwrap_or(f32::MAX),
                            )
                    })
                    .map(|(index, region)| (index, *region))
            }
        }) else {
            return;
        };

        let (track, thumb, max_offset) = match drag.handle.axis {
            ScrollbarAxis::Horizontal => (
                region.horizontal_track,
                region.horizontal_thumb,
                region.max_offset().x,
            ),
            ScrollbarAxis::Vertical => (
                region.vertical_track,
                region.vertical_thumb,
                region.max_offset().y,
            ),
        };
        let (Some(track), Some(thumb)) = (track, thumb) else {
            return;
        };

        self.scroll_states.remove(&drag.handle.id);
        self.touch_scroll_inertia_states.remove(&drag.handle.id);
        self.touch_scroll_inertia_states.remove(&region.id);
        self.active_scrollbar_drag = Some(ScrollbarDrag {
            handle: ScrollbarHandle {
                id: region.id,
                axis: drag.handle.axis,
            },
            scroll_region_index,
            start_cursor: drag.start_cursor,
            start_scroll_offset: drag.start_scroll_offset,
            track,
            thumb,
            max_offset,
        });
        self.hovered_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        self.set_scroll_offset(region.id, next_offset);
    }

    pub(in crate::runtime) fn handle_canvas_drag(&mut self) -> bool {
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };

        let Some(mut drag) = self.active_canvas_drag.take() else {
            return false;
        };

        if !drag.started {
            if let Some(command) = drag.on_drag_start.clone() {
                self.execute_canvas_drag_command(
                    &command,
                    drag.context.clone(),
                    drag.start_position,
                    cursor_position,
                    drag.button,
                );
            }
            drag.started = true;
        }

        if let Some(command) = drag.on_drag.clone() {
            self.execute_canvas_drag_command(
                &command,
                drag.context.clone(),
                drag.start_position,
                cursor_position,
                drag.button,
            );
        }

        self.active_canvas_drag = Some(drag);
        true
    }

    pub(super) fn end_scrollbar_drag(&mut self) -> bool {
        if self.active_scrollbar_drag.take().is_none() {
            return false;
        }
        self.sync_scrollbar_hover();
        self.invalidate_computed_scene();
        true
    }

    pub(super) fn end_canvas_drag(&mut self) -> bool {
        let Some(drag) = self.active_canvas_drag.take() else {
            return false;
        };
        if !drag.started {
            return false;
        }
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };
        if let Some(command) = drag.on_drag_end {
            self.execute_canvas_drag_command(
                &command,
                drag.context,
                drag.start_position,
                cursor_position,
                drag.button,
            );
        }
        true
    }

    pub(in crate::runtime) fn update_cursor_icon(&mut self) -> bool {
        let next_icon = if self.active_scrollbar_drag.is_some() || self.hovered_scrollbar.is_some()
        {
            CursorIcon::Pointer
        } else if self.active_text_selection.is_some() {
            CursorIcon::Text
        } else if let Some(active) = self.active_splitter_resize.as_ref() {
            match active.axis {
                crate::ui::layout::Axis::Horizontal => CursorIcon::EwResize,
                crate::ui::layout::Axis::Vertical => CursorIcon::NsResize,
            }
        } else if let Some(cursor_style) = self
            .hovered_widgets
            .iter()
            .rev()
            .find_map(|hovered| hovered.cursor_style)
        {
            cursor_icon(cursor_style)
        } else {
            CursorIcon::Default
        };

        if self.cursor_icon == Some(next_icon) {
            return false;
        }

        self.cursor_icon = Some(next_icon);
        if let Some(window) = self.window.as_ref() {
            window.set_cursor(Cursor::Icon(next_icon));
        }
        true
    }

    pub(in crate::runtime) fn set_scroll_offset(&mut self, widget_id: WidgetId, offset: Point) {
        let offset = Point::new(offset.x.max(Dp::ZERO), offset.y.max(Dp::ZERO));
        let previous = self
            .scroll_states
            .get(&widget_id)
            .copied()
            .unwrap_or(Point::ZERO);
        let mut changed =
            (previous.x - offset.x).abs() > 0.01 || (previous.y - offset.y).abs() > 0.01;

        if let Some(state) = self.text_edit_states.get_mut(&widget_id) {
            if (state.scroll_x - offset.x).abs() > 0.01 || (state.scroll_y - offset.y).abs() > 0.01
            {
                state.scroll_x = offset.x;
                state.scroll_y = offset.y;
                changed = true;
            }
        }

        if !changed {
            return;
        }

        if offset.x.abs() <= 0.01 && offset.y.abs() <= 0.01 {
            self.scroll_states.remove(&widget_id);
        } else {
            self.scroll_states.insert(widget_id, offset);
        }
        self.scroll_epoch = self.scroll_epoch.wrapping_add(1);
        // 记录本容器发生了滚动变化,供纯滚动快路径定位需要重新收集的子树根。
        self.scroll_dirty_widgets.insert(widget_id);
        self.invalidation.request_redraw();
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}
