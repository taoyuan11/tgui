use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn uses_system_theme(&self) -> bool {
        self.theme_store.mode() == ThemeMode::System
    }

    pub(in crate::runtime) fn apply_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.animation_epoch = self.animation_epoch.wrapping_add(1);
        self.layout_animation_epoch = self.layout_animation_epoch.wrapping_add(1);
        self.invalidate_scene_with_reason("apply_theme");
    }

    pub(in crate::runtime) fn apply_window_theme(&mut self, window_theme: Option<WindowTheme>) {
        self.sync_platform_window_identity();
        let system_theme = window_theme
            .or_else(|| resolve_window_theme(self.window.as_deref()))
            .or(self.theme_store.system_theme());
        let changed = self.theme_store.set_system_theme(system_theme);
        if changed && self.uses_system_theme() {
            self.apply_theme(self.theme_store.current().as_ref().clone());
        }
    }

    pub(in crate::runtime) fn initialize_platform_window_binding_state(
        &mut self,
        window_theme: Option<WindowTheme>,
    ) {
        self.sync_platform_window_identity();
        if self.theme_store.set_system_theme(window_theme) {
            self.binding_sync.theme_initialized = false;
        }
    }

    fn sync_platform_window_identity(&mut self) -> bool {
        let identity = self
            .window
            .as_ref()
            .map(|window| Arc::as_ptr(window) as *const () as usize);
        if self.binding_sync.window_identity == identity {
            return false;
        }

        self.binding_sync.window_identity = identity;
        // A platform window owns its title. Replacing it must replay even an unchanged binding.
        self.binding_sync.title_token = None;
        self.binding_sync.title = None;
        true
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

    pub(in crate::runtime) fn sync_theme_binding(&mut self) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let window_changed = self.sync_platform_window_identity();
        let mut inputs_changed = !self.binding_sync.theme_initialized;
        let mut resolved_system_theme = None;

        if window_changed {
            resolved_system_theme = resolve_window_theme(self.window.as_deref());
            let system_theme = resolved_system_theme.or(self.theme_store.system_theme());
            inputs_changed |= self.theme_store.set_system_theme(system_theme);
        }

        let revision = self.invalidation.revision();
        let mode_identity = self
            .window_bindings
            .theme_mode
            .as_ref()
            .map(Signal::sync_identity);
        let theme_set_identity = self
            .window_bindings
            .theme_set
            .as_ref()
            .map(Signal::sync_identity);
        let theme_bindings_dirty = !self.binding_sync.theme_initialized
            || self.binding_sync.theme_checked_revision != Some(revision)
            || self.binding_sync.theme_mode_token.map(|token| token.0) != mode_identity
            || self.binding_sync.theme_set_token.map(|token| token.0) != theme_set_identity;
        if theme_bindings_dirty {
            let mode_token = self
                .window_bindings
                .theme_mode
                .as_ref()
                .map(Signal::sync_token);
            if !self.binding_sync.theme_initialized
                || self.binding_sync.theme_mode_token != mode_token
            {
                let mode = self
                    .window_bindings
                    .theme_mode
                    .as_ref()
                    .map(Signal::get)
                    .unwrap_or_else(|| match &self.config.theme {
                        ThemeSelection::System => ThemeMode::System,
                        ThemeSelection::Mode(mode) => *mode,
                    });
                self.binding_sync.theme_mode_token = mode_token;
                inputs_changed |= self.theme_store.set_mode(mode);
            }

            let theme_set_token = self
                .window_bindings
                .theme_set
                .as_ref()
                .map(Signal::sync_token);
            if !self.binding_sync.theme_initialized
                || self.binding_sync.theme_set_token != theme_set_token
            {
                let theme_set = self
                    .window_bindings
                    .theme_set
                    .as_ref()
                    .map(Signal::get)
                    .unwrap_or_else(|| self.config.theme_set.clone());
                self.binding_sync.theme_set_token = theme_set_token;
                if self.theme_store.theme_set() != &theme_set {
                    inputs_changed |= self.theme_store.set_theme_set(theme_set);
                }
            }
            self.binding_sync.theme_checked_revision = Some(revision);
        }

        self.binding_sync.theme_initialized = true;
        let changed = if inputs_changed {
            #[cfg(test)]
            {
                self.binding_sync.theme_resolves += 1;
            }
            let resolved_theme = self.theme_store.current().as_ref().clone();
            if self.theme != resolved_theme {
                self.apply_theme(resolved_theme);
                true
            } else {
                false
            }
        } else {
            false
        };

        if let Some(started_at) = started_at {
            log_text_profile(
                "textarea_theme_sync",
                started_at.elapsed(),
                format!(
                    "mode={:?} resolved_system_theme={:?} applied_system_theme={:?} inputs_changed={} changed={}",
                    self.theme_store.mode(),
                    resolved_system_theme,
                    self.theme_store.system_theme(),
                    inputs_changed,
                    changed,
                ),
            );
        }
        changed
    }

    pub(in crate::runtime) fn refresh_platform_theme(&mut self) -> bool {
        self.sync_theme_binding()
    }

    pub(in crate::runtime) fn sync_bindings(&mut self, now: Instant) -> bool {
        let started_at = text_profile_enabled().then_some(Instant::now());
        let theme_changed = self.sync_theme_binding();
        let revision = self.invalidation.revision();
        let title_identity = self
            .window_bindings
            .title
            .as_ref()
            .map(Signal::sync_identity);
        let reduced_motion_identity = self
            .window_bindings
            .reduced_motion
            .as_ref()
            .map(Signal::sync_identity);
        let clear_color_identity = self
            .window_bindings
            .clear_color
            .as_ref()
            .map(Signal::sync_identity);
        let window_properties_dirty = self.binding_sync.window_properties_checked_revision
            != Some(revision)
            || self.binding_sync.title_token.map(|token| token.0) != title_identity
            || self.binding_sync.reduced_motion_token.map(|token| token.0)
                != reduced_motion_identity
            || self.binding_sync.clear_color_token.map(|token| token.0) != clear_color_identity;

        let mut reduced_motion_changed = false;
        if window_properties_dirty {
            let reduced_motion_token = self
                .window_bindings
                .reduced_motion
                .as_ref()
                .map(Signal::sync_token);
            if self.binding_sync.reduced_motion_token != reduced_motion_token {
                self.binding_sync.reduced_motion_token = reduced_motion_token;
                let next = self.active_reduced_motion();
                reduced_motion_changed = self.reduced_motion != next;
                self.reduced_motion = next;
            }

            if let (Some(window), Some(signal)) =
                (self.window.as_ref(), self.window_bindings.title.as_ref())
            {
                let token = signal.sync_token();
                if self.binding_sync.title_token != Some(token) {
                    let title = signal.get();
                    self.binding_sync.title_token = Some(token);
                    if self.binding_sync.title.as_ref() != Some(&title) {
                        window.set_title(&title);
                        self.binding_sync.title = Some(title);
                    }
                }
            } else if let Some(signal) = self.window_bindings.title.as_ref() {
                // Remember the source while suspended; attaching/replacing a window clears this
                // token so the latest value is replayed exactly once.
                self.binding_sync.title_token = Some(signal.sync_token());
            } else {
                self.binding_sync.title_token = None;
                self.binding_sync.title = None;
            }

            if let Some(signal) = self.window_bindings.clear_color.as_ref() {
                let token = signal.sync_token();
                if self.binding_sync.clear_color_token != Some(token) {
                    self.binding_sync.clear_color_token = Some(token);
                    self.binding_sync.clear_color_target = Some(signal.get());
                }
            } else {
                self.binding_sync.clear_color_token = None;
                self.binding_sync.clear_color_target = None;
                self.binding_sync.clear_color_animation_initialized = false;
            }
            self.binding_sync.window_properties_checked_revision = Some(revision);
        }

        let mut clear_color_changed = false;
        let next_clear_color = if self.renderer.is_none() {
            None
        } else if let Some(signal) = self.window_bindings.clear_color.as_ref() {
            if self.binding_sync.clear_color_target.is_none() {
                self.binding_sync.clear_color_target = Some(signal.get());
            }
            let target = self
                .binding_sync
                .clear_color_target
                .unwrap_or(self.config.clear_color);
            let key = AnimationKey::Window(WindowProperty::ClearColor);
            if !self.binding_sync.clear_color_animation_initialized
                || !self.animation_engine.color_settled_at(key, target)
            {
                self.binding_sync.clear_color_animation_initialized = true;
                Some(
                    self.animation_engine
                        .resolve_color(key, target, signal.transition(), now),
                )
            } else {
                None
            }
        } else if !self.config.clear_color_overridden {
            self.binding_sync.clear_color_token = None;
            self.binding_sync.clear_color_target = None;
            self.binding_sync.clear_color_animation_initialized = false;
            let target = self.theme.colors.background;
            if self.last_synced_clear_color != Some(target)
                || !self.window_color_settled_at(WindowProperty::ThemeBackground, target)
            {
                Some(self.theme_background_color(now))
            } else {
                None
            }
        } else {
            self.binding_sync.clear_color_token = None;
            self.binding_sync.clear_color_target = None;
            self.binding_sync.clear_color_animation_initialized = false;
            None
        };
        if let (Some(renderer), Some(next_clear_color)) = (self.renderer.as_mut(), next_clear_color)
        {
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
        with_runtime_redraw_stack(|| self.request_redraw_if_dirty_inner(now));
    }

    fn request_redraw_if_dirty_inner(&mut self, now: Instant) {
        let revision = self.invalidation.revision();
        let caret_blink_changed = self.caret_blink_needs_redraw(now);
        if revision != self.last_invalidation_revision {
            // A model/media dependency changed in addition to any scheduled Toast tick. Do not
            // reuse prepared card trees across that change; the normal retained/full path will
            // rebuild them with the new signal/theme/media state.
            self.toast_motion_patch_pending = false;
            let started_at = text_profile_enabled().then_some(Instant::now());
            let previous_revision = self.last_invalidation_revision;
            let media_only = self
                .invalidation
                .media_only_since(previous_revision, revision);
            let media_completions = self.media_manager.drain_completions();
            if media_only
                && !media_completions.is_empty()
                && self.try_patch_media_completions(&media_completions)
            {
                self.rebuild_reactive_slot_bindings(now);
                self.rebuild_strict_capability_report();
                self.last_invalidation_revision = revision;
                super::action_stats::record("media_texture_slot_write");
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
                if let Some(started_at) = started_at {
                    log_text_profile(
                        "textarea_redraw",
                        started_at.elapsed(),
                        format!(
                            "revision {} -> {} media_completions={} invalidation_action=media_texture_slot_write requested_redraw=true",
                            previous_revision,
                            revision,
                            media_completions.len(),
                        ),
                    );
                }
                return;
            }
            if media_only && !media_completions.is_empty() && self.strict_reactive_tree() {
                self.last_invalidation_revision = revision;
                super::action_stats::record("strict_reactive_media_rejected");
                if let Some(started_at) = started_at {
                    log_text_profile(
                        "textarea_redraw",
                        started_at.elapsed(),
                        format!(
                            "revision {} -> {} media_completions={} invalidation_action=strict_reactive_media_rejected requested_redraw=false",
                            previous_revision,
                            revision,
                            media_completions.len(),
                        ),
                    );
                }
                return;
            }
            let (dirty_kind, dirty_dependencies) = self
                .invalidation
                .dirty_dependencies_since(previous_revision);
            self.last_invalidation_revision = revision;
            let reactive_updates = self.invalidation.drain_reactive_updates();
            let reactive_redraw = !reactive_updates.targets.is_empty();
            if reactive_redraw {
                super::action_stats::record("reactive_slot_update");
            }
            let bindings_redraw = self.sync_bindings(now);
            let invalidation_action = self.invalidate_cached_scene_for_dependencies(
                dirty_kind,
                &dirty_dependencies,
                &reactive_updates.targets,
                reactive_updates.processed_signals,
                now,
            );
            super::action_stats::record(invalidation_action);
            let media_action = if media_completions.is_empty() {
                "none"
            } else if self.try_patch_media_completions(&media_completions) {
                self.rebuild_reactive_slot_bindings(now);
                self.rebuild_strict_capability_report();
                super::action_stats::record("media_texture_slot_write");
                "media_texture_slot_write"
            } else if self.strict_reactive_tree() {
                super::action_stats::record("strict_reactive_media_rejected");
                "strict_reactive_media_rejected"
            } else {
                self.invalidate_scene_with_reason("media_completion_patch_failed");
                super::action_stats::record("media_texture_full_rebuild");
                "media_texture_full_rebuild"
            };
            let requested_redraw = reactive_redraw
                || bindings_redraw
                || invalidation_action != "unrelated"
                || media_action != "none";

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
                        "revision {} -> {} dirty_kind={} dirty_dependencies={} reactive_targets={} invalidation_action={} media_action={} bindings_redraw={} requested_redraw={}",
                        previous_revision,
                        revision,
                        dirty_dependency_set_label(dirty_kind),
                        dirty_dependencies.len(),
                        reactive_updates.targets.len(),
                        invalidation_action,
                        media_action,
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

fn with_runtime_redraw_stack<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        const RUNTIME_REDRAW_STACK_SIZE: usize = 16 * 1024 * 1024;
        return stacker::grow(RUNTIME_REDRAW_STACK_SIZE, f);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        f()
    }
}
