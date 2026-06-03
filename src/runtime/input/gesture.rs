use super::*;
use crate::platform::event::{ButtonSource, FingerId, MouseButton, PointerSource};
use crate::runtime::state::{ActiveGestureSession, ActivePinchSession, GestureAxisLock};
use crate::ui::unit::dp;
use crate::ui::widget::{GestureEdge, GesturePhase, GestureSource, SwipeAxis, SwipeDirection};

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn gesture_source_from_button(button: &ButtonSource) -> GestureSource {
        match button {
            ButtonSource::Touch { .. } => GestureSource::Touch,
            _ => GestureSource::Mouse,
        }
    }

    pub(super) fn gesture_finger_id_from_button(button: &ButtonSource) -> Option<FingerId> {
        match button {
            ButtonSource::Touch { finger_id, .. } => Some(*finger_id),
            _ => None,
        }
    }

    pub(super) fn gesture_finger_id_from_pointer_source(
        source: &PointerSource,
    ) -> Option<FingerId> {
        match source {
            PointerSource::Touch { finger_id, .. } => Some(*finger_id),
            _ => None,
        }
    }

    pub(super) fn begin_gesture_session(
        &mut self,
        viewport: Rect,
        now: Instant,
        button: &ButtonSource,
    ) -> bool {
        let Some(cursor_position) = self.cursor_position else {
            return false;
        };
        let Some((widget_id, target_id, recognizer)) = self.gesture_hit_target(viewport) else {
            return false;
        };
        let gesture_finger = Self::gesture_finger_id_from_button(button);

        if let Some(finger_id) = gesture_finger {
            if self.try_begin_pinch_session(widget_id, target_id, finger_id, cursor_position) {
                return true;
            }
            if self.touch_press_is_blocked_by_active_gesture(finger_id) {
                return false;
            }
        }

        let edge_candidate = self.detect_gesture_edge(cursor_position, viewport, &recognizer);
        let (scroll_can_x, scroll_can_y) = self.scroll_region_axes_at(cursor_position);
        let long_press_deadline = recognizer
            .on_long_press
            .as_ref()
            .map(|_| now + super::super::LONG_PRESS_THRESHOLD);
        let mut session = ActiveGestureSession {
            widget_id,
            target_id,
            recognizer,
            source: Self::gesture_source_from_button(button),
            finger_id: gesture_finger,
            start_position: cursor_position,
            current_position: cursor_position,
            pressed_at: now,
            long_press_deadline,
            edge_candidate,
            scroll_can_x,
            scroll_can_y,
            axis_lock: None,
            active_kind: None,
            active_direction: None,
            swipe_axis: None,
            captured: false,
            long_press_triggered: false,
        };

        let is_right_mouse = matches!(button.clone().mouse_button(), Some(MouseButton::Right));
        if is_right_mouse && session.recognizer.on_long_press.is_some() {
            if let Some(command) = session.recognizer.on_long_press.clone() {
                let event = session.long_press_event(GesturePhase::Recognized);
                self.execute_value_command(&command, event);
            }
            session.long_press_triggered = true;
            session.long_press_deadline = None;
            self.pending_click = None;
        }

        self.active_gesture = Some(session);
        true
    }

    pub(super) fn handle_gesture_pointer_move(
        &mut self,
        viewport: Rect,
        finger_id: Option<FingerId>,
    ) -> bool {
        if let Some(changed) = self.handle_pinch_pointer_move(finger_id) {
            return changed;
        }

        let Some(mut session) = self.active_gesture.take() else {
            return false;
        };
        if session.finger_id != finger_id && session.source == GestureSource::Touch {
            self.active_gesture = Some(session);
            return false;
        }
        let Some(position) = self.cursor_position else {
            self.active_gesture = Some(session);
            return false;
        };

        session.current_position = position;
        let delta = session.pointer_delta_from(position);
        if session.long_press_deadline.is_some()
            && (delta.x.abs() >= super::super::LONG_PRESS_MOVE_TOLERANCE
                || delta.y.abs() >= super::super::LONG_PRESS_MOVE_TOLERANCE)
        {
            session.long_press_deadline = None;
        }

        if session.captured {
            let changed = self.dispatch_active_gesture_update(&session);
            self.active_gesture = Some(session);
            return changed;
        }

        let abs_x = delta.x.abs();
        let abs_y = delta.y.abs();
        if abs_x < super::super::SWIPE_ACTIVATION_THRESHOLD
            && abs_y < super::super::SWIPE_ACTIVATION_THRESHOLD
        {
            self.active_gesture = Some(session);
            return false;
        }

        let axis_lock = if abs_x >= abs_y {
            let dominance = abs_x - abs_y;
            if dominance >= super::super::SWIPE_AXIS_LOCK_THRESHOLD
                || abs_y < super::super::SWIPE_AXIS_LOCK_THRESHOLD
            {
                Some(GestureAxisLock::Horizontal)
            } else {
                None
            }
        } else {
            let dominance = abs_y - abs_x;
            if dominance >= super::super::SWIPE_AXIS_LOCK_THRESHOLD
                || abs_x < super::super::SWIPE_AXIS_LOCK_THRESHOLD
            {
                Some(GestureAxisLock::Vertical)
            } else {
                None
            }
        };
        let Some(axis_lock) = axis_lock else {
            self.active_gesture = Some(session);
            return false;
        };
        session.axis_lock = Some(axis_lock);
        let (swipe_axis, direction) = match axis_lock {
            GestureAxisLock::Horizontal => (
                SwipeAxis::Horizontal,
                if delta.x >= 0.0 {
                    SwipeDirection::Right
                } else {
                    SwipeDirection::Left
                },
            ),
            GestureAxisLock::Vertical => (
                SwipeAxis::Vertical,
                if delta.y >= 0.0 {
                    SwipeDirection::Down
                } else {
                    SwipeDirection::Up
                },
            ),
        };

        let edge_capture = session.edge_candidate.filter(|edge| {
            matches!(
                (*edge, direction),
                (GestureEdge::Left, SwipeDirection::Right)
                    | (GestureEdge::Right, SwipeDirection::Left)
                    | (GestureEdge::Top, SwipeDirection::Down)
                    | (GestureEdge::Bottom, SwipeDirection::Up)
            )
        });
        let swipe_capture = self.swipe_capture_allowed(&session, swipe_axis, direction, viewport);
        if edge_capture.is_none() && !swipe_capture {
            self.active_gesture = Some(session);
            return false;
        }

        session.long_press_deadline = None;
        session.captured = true;
        session.active_direction = Some(direction);
        session.swipe_axis = Some(swipe_axis);
        session.active_kind = Some(if edge_capture.is_some() {
            super::super::state::GestureRuntimeKind::EdgeSwipe
        } else {
            super::super::state::GestureRuntimeKind::Swipe
        });
        self.pending_click = None;
        let changed = self.dispatch_active_gesture_phase(&session, GesturePhase::Start);
        self.active_gesture = Some(session);
        changed
    }

    pub(in crate::runtime) fn flush_pending_long_press_if_due(&mut self, now: Instant) -> bool {
        let Some(mut session) = self.active_gesture.take() else {
            return false;
        };
        let should_fire = session
            .long_press_deadline
            .map(|deadline| deadline <= now)
            .unwrap_or(false)
            && !session.long_press_triggered;
        if !should_fire {
            self.active_gesture = Some(session);
            return false;
        }

        session.long_press_deadline = None;
        session.long_press_triggered = true;
        self.pending_click = None;
        if session.source == GestureSource::Touch {
            self.tooltip_state.long_press_candidate = Some(session.widget_id);
            self.tooltip_state.long_press_release_deadline = None;
        }
        let changed = if let Some(command) = session.recognizer.on_long_press.clone() {
            self.execute_value_command(
                &command,
                session.long_press_event(GesturePhase::Recognized),
            );
            true
        } else {
            false
        };
        self.active_gesture = Some(session);
        changed
    }

    pub(super) fn end_gesture_session(
        &mut self,
        finger_id: Option<FingerId>,
        cancel: bool,
    ) -> bool {
        if let Some(changed) = self.end_pinch_session(finger_id, cancel) {
            return changed;
        }
        let Some(session) = self.active_gesture.take() else {
            return false;
        };
        if !cancel && session.source == GestureSource::Touch && session.finger_id != finger_id {
            self.active_gesture = Some(session);
            return false;
        }
        if cancel {
            if session.source == GestureSource::Touch {
                self.reset_tooltip_long_press_session(session.widget_id);
            }
            return self.dispatch_active_gesture_phase(&session, GesturePhase::Cancel);
        }
        if session.source == GestureSource::Touch && session.long_press_triggered {
            self.schedule_tooltip_long_press_hide(session.widget_id, Instant::now());
        } else if session.source == GestureSource::Touch {
            self.reset_tooltip_long_press_session(session.widget_id);
        }
        if session.captured {
            return self.dispatch_active_gesture_phase(&session, GesturePhase::End);
        }
        false
    }

    pub(in crate::runtime) fn cancel_active_gesture(&mut self) -> bool {
        if let Some(session) = self.active_pinch.take() {
            return if session.captured {
                self.dispatch_active_pinch_phase(&session, GesturePhase::Cancel)
            } else {
                false
            };
        }
        let Some(session) = self.active_gesture.take() else {
            return false;
        };
        if session.source == GestureSource::Touch {
            self.reset_tooltip_long_press_session(session.widget_id);
        }
        self.dispatch_active_gesture_phase(&session, GesturePhase::Cancel)
    }

    pub(super) fn gesture_consumes_click(&self) -> bool {
        if self.active_pinch.is_some() {
            return true;
        }
        self.active_gesture
            .as_ref()
            .map(|gesture| gesture.captured || gesture.long_press_triggered)
            .unwrap_or(false)
    }

    pub(super) fn touch_press_is_blocked_by_active_gesture(&self, finger_id: FingerId) -> bool {
        self.active_pinch.is_some()
            || self
                .active_gesture
                .as_ref()
                .map(|gesture| {
                    gesture.source == GestureSource::Touch && gesture.finger_id != Some(finger_id)
                })
                .unwrap_or(false)
    }

    fn handle_pinch_pointer_move(&mut self, finger_id: Option<FingerId>) -> Option<bool> {
        let Some(mut session) = self.active_pinch.take() else {
            return None;
        };
        let Some(finger_id) = finger_id else {
            self.active_pinch = Some(session);
            return Some(false);
        };
        let Some(index) = session.finger_index(finger_id) else {
            self.active_pinch = Some(session);
            return Some(false);
        };
        let Some(position) = self.cursor_position else {
            self.active_pinch = Some(session);
            return Some(false);
        };

        session.current_positions[index] = position;
        if !session.captured {
            let distance_delta = (session.distance().get() - session.start_distance().get()).abs();
            if distance_delta < super::super::PINCH_ACTIVATION_THRESHOLD {
                self.active_pinch = Some(session);
                return Some(false);
            }
            session.captured = true;
            self.pending_click = None;
            let changed = self.dispatch_active_pinch_phase(&session, GesturePhase::Start);
            self.active_pinch = Some(session);
            return Some(changed);
        }

        let changed = self.dispatch_active_pinch_phase(&session, GesturePhase::Update);
        self.active_pinch = Some(session);
        Some(changed)
    }

    fn end_pinch_session(&mut self, finger_id: Option<FingerId>, cancel: bool) -> Option<bool> {
        let Some(session) = self.active_pinch.take() else {
            return None;
        };

        if let Some(finger_id) = finger_id {
            if !session.contains_finger(finger_id) {
                self.active_pinch = Some(session);
                return Some(false);
            }
        }

        if cancel {
            return Some(if session.captured {
                self.dispatch_active_pinch_phase(&session, GesturePhase::Cancel)
            } else {
                false
            });
        }

        Some(if session.captured {
            self.dispatch_active_pinch_phase(&session, GesturePhase::End)
        } else {
            false
        })
    }

    fn dispatch_active_gesture_update(&mut self, session: &ActiveGestureSession<VM>) -> bool {
        self.dispatch_active_gesture_phase(session, GesturePhase::Update)
    }

    pub(super) fn dispatch_active_gesture_phase(
        &mut self,
        session: &ActiveGestureSession<VM>,
        phase: GesturePhase,
    ) -> bool {
        match session.active_kind {
            Some(super::super::state::GestureRuntimeKind::EdgeSwipe) => {
                if let (Some(command), Some(event)) = (
                    session.recognizer.on_edge_swipe.as_ref().map(
                        |entry: &(
                            crate::ui::widget::GestureEdgeSet,
                            ValueCommand<VM, crate::ui::widget::EdgeSwipeEvent>,
                        )| entry.1.clone(),
                    ),
                    session.edge_swipe_event(phase),
                ) {
                    self.execute_value_command(&command, event);
                    return true;
                }
            }
            Some(super::super::state::GestureRuntimeKind::Swipe) => {
                if let (Some(command), Some(event)) = (
                    session.recognizer.on_swipe.as_ref().map(
                        |entry: &(
                            SwipeAxis,
                            ValueCommand<VM, crate::ui::widget::SwipeGestureEvent>,
                        )| entry.1.clone(),
                    ),
                    session.swipe_event(phase),
                ) {
                    self.execute_value_command(&command, event);
                    return true;
                }
            }
            None => {}
        }
        false
    }

    fn dispatch_active_pinch_phase(
        &mut self,
        session: &ActivePinchSession<VM>,
        phase: GesturePhase,
    ) -> bool {
        if let Some(command) = session.recognizer.on_pinch.clone() {
            self.execute_value_command(&command, session.pinch_event(phase));
            return true;
        }
        false
    }

    fn try_begin_pinch_session(
        &mut self,
        widget_id: WidgetId,
        target_id: HoverTargetId,
        second_finger_id: FingerId,
        second_position: Point,
    ) -> bool {
        if self.active_pinch.is_some() {
            return true;
        }
        let Some(session) = self.active_gesture.take() else {
            return false;
        };

        let pinch_supported = session.source == GestureSource::Touch
            && session.widget_id == widget_id
            && session.target_id == target_id
            && session.finger_id != Some(second_finger_id)
            && !session.captured
            && !session.long_press_triggered
            && session.recognizer.on_pinch.is_some();
        if !pinch_supported {
            self.active_gesture = Some(session);
            return false;
        }

        let Some(first_finger_id) = session.finger_id else {
            self.active_gesture = Some(session);
            return false;
        };

        self.pending_click = None;
        self.deferred_mouse_click = None;
        self.end_touch_scroll_drag();
        self.active_pinch = Some(ActivePinchSession {
            widget_id: session.widget_id,
            target_id: session.target_id,
            recognizer: session.recognizer,
            source: GestureSource::Touch,
            finger_ids: [first_finger_id, second_finger_id],
            start_positions: [session.current_position, second_position],
            current_positions: [session.current_position, second_position],
            captured: false,
        });
        true
    }

    fn gesture_hit_target(
        &mut self,
        viewport: Rect,
    ) -> Option<(
        WidgetId,
        HoverTargetId,
        crate::ui::widget::GestureRecognizer<VM>,
    )> {
        let hit_path = self.hit_path(viewport);
        let interaction = hit_path.last()?;
        let (widget_id, target_id, recognizer) = match interaction {
            HitInteraction::Widget {
                id, interactions, ..
            }
            | HitInteraction::SelectableText {
                id, interactions, ..
            }
            | HitInteraction::Switch {
                id, interactions, ..
            }
            | HitInteraction::Checkbox {
                id, interactions, ..
            }
            | HitInteraction::Radio {
                id, interactions, ..
            }
            | HitInteraction::SelectTrigger {
                id, interactions, ..
            }
            | HitInteraction::TabTrigger {
                id, interactions, ..
            }
            | HitInteraction::Slider {
                id, interactions, ..
            }
            | HitInteraction::TextInput {
                id, interactions, ..
            } => (
                *id,
                HoverTargetId::Widget(*id),
                interactions.gesture.clone(),
            ),
            HitInteraction::SelectOption {
                id,
                option_index,
                interactions,
                ..
            } => (
                *id,
                HoverTargetId::SelectOption {
                    widget_id: *id,
                    option_index: *option_index,
                },
                interactions.gesture.clone(),
            ),
            HitInteraction::Occluder { .. }
            | HitInteraction::Disabled { .. }
            | HitInteraction::CanvasItem { .. } => return None,
        };

        recognizer
            .filter(|gesture| gesture.has_any())
            .map(|recognizer| (widget_id, target_id, recognizer))
    }

    fn detect_gesture_edge(
        &self,
        position: Point,
        viewport: Rect,
        recognizer: &crate::ui::widget::GestureRecognizer<VM>,
    ) -> Option<GestureEdge> {
        let Some((edges, _)) = recognizer.on_edge_swipe.as_ref() else {
            return None;
        };
        if edges.is_empty() {
            return None;
        }
        let band = dp(super::super::EDGE_SWIPE_BAND);
        if edges.contains(GestureEdge::Left) && position.x - viewport.x <= band {
            return Some(GestureEdge::Left);
        }
        if edges.contains(GestureEdge::Right) && viewport.right() - position.x <= band {
            return Some(GestureEdge::Right);
        }
        if edges.contains(GestureEdge::Top) && position.y - viewport.y <= band {
            return Some(GestureEdge::Top);
        }
        if edges.contains(GestureEdge::Bottom) && viewport.bottom() - position.y <= band {
            return Some(GestureEdge::Bottom);
        }
        None
    }

    fn scroll_region_axes_at(&mut self, position: Point) -> (bool, bool) {
        self.scroll_regions()
            .into_iter()
            .rev()
            .find(|region| {
                !region.visible_frame.is_empty() && region.visible_frame.contains(position)
            })
            .map(|region| (region.can_scroll_x(), region.can_scroll_y()))
            .unwrap_or((false, false))
    }

    fn swipe_capture_allowed(
        &self,
        session: &ActiveGestureSession<VM>,
        swipe_axis: SwipeAxis,
        _direction: SwipeDirection,
        _viewport: Rect,
    ) -> bool {
        let Some((configured_axis, _)) = session.recognizer.on_swipe.as_ref() else {
            return false;
        };
        let axis_matches = match configured_axis {
            SwipeAxis::Any => true,
            SwipeAxis::Horizontal => matches!(swipe_axis, SwipeAxis::Horizontal),
            SwipeAxis::Vertical => matches!(swipe_axis, SwipeAxis::Vertical),
        };
        if !axis_matches {
            return false;
        }
        match swipe_axis {
            SwipeAxis::Horizontal => true,
            SwipeAxis::Vertical => !session.scroll_can_y && self.active_touch_scroll.is_none(),
            SwipeAxis::Any => false,
        }
    }
}
