use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn viewport_rect(&self) -> Rect {
        let size = self
            .window
            .as_ref()
            .map(|window| {
                window
                    .surface_size()
                    .to_logical::<f32>(window.scale_factor())
            })
            .unwrap_or(crate::platform::dpi::LogicalSize::new(
                self.config.size.width as f32,
                self.config.size.height as f32,
            ));
        Rect::new(0.0, 0.0, size.width, size.height)
    }

    pub(in crate::runtime) fn invalidate_scene_with_reason(&mut self, reason: &'static str) {
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
        self.cached_scene = None;
        self.text_input_regions.clear();
    }

    pub(in crate::runtime) fn invalidate_computed_scene(&mut self) {
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
                button,
                ..
            } => button.clone().mouse_button() == Some(MouseButton::Left),
            WindowEvent::KeyboardInput { .. } => true,
            _ => false,
        }
    }

    pub(in crate::runtime) fn render_current_frame(&mut self) -> Result<RenderStatus, TguiError> {
        // Android can deliver a redraw before a replacement surface is ready.
        // In that case we simply skip the frame and wait for the next resume/redraw.
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
        let caret_rect = self.ime_cursor_area();
        if let (Some(window), Some(caret_rect)) = (self.window.as_ref(), caret_rect) {
            let _ = window.request_ime_update(ImeRequest::Update(Self::ime_cursor_request_data(
                caret_rect,
                self.unit_context(),
            )));
        }
        let mut renderer = self
            .renderer
            .take()
            .expect("renderer should exist before drawing");
        let computed_started_at = Instant::now();
        let status = {
            let computed = self.computed_scene();
            let computed_duration = computed_started_at.elapsed();
            let render_started_at = Instant::now();
            let status = renderer.render(&computed.scene);
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

    #[cfg(all(target_os = "android", feature = "android"))]
    pub(in crate::runtime) fn render_immediately(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.window.is_none() || self.renderer.is_none() {
            return;
        }

        match self.render_current_frame() {
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
        }
    }

    pub(in crate::runtime) fn render_hidden_frame(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
    ) -> bool {
        #[cfg(all(target_env = "ohos", feature = "ohos"))]
        {
            let _ = event_loop;
            return true;
        }

        #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
        let status = match self.render_current_frame() {
            Ok(status) => status,
            Err(error) => {
                self.fail(event_loop, error);
                return false;
            }
        };

        #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
        if matches!(status, RenderStatus::ReconfigureSurface) {
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.reconfigure();
            }

            if let Err(error) = self.render_current_frame() {
                self.fail(event_loop, error);
                return false;
            }
        }

        #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
        true
    }

    pub(in crate::runtime) fn resume_existing_window(&mut self, event_loop: &dyn ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };

        self.sync_theme_binding();
        self.invalidate_scene_with_reason("resume_existing_window");
        let clear_color =
            if self.window_bindings.clear_color.is_some() || self.config.clear_color_overridden {
                self.config.clear_color
            } else {
                self.theme.colors.background
            };

        match Renderer::new(
            window.clone(),
            clear_color,
            self.config.msaa,
            &self.config.fonts,
        ) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
                self.last_synced_clear_color = Some(clear_color);
            }
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        }

        #[cfg(all(target_os = "android", feature = "android"))]
        {
            let theme = self.theme.clone();
            self.sync_system_bar_style(&theme);
        }

        if !self.render_hidden_frame(event_loop) {
            return;
        }

        window.request_redraw();
        window.set_visible(true);
    }

    pub(in crate::runtime) fn suspend(&mut self) {
        self.renderer = None;
        self.cached_scene = None;
        self.media_event_states.clear();
        self.lifecycle_event_states.clear();
        #[cfg(all(target_os = "android", feature = "android"))]
        {
            self.system_bar_style = None;
        }
    }
}
