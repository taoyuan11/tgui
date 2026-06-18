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

        let mut clear_color_changed = false;
        let next_clear_color = if self.renderer.is_some() {
            if let Some(signal) = self.window_bindings.clear_color.as_ref() {
                Some(self.animation_engine.resolve_color(
                    AnimationKey::Window(WindowProperty::ClearColor),
                    signal.get(),
                    signal.transition(),
                    now,
                ))
            } else if !self.config.clear_color_overridden {
                Some(self.theme_background_color(now))
            } else {
                Some(
                    self.last_synced_clear_color
                        .unwrap_or(self.config.clear_color),
                )
            }
        } else {
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
