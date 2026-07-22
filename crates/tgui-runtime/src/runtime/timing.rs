use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn set_pointer_position(&mut self, position: PhysicalPosition<f64>) {
        let logical = self
            .window
            .as_ref()
            .map(|window| position.to_logical::<f32>(window.scale_factor()))
            .unwrap_or_else(|| position.to_logical::<f32>(1.0));
        self.cursor_position = Some(Point {
            x: dp(logical.x),
            y: dp(logical.y),
        });
    }

    pub(in crate::runtime) fn physical_cursor_position(&self) -> Option<PhysicalPosition<f64>> {
        let cursor = self.cursor_position?;
        let scale_factor = self
            .window
            .as_ref()
            .map(|window| window.scale_factor())
            .unwrap_or(1.0);
        Some(PhysicalPosition::new(
            f64::from(cursor.x.get()) * scale_factor,
            f64::from(cursor.y.get()) * scale_factor,
        ))
    }

    pub(in crate::runtime) fn unit_context(&self) -> UnitContext {
        let scale_factor = self
            .window
            .as_ref()
            .map(|window| window.scale_factor() as f32)
            .unwrap_or(1.0);
        let font_scale = self.platform_font_scale();
        UnitContext::new(scale_factor, font_scale)
    }

    fn platform_font_scale(&self) -> f32 {
        1.0
    }

    pub(in crate::runtime) fn clear_pointer_position(&mut self) {
        let previous_position = self.cursor_position;
        self.cursor_position = None;
        self.hover_popover_anchor = None;
        let had_hovered_widgets = !self.hovered_widgets.is_empty();
        let previous_scrollbar = self.hovered_scrollbar;
        self.button_hover_patch_pending = None;
        self.button_pressed_patch_pending = None;
        if had_hovered_widgets {
            self.button_hover_patch_pending = self.retained_button_hover_patch_candidate(&[]);
            self.row_hover_patch_pending = self.retained_row_hover_patch_candidate(&[]);
        }
        for hovered in std::mem::take(&mut self.hovered_widgets).into_iter().rev() {
            if let Some(command) = hovered.on_mouse_leave {
                self.execute_hover_transition_handler(&command, previous_position);
            }
        }
        self.hovered_scrollbar = self.active_scrollbar_drag.map(|drag| drag.handle);
        if had_hovered_widgets || self.hovered_scrollbar != previous_scrollbar {
            self.hover_epoch = self.hover_epoch.wrapping_add(1);
        }
        self.update_cursor_icon();
    }

    pub(in crate::runtime) fn drive_animations(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        now: Instant,
    ) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        self.flush_pending_click_if_due(now);
        let _ = self.flush_pending_long_press_if_due(now);
        let _ = self.flush_tooltip_release_if_due(now);

        let mut frame_advanced = false;
        let mut smooth_scroll_advanced = false;
        let mut touch_scroll_inertia_advanced = false;
        if self.settle_scroll_motion_for_reduced_motion() {
            frame_advanced = true;
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        let initial_clock_active = self.frame_clock_sources_active();
        let clock_was_armed = self.frame_clock.is_armed();
        let toast_clock_active = self.next_toast_wakeup_deadline.is_some_and(|deadline| {
            deadline <= now + self.frame_clock.interval().saturating_mul(2)
        });
        if initial_clock_active && !clock_was_armed {
            // Refresh once when motion starts after an idle period so a display
            // mode change is observed before the first sampled frame.
            self.refresh_frame_clock_from_monitor(now, true);
        } else if clock_was_armed || toast_clock_active {
            self.refresh_frame_clock_from_monitor(now, false);
        }
        self.frame_clock.set_active(initial_clock_active, now);
        let frame_clock_due = self.frame_clock.consume_due_tick(now);
        // tooltip 唤醒到点：invalidate scene 让下一帧 collect 看到 elapsed >= delay。
        if let Some(deadline) = self.next_tooltip_wakeup_deadline {
            if deadline <= now {
                self.next_tooltip_wakeup_deadline = None;
                self.invalidate_computed_scene();
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
                frame_advanced = true;
            }
        }
        if let Some(deadline) = self.next_toast_wakeup_deadline {
            if deadline <= now {
                self.next_toast_wakeup_deadline = None;
                self.invalidate_computed_scene_for_toast_motion();
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
                frame_advanced = true;
            }
        }
        if frame_clock_due {
            if self.advance_smooth_scroll_at(now) {
                frame_advanced = true;
                smooth_scroll_advanced = true;
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            if self.advance_touch_scroll_inertia(now) {
                frame_advanced = true;
                touch_scroll_inertia_advanced = true;
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
        }
        if self.drive_carousel_auto_play(now) {
            frame_advanced = true;
        }
        if self
            .media_manager
            .advance_animations_for_keys(self.active_media_texture_keys(), now)
        {
            frame_advanced = true;
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        // Controller ticks run synchronously on the event-loop thread. Their AnimatedValue writes
        // still advance revisions and reactive dirty queues, but do not need to enqueue a second
        // proxy user event for every track; the redraw/invalidation drain below handles the batch.
        let controller_revision_before = self.invalidation.revision();
        let controller_frame = {
            let _wake_guard = self.invalidation.suppress_wakeups_without_dispatch();
            self.animations.refresh(now, frame_clock_due)
        };
        let controller_changed = controller_frame.changed;
        let controller_invalidated = self.invalidation.revision() != controller_revision_before;
        if controller_changed || controller_invalidated {
            frame_advanced = true;
            // AnimatedValue tracks concrete scene/layout dependencies. Consume the invalidation
            // produced by the controller tick before requesting the redraw so the retained scene
            // can patch only the affected targets instead of rebuilding the whole tree.
            self.request_redraw_if_dirty(now);
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        let animation_refresh = if frame_clock_due {
            self.animation_engine.refresh(now)
        } else {
            Default::default()
        };
        if animation_refresh.changed {
            self.toast_motion_patch_pending = false;
            frame_advanced = true;
            // A clear-color animation only touches the renderer's surface state. Keep the
            // retained scene valid in that case; theme/widget channels still advance the scene
            // epochs and use their bounded patch/full-recollect fallback below.
            if animation_refresh.scene_changed() {
                self.animation_epoch = self.animation_epoch.wrapping_add(1);
                if animation_refresh.accessibility_geometry_changed {
                    self.accessibility_animation_epoch =
                        self.accessibility_animation_epoch.wrapping_add(1);
                }
                if animation_refresh.layout_changed {
                    self.layout_animation_epoch = self.layout_animation_epoch.wrapping_add(1);
                }
                let patched = self.patch_animation_refresh(&animation_refresh, now);
                if !patched {
                    self.invalidate_computed_scene();
                }
            }
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        let frame_clock_active = self.animation_engine.has_active_animations()
            || controller_frame.active
            || (!self.reduced_motion
                && (!self.smooth_scroll_states.is_empty()
                    || !self.touch_scroll_inertia_states.is_empty()));
        let animation_deadline = self.frame_clock.set_active(frame_clock_active, now);
        let media_deadline = self.next_media_animation_deadline();
        let click_deadline = self.pending_click_deadline();
        let gesture_deadline = self
            .active_gesture
            .as_ref()
            .and_then(|gesture| gesture.long_press_deadline);
        let tooltip_release_deadline = self.tooltip_state.long_press_release_deadline;
        let caret_deadline = self.next_caret_blink_deadline(now);
        let key_repeat_deadline = self.next_key_repeat_deadline();
        let carousel_deadline = self.next_carousel_wakeup_deadline;
        let tooltip_deadline = self.next_tooltip_wakeup_deadline;
        let toast_deadline = self.next_toast_wakeup_deadline;
        let next_deadline = super::handler_support::earliest_deadline([
            animation_deadline,
            media_deadline,
            click_deadline,
            gesture_deadline,
            tooltip_release_deadline,
            caret_deadline,
            key_repeat_deadline,
            tooltip_deadline,
            toast_deadline,
            carousel_deadline,
        ]);

        self.set_control_flow_for_deadline(event_loop, next_deadline, now);

        if let Some(started_at) = started_at {
            if animation_refresh.changed || animation_deadline.is_some() {
                log_text_profile(
                    "textarea_animation_keys",
                    started_at.elapsed(),
                    format!(
                        "changed={} layout_changed={} active_keys={}",
                        animation_refresh.changed,
                        animation_refresh.layout_changed,
                        self.animation_engine.active_keys_summary(),
                    ),
                );
            }
            log_text_profile(
                "textarea_animation",
                started_at.elapsed(),
                format!(
                    "smooth_scroll_advanced={} touch_scroll_inertia_advanced={} controller_changed={} engine_changed={} engine_layout_changed={} frame_advanced={} animation_active={} controller_active={} media_active={} pending_click={} gesture_deadline={} tooltip_release_deadline={} caret_deadline={} key_repeat_deadline={} carousel_deadline={} smooth_scroll_deadline={} touch_scroll_inertia_deadline={} next_deadline={}",
                    smooth_scroll_advanced,
                    touch_scroll_inertia_advanced,
                    controller_changed,
                    animation_refresh.changed,
                    animation_refresh.layout_changed,
                    frame_advanced,
                    animation_deadline.is_some(),
                    controller_frame.active,
                    media_deadline.is_some(),
                    click_deadline.is_some(),
                    gesture_deadline.is_some(),
                    tooltip_release_deadline.is_some(),
                    caret_deadline.is_some(),
                    key_repeat_deadline.is_some(),
                    carousel_deadline.is_some(),
                    !self.smooth_scroll_states.is_empty(),
                    !self.touch_scroll_inertia_states.is_empty(),
                    next_deadline.is_some(),
                ),
            );
        }

        frame_advanced
    }

    pub(in crate::runtime) fn animated_theme(&mut self, now: Instant) -> Theme {
        if self.theme_colors_settled() {
            return self.theme.clone();
        }
        let transition =
            (!self.reduced_motion).then(|| theme_transition(self.theme.motion.normal_ms));
        let mut theme = self.theme.clone();
        theme.colors.background = self.resolve_theme_color(
            WindowProperty::ThemeBackground,
            theme.colors.background,
            transition,
            now,
        );
        theme.colors.surface = self.resolve_theme_color(
            WindowProperty::ThemeSurface,
            theme.colors.surface,
            transition,
            now,
        );
        theme.colors.surface_low = self.resolve_theme_color(
            WindowProperty::ThemeSurfaceLow,
            theme.colors.surface_low,
            transition,
            now,
        );
        theme.colors.surface_high = self.resolve_theme_color(
            WindowProperty::ThemeSurfaceHigh,
            theme.colors.surface_high,
            transition,
            now,
        );
        theme.colors.primary = self.resolve_theme_color(
            WindowProperty::ThemePrimary,
            theme.colors.primary,
            transition,
            now,
        );
        theme.colors.on_surface = self.resolve_theme_color(
            WindowProperty::ThemeOnSurface,
            theme.colors.on_surface,
            transition,
            now,
        );
        theme.colors.on_surface_muted = self.resolve_theme_color(
            WindowProperty::ThemeOnSurfaceMuted,
            theme.colors.on_surface_muted,
            transition,
            now,
        );
        theme.colors.primary_container = self.resolve_theme_color(
            WindowProperty::ThemePrimaryContainer,
            theme.colors.primary_container,
            transition,
            now,
        );
        theme.colors.focus_ring = self.resolve_theme_color(
            WindowProperty::ThemeFocusRing,
            theme.colors.focus_ring,
            transition,
            now,
        );
        theme.colors.selection = self.resolve_theme_color(
            WindowProperty::ThemeSelection,
            theme.colors.selection,
            transition,
            now,
        );
        theme
    }

    pub(in crate::runtime) fn theme_background_color(&mut self, now: Instant) -> Color {
        if self.window_color_settled_at(
            WindowProperty::ThemeBackground,
            self.theme.colors.background,
        ) {
            self.theme.colors.background
        } else {
            self.resolve_theme_color(
                WindowProperty::ThemeBackground,
                self.theme.colors.background,
                (!self.reduced_motion).then(|| theme_transition(self.theme.motion.normal_ms)),
                now,
            )
        }
    }

    fn theme_colors_settled(&self) -> bool {
        [
            (
                WindowProperty::ThemeBackground,
                self.theme.colors.background,
            ),
            (WindowProperty::ThemeSurface, self.theme.colors.surface),
            (
                WindowProperty::ThemeSurfaceLow,
                self.theme.colors.surface_low,
            ),
            (
                WindowProperty::ThemeSurfaceHigh,
                self.theme.colors.surface_high,
            ),
            (WindowProperty::ThemePrimary, self.theme.colors.primary),
            (WindowProperty::ThemeOnSurface, self.theme.colors.on_surface),
            (
                WindowProperty::ThemeOnSurfaceMuted,
                self.theme.colors.on_surface_muted,
            ),
            (
                WindowProperty::ThemePrimaryContainer,
                self.theme.colors.primary_container,
            ),
            (WindowProperty::ThemeFocusRing, self.theme.colors.focus_ring),
            (WindowProperty::ThemeSelection, self.theme.colors.selection),
        ]
        .into_iter()
        .all(|(property, target)| self.window_color_settled_at(property, target))
    }
}
