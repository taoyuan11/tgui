use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn uses_system_theme(&self) -> bool {
        matches!(self.active_theme_selection(), ThemeSelection::System)
    }

    pub(in crate::runtime) fn apply_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.animation_epoch = self.animation_epoch.wrapping_add(1);
        self.layout_animation_epoch = self.layout_animation_epoch.wrapping_add(1);
        self.invalidate_scene_with_reason("apply_theme");
    }

    pub(in crate::runtime) fn apply_window_theme(&mut self, window_theme: Option<WindowTheme>) {
        if self.uses_system_theme() {
            self.apply_theme(resolve_theme(
                &self.active_theme_selection(),
                &self.active_theme_set(),
                resolve_window_theme(self.window.as_deref()).or(window_theme),
            ));
        }
    }

    pub(in crate::runtime) fn active_theme_selection(&self) -> ThemeSelection {
        self.window_bindings
            .theme_mode
            .as_ref()
            .map(|signal| ThemeSelection::from_mode(signal.get()))
            .unwrap_or_else(|| self.config.theme.clone())
    }

    pub(in crate::runtime) fn active_theme_set(&self) -> ThemeSet {
        self.window_bindings
            .theme_set
            .as_ref()
            .map(Signal::get)
            .unwrap_or_else(|| self.config.theme_set.clone())
    }

    pub(in crate::runtime) fn active_reduced_motion(&self) -> bool {
        self.window_bindings
            .reduced_motion
            .as_ref()
            .map(Signal::get)
            .unwrap_or(self.config.reduced_motion)
    }

    pub(in crate::runtime) fn sync_theme_binding(&mut self) {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let selection = self.active_theme_selection();
        let theme_set = self.active_theme_set();
        let resolved_system_theme = resolve_window_theme(self.window.as_deref());
        let previous_store_theme = self.theme_store.system_theme();
        let system_theme = resolved_system_theme.or(previous_store_theme);
        self.theme_store.set_theme_set(theme_set.clone());
        self.theme_store.set_system_theme(system_theme);
        let resolved_theme = match selection {
            ThemeSelection::System => {
                self.theme_store.set_mode(ThemeMode::System);
                self.theme_store.current().as_ref().clone()
            }
            ThemeSelection::Mode(mode) => {
                self.theme_store.set_mode(mode);
                self.theme_store.current().as_ref().clone()
            }
        };
        let changed = self.theme != resolved_theme;
        if self.theme != resolved_theme {
            self.apply_theme(resolved_theme);
        }
        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_theme_sync",
                started_at.elapsed(),
                format!(
                    "selection={:?} resolved_system_theme={:?} previous_store_theme={:?} applied_system_theme={:?} changed={}",
                    selection,
                    resolved_system_theme,
                    previous_store_theme,
                    system_theme,
                    changed,
                ),
            );
        }
    }

    pub(in crate::runtime) fn refresh_platform_theme(&mut self) -> bool {
        let previous_theme = self.theme.clone();
        self.sync_theme_binding();
        self.theme != previous_theme
    }

    pub(in crate::runtime) fn sync_bindings(&mut self, now: Instant) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let previous_theme = self.theme.clone();
        self.sync_theme_binding();
        let theme_changed = self.theme != previous_theme;
        let previous_reduced_motion = self.reduced_motion;
        self.reduced_motion = self.active_reduced_motion();
        let reduced_motion_changed = previous_reduced_motion != self.reduced_motion;

        if let Some(window) = self.window.as_ref() {
            if let Some(signal) = self.window_bindings.title.as_ref() {
                window.set_title(&signal.get());
            }
        }

        let theme = self.animated_theme(now);
        let mut clear_color_changed = false;
        if let Some(renderer) = self.renderer.as_mut() {
            let next_clear_color = if let Some(signal) = self.window_bindings.clear_color.as_ref() {
                self.animation_engine.resolve_color(
                    AnimationKey::Window(WindowProperty::ClearColor),
                    signal.get(),
                    signal.transition(),
                    now,
                )
            } else if !self.config.clear_color_overridden {
                theme.colors.background
            } else {
                self.last_synced_clear_color
                    .unwrap_or(self.config.clear_color)
            };
            clear_color_changed = self.last_synced_clear_color != Some(next_clear_color);
            if clear_color_changed {
                renderer.set_clear_color(next_clear_color);
                self.last_synced_clear_color = Some(next_clear_color);
            }
        }

        let _ = started_at;
        theme_changed || clear_color_changed || reduced_motion_changed
    }

    pub(in crate::runtime) fn request_redraw_if_dirty(&mut self, now: Instant) {
        let revision = self.invalidation.revision();
        let caret_blink_changed = self.caret_blink_needs_redraw(now);
        if revision != self.last_invalidation_revision {
            let started_at = text_profile_enabled().then_some(Instant::now());
            let previous_revision = self.last_invalidation_revision;
            let (dirty_kind, dirty_dependencies) = self
                .invalidation
                .dirty_dependencies_since(previous_revision);
            self.last_invalidation_revision = revision;
            let bindings_redraw = self.sync_bindings(now);
            let invalidation_action =
                self.invalidate_cached_scene_for_dependencies(dirty_kind, &dirty_dependencies, now);
            let requested_redraw = bindings_redraw || invalidation_action != "unrelated";

            if requested_redraw {
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }

            if let Some(started_at) = started_at {
                log_text_profile(
                    "textarea_redraw",
                    started_at.elapsed(),
                    format!(
                        "revision {} -> {} dirty_kind={} dirty_dependencies={} invalidation_action={} bindings_redraw={} requested_redraw={}",
                        previous_revision,
                        revision,
                        dirty_dependency_set_label(dirty_kind),
                        dirty_dependencies.len(),
                        invalidation_action,
                        bindings_redraw,
                        requested_redraw
                    ),
                );
            }
        }

        if caret_blink_changed {
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
    }
}
