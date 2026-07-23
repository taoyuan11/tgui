use super::*;

const FRAME_CLOCK_REFRESH_PROBE_INTERVAL: Duration = Duration::from_millis(250);

impl<VM: 'static> BoundRuntimeHandler<VM> {
    fn idle_redraw_eligible(&self) -> bool {
        !self.window_occluded
            && !self.surface_occluded
            && self.renderer.is_some()
            && self.window.as_ref().is_some_and(|window| {
                let size = window.surface_size();
                size.width > 0 && size.height > 0
            })
    }

    pub(super) fn arm_idle_redraw(&mut self, now: Instant) {
        self.next_idle_redraw_deadline = self
            .idle_redraw_eligible()
            .then(|| now.checked_add(IDLE_REDRAW_INTERVAL).unwrap_or(now));
    }

    pub(super) fn disarm_idle_redraw(&mut self) {
        self.next_idle_redraw_deadline = None;
    }

    pub(super) fn update_idle_redraw_after_render(&mut self, status: &RenderStatus, now: Instant) {
        match status {
            RenderStatus::Rendered => {
                self.surface_occluded = false;
                self.arm_idle_redraw(now);
            }
            RenderStatus::ReconfigureSurface | RenderStatus::SkipFrame => {}
        }
    }

    pub(super) fn mark_surface_occluded(&mut self) {
        self.surface_occluded = true;
        self.disarm_idle_redraw();
    }

    pub(super) fn set_window_occluded(&mut self, occluded: bool) {
        if self.window_occluded == occluded {
            if !occluded && self.surface_occluded {
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            return;
        }

        self.window_occluded = occluded;
        if occluded {
            self.disarm_idle_redraw();
            return;
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    pub(super) fn drive_idle_redraw(&mut self, now: Instant) -> bool {
        let eligible = self.idle_redraw_eligible();
        self.drive_idle_redraw_inner(now, eligible)
    }

    pub(super) fn drive_idle_redraw_inner(&mut self, now: Instant, eligible: bool) -> bool {
        if !eligible {
            self.disarm_idle_redraw();
            return false;
        }

        let Some(deadline) = self.next_idle_redraw_deadline else {
            return false;
        };
        if deadline > now {
            return false;
        }

        self.next_idle_redraw_deadline = Some(next_periodic_deadline_after(
            deadline,
            IDLE_REDRAW_INTERVAL,
            now,
        ));
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
            return true;
        }
        false
    }

    pub(super) fn refresh_frame_clock_from_monitor(&mut self, now: Instant, force: bool) -> bool {
        if !force
            && self.last_frame_clock_probe.is_some_and(|last| {
                now.saturating_duration_since(last) < FRAME_CLOCK_REFRESH_PROBE_INTERVAL
            })
        {
            return false;
        }

        self.last_frame_clock_probe = Some(now);
        let refresh_rate_millihertz = self
            .window
            .as_ref()
            .and_then(|window| window.current_monitor())
            .and_then(|monitor| monitor.refresh_rate_millihertz());
        let changed = self
            .frame_clock
            .update_refresh_rate(refresh_rate_millihertz, now);
        if changed {
            if let Some(layout) = self
                .cached_scene
                .as_mut()
                .and_then(|cached| cached.layout.as_mut())
            {
                layout.set_frame_clock(self.frame_clock.snapshot());
            }
            // A retained Toast frame may have been collected on the old cadence.
            // Recollect once so its nested overlay/portal deadline uses the new clock.
            self.next_toast_wakeup_deadline = None;
            self.invalidate_computed_scene();
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        changed
    }

    pub(super) fn resolve_theme_color(
        &mut self,
        property: WindowProperty,
        target: Color,
        transition: Option<Transition>,
        now: Instant,
    ) -> Color {
        self.animation_engine
            .resolve_color(AnimationKey::Window(property), target, transition, now)
    }

    pub(super) fn window_color_settled_at(&self, property: WindowProperty, target: Color) -> bool {
        self.animation_engine
            .color_settled_at(AnimationKey::Window(property), target)
    }

    pub(super) fn frame_clock_sources_active(&self) -> bool {
        self.animation_engine.has_active_animations()
            || self.animations.has_active_controllers()
            || (!self.reduced_motion
                && (!self.smooth_scroll_states.is_empty()
                    || !self.touch_scroll_inertia_states.is_empty()))
    }

    pub(super) fn next_deadline(&mut self, now: Instant) -> Option<Instant> {
        let frame_clock_active = self.frame_clock_sources_active();
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
        let tooltip_deadline = self.next_tooltip_wakeup_deadline;
        let toast_deadline = self.next_toast_wakeup_deadline;
        let carousel_deadline = self.next_carousel_wakeup_deadline;
        let idle_redraw_deadline = self.next_idle_redraw_deadline;
        earliest_deadline([
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
            idle_redraw_deadline,
        ])
    }

    pub(super) fn set_control_flow_for_deadline(
        &self,
        event_loop: &dyn ActiveEventLoop,
        deadline: Option<Instant>,
        now: Instant,
    ) {
        if let Some(deadline) = deadline {
            // During the animation tick, stale deadlines have already fired and must be
            // replaced. A future deadline that is earlier than this one belongs to the
            // current frame clock and should keep its original cadence.
            if let ControlFlow::WaitUntil(existing) = event_loop.control_flow() {
                if existing > now && existing <= deadline {
                    return;
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            match event_loop.control_flow() {
                // A future wake may belong to another window. Multi-window
                // handlers share one event loop, so an idle window must not
                // erase an active sibling's earlier deadline.
                ControlFlow::WaitUntil(existing) if existing > now => {}
                ControlFlow::Wait => {}
                ControlFlow::WaitUntil(_) | ControlFlow::Poll => {
                    event_loop.set_control_flow(ControlFlow::Wait);
                }
            }
        }
    }

    pub(super) fn schedule_next_deadline(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        now: Instant,
    ) -> Option<Instant> {
        let deadline = self.next_deadline(now);
        self.set_control_flow_for_deadline_after_redraw(event_loop, deadline);
        deadline
    }

    fn set_control_flow_for_deadline_after_redraw(
        &self,
        event_loop: &dyn ActiveEventLoop,
        deadline: Option<Instant>,
    ) {
        if let Some(deadline) = deadline {
            // Redraw happens after the tick has already scheduled the next wakeup.
            // Replacing it here with `Instant::now() + frame_interval` shifts every
            // animation frame by the render cost, which is visible in debug builds.
            if let ControlFlow::WaitUntil(existing) = event_loop.control_flow() {
                if existing <= deadline {
                    return;
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            match event_loop.control_flow() {
                ControlFlow::WaitUntil(existing) if existing > Instant::now() => {}
                ControlFlow::Wait => {}
                ControlFlow::WaitUntil(_) | ControlFlow::Poll => {
                    event_loop.set_control_flow(ControlFlow::Wait);
                }
            }
        }
    }

    pub(super) fn active_media_texture_keys(
        &self,
    ) -> impl Iterator<Item = &crate::media::MediaTextureKey> {
        self.cached_scene
            .as_ref()
            .into_iter()
            .flat_map(|cached| cached.media_texture_bindings.keys())
    }

    pub(super) fn next_media_animation_deadline(&self) -> Option<Instant> {
        self.media_manager
            .next_animation_deadline_for_keys(self.active_media_texture_keys())
    }

    pub(super) fn text_edit_state(&self, id: WidgetId) -> Option<&TextEditState> {
        self.text_edit_states.get(&id)
    }

    pub(super) fn default_text_edit_state(&self, widget_id: WidgetId, text: &str) -> TextEditState {
        let scroll_offset = self
            .scroll_states
            .get(&widget_id)
            .copied()
            .unwrap_or(Point::ZERO);
        TextEditState {
            scroll_x: scroll_offset.x,
            scroll_y: scroll_offset.y,
            ..TextEditState::caret_at(text)
        }
    }

    pub(super) fn update_text_edit_state(
        &mut self,
        widget_id: WidgetId,
        text: &str,
        update: impl FnOnce(&mut TextEditState),
    ) -> bool {
        let default_state = self.default_text_edit_state(widget_id, text);
        let state = self
            .text_edit_states
            .entry(widget_id)
            .and_modify(|state| *state = state.clone().clamped_to(text))
            .or_insert(default_state);
        let before = state.clone();
        update(state);
        *state = state.clone().clamped_to(text);
        if *state == before {
            return false;
        }
        self.invalidate_text_input_scene();
        true
    }

    pub(super) fn window_id(&self) -> Option<WindowId> {
        self.window_id
    }

    pub(super) fn create_or_resume_surface(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        modal_parent: Option<&Arc<dyn Window>>,
    ) {
        self.set_dialog_proxy(event_loop);

        if self.window.is_some() && self.renderer.is_some() {
            return;
        }

        if self.window.is_some() {
            self.resume_existing_window(event_loop);
            return;
        }

        let mut attributes = WindowAttributes::default()
            .with_transparent(true)
            .with_decorations(self.config.decorations)
            .with_title(self.config.title.clone())
            .with_inner_size(self.config.size)
            .with_visible(false);

        #[cfg(target_os = "windows")]
        if self.config.clear_color.a < 255 {
            use winit::platform::windows::WindowAttributesExtWindows;

            attributes = attributes.with_no_redirection_bitmap(true);
        }

        if let Some(min_size) = self.config.min_size {
            attributes = attributes.with_min_inner_size(min_size);
        }

        if let Some(max_size) = self.config.max_size {
            attributes = attributes.with_max_inner_size(max_size);
        }

        if let Some(position) = default_window_position(event_loop, self.config.size) {
            attributes = attributes.with_position(position);
        }

        if let Some(icon_bytes) = self.config.window_icon {
            match image::load_from_memory(icon_bytes) {
                Ok(image) => {
                    let (w, h) = image.dimensions();
                    let rgba = image.into_rgba8().into_raw();

                    match Icon::from_rgba(rgba, w, h) {
                        Ok(ok) => {
                            attributes = attributes.with_window_icon(Some(ok));
                        }
                        Err(err) => {
                            self.fail(event_loop, TguiError::Icon(err.to_string()));
                        }
                    }
                }
                Err(err) => {
                    self.fail(event_loop, TguiError::Icon(err.to_string()));
                }
            }
        }

        if self.blocks_main_window() {
            if let Some(parent) = modal_parent {
                attributes = configure_native_modal_window(attributes, parent.as_ref());
            }
        }

        let window: Arc<dyn Window> = match event_loop.create_window(attributes) {
            Ok(window) => Arc::from(window),
            Err(error) => {
                self.fail(event_loop, error.into());
                return;
            }
        };

        let window_theme = resolve_window_theme(Some(window.as_ref()));
        self.theme = resolve_theme(
            &self.active_theme_selection(),
            &self.active_theme_set(),
            window_theme,
        );
        let clear_color =
            if self.window_bindings.clear_color.is_some() || self.config.clear_color_overridden {
                self.config.clear_color
            } else {
                self.theme.colors.background
            };

        let renderer = match Renderer::new(window.clone(), clear_color, self.config.msaa) {
            Ok(renderer) => renderer,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };

        self.window_id = Some(window.id());
        self.gpu_scroll_supported = renderer.push_constants_supported();
        self.renderer = Some(renderer);
        self.last_synced_clear_color = Some(clear_color);
        self.window = Some(window);
        self.refresh_frame_clock_from_monitor(Instant::now(), true);
        self.initialize_platform_window_binding_state(window_theme);
        self.initialize_accessibility_adapter();
        #[cfg(feature = "video")]
        self.notify_video_surface_restored();

        if !self.render_hidden_frame(event_loop) {
            return;
        }

        #[cfg(feature = "video")]
        self.notify_video_app_foreground();

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
            window.set_visible(true);
        }
    }
}

pub(super) fn earliest_deadline<const N: usize>(
    deadlines: [Option<Instant>; N],
) -> Option<Instant> {
    deadlines.into_iter().flatten().min()
}

fn next_periodic_deadline_after(deadline: Instant, interval: Duration, now: Instant) -> Instant {
    let interval_nanos = interval.as_nanos().max(1);
    let elapsed_nanos = now.saturating_duration_since(deadline).as_nanos();
    let periods = elapsed_nanos / interval_nanos + 1;
    let delta_nanos = interval_nanos
        .saturating_mul(periods)
        .min(u128::from(u64::MAX));
    deadline
        .checked_add(Duration::from_nanos(delta_nanos as u64))
        .or_else(|| now.checked_add(interval))
        .unwrap_or(now)
}
