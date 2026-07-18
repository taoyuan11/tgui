use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn viewport_rect(&self) -> Rect {
        if let Some(window) = self.window.as_ref() {
            let scale_factor = window.scale_factor();
            let size = window.surface_size().to_logical::<f32>(scale_factor);
            let insets = self.config.viewport_insets;
            let width = (size.width - insets.left.get() - insets.right.get()).max(0.0);
            let height = (size.height - insets.top.get() - insets.bottom.get()).max(0.0);
            return Rect::new(insets.left.get(), insets.top.get(), width, height);
        }

        Rect::new(
            0.0,
            0.0,
            self.config.size.width as f32,
            self.config.size.height as f32,
        )
    }

    pub(in crate::runtime) fn invalidate_scene_with_reason(&mut self, reason: &'static str) {
        self.toast_motion_patch_pending = false;
        self.button_hover_patch_pending = None;
        self.button_pressed_patch_pending = None;
        self.row_hover_patch_pending = None;
        if text_profile_enabled() {
            Log::with_tag("tgui-text-prof").debug(format_args!(
                "textarea_invalidate_scene took 0.000ms reason={} had_cache={} focused_widget={:?} focused_input_region={} text_input_epoch={} hover_epoch={} animation_epoch={}",
                reason,
                self.cached_scene.is_some(),
                self.focused_widget_id(),
                self.focused_widget_id()
                    .map(|id| self.text_input_regions.contains_key(&id))
                    .unwrap_or(false),
                self.text_input_epoch,
                self.hover_epoch,
                self.animation_epoch,
            ));
        }
        if let Some(cached) = self.cached_scene.as_mut() {
            cached.layout_valid = false;
            cached.computed_valid = false;
        }
        self.text_input_regions.clear();
    }

    pub(in crate::runtime) fn invalidate_computed_scene(&mut self) {
        self.toast_motion_patch_pending = false;
        self.button_hover_patch_pending = None;
        self.button_pressed_patch_pending = None;
        self.row_hover_patch_pending = None;
        if let Some(cached) = self.cached_scene.as_mut() {
            cached.computed_valid = false;
        }
        self.text_input_regions.clear();
    }

    pub(in crate::runtime) fn invalidate_computed_scene_for_toast_motion(&mut self) {
        let can_patch = self.cached_scene.as_ref().is_some_and(|cached| {
            cached.layout_valid && cached.computed_valid && cached.layout.is_some()
        });
        self.toast_motion_patch_pending = can_patch;
        if let Some(cached) = self.cached_scene.as_mut() {
            cached.computed_valid = false;
        }
        self.text_input_regions.clear();
    }

    pub(in crate::runtime) fn invalidate_text_input_scene(&mut self) {
        self.text_input_epoch = self.text_input_epoch.wrapping_add(1);
    }

    pub(in crate::runtime) fn should_dispatch_widget_event(event: &WindowEvent) -> bool {
        match event {
            WindowEvent::PointerMoved { .. }
            | WindowEvent::PointerEntered { .. }
            | WindowEvent::MouseWheel { .. } => true,
            WindowEvent::PointerButton {
                state: ElementState::Pressed,
                ..
            } => true,
            WindowEvent::KeyboardInput { .. } => true,
            WindowEvent::DragDropped { .. } => true,
            _ => false,
        }
    }

    pub(in crate::runtime) fn render_current_frame(&mut self) -> Result<RenderStatus, TguiError> {
        // Some lifecycle paths can request redraw before a renderer exists.
        // In that case we skip the frame and wait for the next resume/redraw.
        if self.renderer.is_none() {
            return Ok(RenderStatus::SkipFrame);
        }

        let frame_started_at = text_profile_enabled().then_some(Instant::now());
        let sync_started_at = Instant::now();
        self.sync_bindings(Instant::now());
        let sync_duration = sync_started_at.elapsed();
        let media_started_at = Instant::now();
        self.dispatch_media_events();
        let media_duration = media_started_at.elapsed();
        let mut renderer = self
            .renderer
            .take()
            .expect("renderer should exist before drawing");
        self.gpu_scroll_supported = renderer.push_constants_supported();
        let font_manager = self.font_manager.clone();
        let computed_started_at = Instant::now();
        let status = {
            let computed = self.computed_scene_mut();
            let computed_duration = computed_started_at.elapsed();
            let render_started_at = Instant::now();
            let status = renderer.render(
                &mut computed.scene,
                &font_manager,
                &computed.scroll_regions,
                &computed.transform_records,
            );
            let render_duration = render_started_at.elapsed();
            if let Some(frame_started_at) = frame_started_at {
                let status_name = match &status {
                    Ok(RenderStatus::Rendered) => "Rendered",
                    Ok(RenderStatus::SkipFrame) => "SkipFrame",
                    Ok(RenderStatus::ReconfigureSurface) => "ReconfigureSurface",
                    Err(_) => "Error",
                };
                log_text_profile(
                    "textarea_render",
                    frame_started_at.elapsed(),
                    format!(
                        "sync_ms={:.3} media_ms={:.3} computed_scene_ms={:.3} render_ms={:.3} status={}",
                        sync_duration.as_secs_f64() * 1000.0,
                        media_duration.as_secs_f64() * 1000.0,
                        computed_duration.as_secs_f64() * 1000.0,
                        render_duration.as_secs_f64() * 1000.0,
                        status_name,
                    ),
                );
            }
            status
        };
        self.renderer = Some(renderer);
        self.sync_accessibility_tree();
        if !self.first_frame_logged {
            if matches!(&status, Ok(RenderStatus::Rendered)) {
                self.first_frame_logged = true;
                log_startup_phase(
                    "first_frame",
                    self.startup_started_at.elapsed(),
                    format_args!("window_key={}", self.window_key),
                );
            }
        }
        status
    }

    pub(in crate::runtime) fn render_hidden_frame(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
    ) -> bool {
        let status = match self.render_current_frame() {
            Ok(status) => status,
            Err(error) => {
                self.fail(event_loop, error);
                return false;
            }
        };

        if matches!(status, RenderStatus::ReconfigureSurface) {
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.reconfigure();
            }

            if let Err(error) = self.render_current_frame() {
                self.fail(event_loop, error);
                return false;
            }
        }

        true
    }

    pub(in crate::runtime) fn resume_existing_window(&mut self, event_loop: &dyn ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };

        // The system appearance may have changed while the event loop was suspended and no
        // `ThemeChanged` event could be delivered. Resume is the one event-driven re-query.
        let window_theme = resolve_window_theme(Some(window.as_ref()));
        self.initialize_platform_window_binding_state(window_theme);
        self.sync_theme_binding();
        self.invalidate_scene_with_reason("resume_existing_window");
        self.refresh_frame_clock_from_monitor(Instant::now(), true);
        let clear_color =
            if self.window_bindings.clear_color.is_some() || self.config.clear_color_overridden {
                self.config.clear_color
            } else {
                self.theme.colors.background
            };

        match Renderer::new(window.clone(), clear_color, self.config.msaa) {
            Ok(renderer) => {
                self.gpu_scroll_supported = renderer.push_constants_supported();
                self.renderer = Some(renderer);
                self.last_synced_clear_color = Some(clear_color);
                self.initialize_accessibility_adapter();
                #[cfg(feature = "video")]
                self.notify_video_surface_restored();
            }
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        }

        if !self.render_hidden_frame(event_loop) {
            return;
        }

        #[cfg(feature = "video")]
        self.notify_video_app_foreground();

        window.request_redraw();
        window.set_visible(true);
    }

    pub(in crate::runtime) fn suspend(&mut self) {
        #[cfg(feature = "video")]
        {
            self.notify_video_app_background();
            self.notify_video_surface_lost();
        }

        self.renderer = None;
        self.gpu_scroll_supported = false;
        self.cached_scene = None;
        self.media_event_states.clear();
        self.lifecycle_event_states.clear();
    }
}
