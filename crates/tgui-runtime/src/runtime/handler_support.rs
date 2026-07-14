use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
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

    pub(super) fn next_deadline(&self, now: Instant) -> Option<Instant> {
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
        let smooth_scroll_deadline =
            (!self.smooth_scroll_states.is_empty()).then_some(now + Duration::from_millis(16));
        let touch_scroll_inertia_deadline = (!self.touch_scroll_inertia_states.is_empty())
            .then_some(now + super::TOUCH_SCROLL_INERTIA_FRAME);
        let tooltip_deadline = self.next_tooltip_wakeup_deadline;
        let toast_deadline = self.next_toast_wakeup_deadline;
        let carousel_deadline = self.next_carousel_wakeup_deadline;
        earliest_deadline([
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
        } else if !matches!(event_loop.control_flow(), ControlFlow::Wait) {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    pub(super) fn schedule_next_deadline(
        &self,
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
        } else if !matches!(event_loop.control_flow(), ControlFlow::Wait) {
            event_loop.set_control_flow(ControlFlow::Wait);
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
