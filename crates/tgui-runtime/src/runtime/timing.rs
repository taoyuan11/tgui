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
                self.invalidate_computed_scene();
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
                frame_advanced = true;
            }
        }
        if self.advance_smooth_scroll() {
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
        let frame_clock_due = match event_loop.control_flow() {
            ControlFlow::WaitUntil(deadline) => deadline <= now,
            ControlFlow::Poll | ControlFlow::Wait => true,
        };
        let controller_changed = frame_clock_due && self.animations.refresh(now);
        if controller_changed {
            frame_advanced = true;
            self.animation_epoch = self.animation_epoch.wrapping_add(1);
            self.layout_animation_epoch = self.layout_animation_epoch.wrapping_add(1);
            self.invalidate_computed_scene();
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
            frame_advanced = true;
            self.animation_epoch = self.animation_epoch.wrapping_add(1);
            if animation_refresh.layout_changed {
                self.layout_animation_epoch = self.layout_animation_epoch.wrapping_add(1);
            }
            let patched = if animation_refresh.layout_widget_ids.is_empty()
                && !animation_refresh.scene_widget_ids.is_empty()
            {
                self.patch_animation_scene_widgets(&animation_refresh.scene_widget_ids, now)
            } else {
                false
            };
            if !patched {
                self.invalidate_computed_scene();
            }
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }

        let animation_deadline = self.animation_engine.next_frame_deadline(now);
        let controller_deadline = self.animations.next_frame_deadline(now);
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
        let smooth_scroll_deadline =
            (!self.smooth_scroll_states.is_empty()).then_some(now + Duration::from_millis(16));
        let touch_scroll_inertia_deadline = (!self.touch_scroll_inertia_states.is_empty())
            .then_some(now + super::TOUCH_SCROLL_INERTIA_FRAME);
        let tooltip_deadline = self.next_tooltip_wakeup_deadline;
        let toast_deadline = self.next_toast_wakeup_deadline;
        let next_deadline = super::handler_support::earliest_deadline([
            animation_deadline,
            controller_deadline,
            media_deadline,
            click_deadline,
            gesture_deadline,
            tooltip_release_deadline,
            caret_deadline,
            key_repeat_deadline,
            smooth_scroll_deadline,
            touch_scroll_inertia_deadline,
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
                    controller_deadline.is_some(),
                    media_deadline.is_some(),
                    click_deadline.is_some(),
                    gesture_deadline.is_some(),
                    tooltip_release_deadline.is_some(),
                    caret_deadline.is_some(),
                    key_repeat_deadline.is_some(),
                    carousel_deadline.is_some(),
                    smooth_scroll_deadline.is_some(),
                    touch_scroll_inertia_deadline.is_some(),
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
        let transition = Some(default_theme_transition());
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
                Some(default_theme_transition()),
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
