use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn handle_bound_window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        event: WindowEvent,
    ) -> bool {
        match &event {
            WindowEvent::PointerMoved { position, .. }
            | WindowEvent::PointerEntered { position, .. } => {
                self.set_pointer_position(*position);
            }
            _ => {}
        }

        if let WindowEvent::ModifiersChanged(modifiers) = &event {
            self.modifiers = modifiers.state();
        }

        if matches!(event, WindowEvent::PointerLeft { .. }) {
            self.clear_pointer_position();
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        if Self::should_dispatch_widget_event(&event) {
            let viewport = self.viewport_rect();
            let revision_before = self.invalidation.revision();
            let mut needs_redraw = !matches!(
                event,
                WindowEvent::PointerMoved { .. } | WindowEvent::PointerEntered { .. }
            );

            match &event {
                WindowEvent::PointerMoved { .. } | WindowEvent::PointerEntered { .. } => {
                    let finger_id = match &event {
                        WindowEvent::PointerMoved { source, .. } => {
                            Self::gesture_finger_id_from_pointer_source(source)
                        }
                        WindowEvent::PointerEntered { .. } => None,
                        _ => None,
                    };
                    let touch_input_blocked = finger_id
                        .map(|finger_id| self.touch_press_is_blocked_by_active_gesture(finger_id))
                        .unwrap_or(false);
                    needs_redraw |= self.handle_gesture_pointer_move(viewport, finger_id);
                    if touch_input_blocked || self.active_pinch.is_some() {
                    } else if self.active_touch_scroll.is_some() {
                        needs_redraw |= self.handle_touch_scroll_drag();
                    } else if self.active_scrollbar_drag.is_some() {
                        needs_redraw |= self.handle_scrollbar_drag();
                        needs_redraw |= self.sync_scrollbar_hover();
                        needs_redraw |= self.update_cursor_icon();
                    } else if self.active_slider_drag.is_some() {
                        needs_redraw |= self.handle_slider_drag();
                        needs_redraw |= self.handle_hover(viewport);
                        needs_redraw |= self.update_cursor_icon();
                    } else if self.active_data_grid_column_resize.is_some() {
                        needs_redraw |= self.handle_data_grid_column_resize();
                        needs_redraw |= self.handle_hover(viewport);
                        needs_redraw |= self.update_cursor_icon();
                    } else if self.active_canvas_drag.is_some() {
                        needs_redraw |= self.handle_canvas_drag();
                        needs_redraw |= self.handle_hover(viewport);
                    } else if self.active_text_selection.is_some() {
                        needs_redraw |= self.handle_text_selection_drag();
                        needs_redraw |= self.handle_hover(viewport);
                    } else {
                        needs_redraw |= self.handle_hover(viewport);
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    needs_redraw |= self.handle_mouse_wheel(*delta);
                }
                WindowEvent::PointerButton {
                    state: ElementState::Pressed,
                    position,
                    button,
                    ..
                } => {
                    self.set_pointer_position(*position);
                    let is_touch = matches!(button, ButtonSource::Touch { .. });
                    let touch_finger_id = Self::gesture_finger_id_from_button(button);
                    let canvas_button = canvas_mouse_button(button.clone().mouse_button());
                    needs_redraw |= self.begin_gesture_session(viewport, Instant::now(), button);
                    if is_touch
                        && touch_finger_id
                            .map(|finger_id| {
                                self.touch_press_is_blocked_by_active_gesture(finger_id)
                            })
                            .unwrap_or(false)
                    {
                    } else if button.clone().mouse_button() == Some(MouseButton::Left)
                        && self.begin_scrollbar_drag()
                    {
                        needs_redraw = true;
                        needs_redraw |= self.update_cursor_icon();
                    } else if let Some(canvas_button) = canvas_button {
                        if is_touch {
                            if !self.begin_touch_scroll_drag(viewport) {
                                self.handle_mouse_press(viewport, Instant::now(), canvas_button);
                            }
                        } else {
                            self.handle_mouse_press(viewport, Instant::now(), canvas_button);
                        }
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    needs_redraw |= self.handle_platform_keyboard_input(event);
                }
                _ => {}
            }

            if self.invalidation.revision() != revision_before {
                needs_redraw = true;
            }

            if needs_redraw {
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
        }

        if let Some(window_command) = self
            .commands
            .iter()
            .find(|entry| entry.trigger.matches(&event))
            .cloned()
        {
            self.execute_command(&window_command.command);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        if self.drain_window_requests() {
            return true;
        }

        match event {
            WindowEvent::CloseRequested => {
                return self.close_policy() == super::super::WindowClosePolicy::Close
            }
            WindowEvent::Focused(false) => {
                self.update_accessibility_window_focus_state(false);
                self.deferred_mouse_click = None;
                let _ = self.end_gesture_session(None, true);
                self.end_scrollbar_drag();
                self.end_touch_scroll_drag();
                self.end_slider_drag();
                self.end_canvas_drag();
                self.end_data_grid_column_resize();
                self.active_data_grid_column_reorder = None;
                self.pressed_widget = None;
                self.update_focus(None, None, false);
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::Focused(true) => {
                self.update_accessibility_window_focus_state(true);
            }
            WindowEvent::ThemeChanged(theme) => {
                self.apply_window_theme(Some(theme));
                self.sync_bindings(Instant::now());
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.apply_window_theme(None);
                self.invalidate_scene_with_reason("window_scale_factor_changed");
                if let Some(window) = self.window.as_ref() {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.resize(window.surface_size(), window.scale_factor() as f32);
                    }
                    window.request_redraw();
                }
            }
            WindowEvent::Ime(event) => {
                let needs_redraw = self.handle_ime_event(&event);
                if needs_redraw {
                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::PointerButton {
                state: ElementState::Released,
                position,
                button,
                ..
            } => {
                self.set_pointer_position(position);
                let touch_scroll_was_active = self.active_touch_scroll.is_some();
                let is_touch = matches!(button, ButtonSource::Touch { .. });
                let is_mouse = matches!(button, ButtonSource::Mouse(_));
                let gesture_finger = Self::gesture_finger_id_from_button(&button);
                let touch_press_blocked = is_touch
                    && gesture_finger
                        .map(|finger_id| self.touch_press_is_blocked_by_active_gesture(finger_id))
                        .unwrap_or(false);
                let gesture_consumed = self.gesture_consumes_click() || touch_press_blocked;
                if is_touch && touch_scroll_was_active && !touch_press_blocked {
                    let _ = self.handle_touch_scroll_drag();
                }
                let touch_scroll_activated = self
                    .active_touch_scroll
                    .as_ref()
                    .map(|drag| drag.activated)
                    .unwrap_or(false);
                if let Some(canvas_button) = canvas_mouse_button(button.clone().mouse_button()) {
                    if is_touch && touch_scroll_was_active && !touch_scroll_activated {
                        if !gesture_consumed {
                            self.handle_mouse_press(
                                self.viewport_rect(),
                                Instant::now(),
                                canvas_button,
                            );
                        }
                    } else if !is_touch && !gesture_consumed {
                        self.handle_canvas_mouse_release(canvas_button);
                    } else if !touch_scroll_was_active && !gesture_consumed {
                        self.handle_canvas_mouse_release(canvas_button);
                    }
                }
                let _ = self.end_gesture_session(gesture_finger, false);
                if is_mouse && !gesture_consumed {
                    if let Some(deferred) = self.deferred_mouse_click.take() {
                        if self.pressed_widget == Some(deferred.widget_id) {
                            if let Some(handler) = deferred.click_handler {
                                self.execute_click_handler(&handler, self.cursor_position);
                            } else {
                                self.dispatch_widget_click(
                                    HoverTargetId::Widget(deferred.widget_id),
                                    deferred.interactions,
                                    Instant::now(),
                                );
                            }
                        }
                    }
                } else if is_touch && !gesture_consumed && !touch_scroll_activated {
                    if let Some(deferred) = self.deferred_mouse_click.take() {
                        if self.pressed_widget == Some(deferred.widget_id) {
                            if let Some(handler) = deferred.click_handler {
                                self.execute_click_handler(&handler, self.cursor_position);
                            } else {
                                self.dispatch_widget_click(
                                    HoverTargetId::Widget(deferred.widget_id),
                                    deferred.interactions,
                                    Instant::now(),
                                );
                            }
                        }
                    }
                } else {
                    self.deferred_mouse_click = None;
                }
                if matches!(
                    canvas_mouse_button(button.clone().mouse_button()),
                    Some(CanvasMouseButton::Left)
                ) {
                    let _ = self.finish_tab_reorder();
                    let _ = self.finish_data_grid_column_reorder();
                } else {
                    self.active_tab_reorder = None;
                    self.active_data_grid_column_reorder = None;
                }
                self.end_scrollbar_drag();
                self.end_touch_scroll_drag();
                self.end_slider_drag();
                self.end_data_grid_column_resize();
                self.pressed_widget = None;
                self.end_text_selection_drag();
                self.handle_hover(self.viewport_rect());
                self.update_cursor_icon();
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::SurfaceResized(size) => {
                self.invalidate_scene_with_reason("window_surface_resized");
                if let Some(renderer) = self.renderer.as_mut() {
                    let scale_factor = self
                        .window
                        .as_ref()
                        .map(|window| window.scale_factor() as f32)
                        .unwrap_or(1.0);
                    renderer.resize(size, scale_factor);
                }

                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::PointerLeft { kind, .. } => {
                self.deferred_mouse_click = None;
                let gesture_finger = match kind {
                    crate::platform::event::PointerKind::Touch(finger_id) => Some(finger_id),
                    _ => None,
                };
                let _ = self.end_gesture_session(gesture_finger, true);
                self.end_touch_scroll_drag();
            }
            WindowEvent::RedrawRequested => match self.render_current_frame() {
                Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                Ok(RenderStatus::ReconfigureSurface) => {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.reconfigure();
                    }
                    match self.render_current_frame() {
                        Ok(RenderStatus::Rendered | RenderStatus::SkipFrame) => {}
                        Ok(RenderStatus::ReconfigureSurface) => {}
                        Err(error) => self.fail(event_loop, error),
                    }
                }
                Err(error) => self.fail(event_loop, error),
            },
            _ => {}
        }

        false
    }

    pub(in crate::runtime) fn handle_bound_about_to_wait(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
    ) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let now = Instant::now();
        let accessibility_actions_handled = self.drain_accessibility_actions();
        if accessibility_actions_handled {
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        let repeated_key_handled = self.drive_key_repeat(now);
        if repeated_key_handled {
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        let flush_started_at = Instant::now();
        let flush_outcome = self.flush_pending_text_input_changes();
        let flush_duration = flush_started_at.elapsed();
        if flush_outcome.requires_global_invalidation {
            self.invalidation.mark_dirty();
        }
        let theme_started_at = Instant::now();
        let theme_changed = self.refresh_platform_theme();
        let theme_duration = theme_started_at.elapsed();
        if self.invalidation.take_redraw_request() {
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        if theme_changed {
            self.sync_bindings(now);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        let redraw_started_at = Instant::now();
        self.request_redraw_if_dirty(now);
        let redraw_duration = redraw_started_at.elapsed();
        let lifecycle_started_at = Instant::now();
        self.dispatch_lifecycle_events_if_needed();
        let lifecycle_duration = lifecycle_started_at.elapsed();
        let animation_duration = {
            let animation_started_at = Instant::now();
            self.drive_animations(event_loop, now);
            animation_started_at.elapsed()
        };
        let close_requested = self.drain_window_requests();
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_about_to_wait",
                started_at.elapsed(),
                format!(
                    "repeated_key_handled={} flushed_text_changes={} flush_ms={:.3} lifecycle_ms={:.3} theme_changed={} theme_ms={:.3} redraw_ms={:.3} animation_ms={:.3} close_requested={}",
                    repeated_key_handled,
                    flush_outcome.changed,
                    flush_duration.as_secs_f64() * 1000.0,
                    lifecycle_duration.as_secs_f64() * 1000.0,
                    theme_changed,
                    theme_duration.as_secs_f64() * 1000.0,
                    redraw_duration.as_secs_f64() * 1000.0,
                    animation_duration.as_secs_f64() * 1000.0,
                    close_requested,
                ),
            );
        }
        close_requested
    }
}
