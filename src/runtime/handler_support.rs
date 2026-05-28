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

    pub(super) fn next_deadline(&self, now: Instant) -> Option<Instant> {
        let animation_deadline = self.animation_engine.next_frame_deadline(now);
        let controller_deadline = self.animations.next_frame_deadline(now);
        let click_deadline = self.pending_click.as_ref().map(|pending| pending.deadline);
        let gesture_deadline = self
            .active_gesture
            .as_ref()
            .and_then(|gesture| gesture.long_press_deadline);
        let tooltip_release_deadline = self.tooltip_state.long_press_release_deadline;
        let caret_deadline = self.next_caret_blink_deadline(now);
        let key_repeat_deadline = self.next_key_repeat_deadline();
        let smooth_scroll_deadline =
            (!self.smooth_scroll_states.is_empty()).then_some(now + Duration::from_millis(16));
        let tooltip_deadline = self.next_tooltip_wakeup_deadline;
        let toast_deadline = self.next_toast_wakeup_deadline;
        [
            animation_deadline,
            controller_deadline,
            click_deadline,
            gesture_deadline,
            tooltip_release_deadline,
            caret_deadline,
            key_repeat_deadline,
            smooth_scroll_deadline,
            tooltip_deadline,
            toast_deadline,
        ]
        .into_iter()
        .flatten()
        .min()
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
            .with_transparent(!cfg!(all(target_env = "ohos", feature = "ohos")))
            .with_decorations(self.config.decorations)
            .with_title(self.config.title.clone())
            .with_surface_size(self.config.size)
            .with_visible(false);

        #[cfg(target_os = "windows")]
        if self.config.clear_color.a < 255 {
            let platform_attrs =
                winit_win32::WindowAttributesWindows::default().with_no_redirection_bitmap(true);
            attributes = attributes.with_platform_attributes(Box::new(platform_attrs));
        }

        if let Some(min_size) = self.config.min_size {
            attributes = attributes.with_min_surface_size(min_size);
        }

        if let Some(max_size) = self.config.max_size {
            attributes = attributes.with_max_surface_size(max_size);
        }

        if let Some(position) = default_window_position(event_loop, self.config.size) {
            attributes = attributes.with_position(position);
        }

        if let Some(icon_bytes) = self.config.window_icon {
            match image::load_from_memory(icon_bytes) {
                Ok(image) => {
                    let (w, h) = image.dimensions();
                    let rgba = image.into_rgba8().into_raw();

                    match RgbaIcon::new(rgba, w, h) {
                        Ok(ok) => {
                            let icon = Icon::from(ok);
                            attributes = attributes.with_window_icon(Some(icon));
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

        self.theme = resolve_theme(
            &self.active_theme_selection(),
            &self.active_theme_set(),
            resolve_window_theme(
                Some(window.as_ref()),
                #[cfg(all(target_os = "android", feature = "android"))]
                self.android_app.as_ref(),
            ),
        );
        #[cfg(all(target_os = "android", feature = "android"))]
        {
            let theme = self.theme.clone();
            self.sync_system_bar_style(&theme);
        }
        let clear_color =
            if self.window_bindings.clear_color.is_some() || self.config.clear_color_overridden {
                self.config.clear_color
            } else {
                self.theme.colors.background
            };

        let renderer = match Renderer::new(
            window.clone(),
            clear_color,
            self.config.msaa,
            &self.config.fonts,
        ) {
            Ok(renderer) => renderer,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };

        self.window_id = Some(window.id());
        self.renderer = Some(renderer);
        self.last_synced_clear_color = Some(clear_color);
        self.window = Some(window);

        if !self.render_hidden_frame(event_loop) {
            return;
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
            window.set_visible(true);
        }
    }
}
