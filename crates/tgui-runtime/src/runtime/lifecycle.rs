use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn dispatch_media_events(&mut self) {
        let Some(tree) = self.widget_tree.as_ref() else {
            self.media_event_states.clear();
            return;
        };

        let states = tree.media_event_states(&self.media_manager, &self.theme);
        let current_ids: HashSet<_> = states.iter().map(|state| state.widget_id).collect();
        self.media_event_states
            .retain(|widget_id, _| current_ids.contains(widget_id));

        let mut pending = Vec::new();
        for state in states {
            let previous = self.media_event_states.get(&state.widget_id);
            collect_pending_media_event(&state, previous, &mut pending);
            self.media_event_states.insert(
                state.widget_id,
                DispatchedMediaState {
                    phase: state.media_phase.clone(),
                },
            );
        }

        if pending.is_empty() {
            return;
        }

        for event in pending {
            match event {
                PendingMediaEvent::Command(command) => self.execute_command(&command),
                PendingMediaEvent::Error(command, error) => {
                    self.execute_value_command(&command, error);
                }
            }
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    #[cfg(feature = "audio")]
    fn sync_audio_widget_on_mount(&self, current: &AudioLifecycleState) {
        current.controller.set_looping(current.looping);
        if current.autoplay {
            current.controller.play();
        }
    }

    #[cfg(feature = "audio")]
    fn sync_audio_widget_on_update(
        &self,
        current: &AudioLifecycleState,
        previous: &AudioLifecycleState,
    ) {
        if current.controller != previous.controller {
            previous.controller.stop();
            self.sync_audio_widget_on_mount(current);
            return;
        }

        if current.looping != previous.looping {
            current.controller.set_looping(current.looping);
        }
    }

    #[cfg(feature = "audio")]
    fn sync_audio_widget_lifecycle(
        &self,
        state: &LifecycleEventState<VM>,
        previous: Option<&DispatchedLifecycleState<VM>>,
    ) {
        let Some(current) = audio_lifecycle_state(&state.snapshot) else {
            return;
        };
        let previous_audio =
            previous.and_then(|previous| audio_lifecycle_state(&previous.snapshot));
        match previous_audio.as_ref() {
            None => self.sync_audio_widget_on_mount(&current),
            Some(previous_audio) if current != *previous_audio => {
                self.sync_audio_widget_on_update(&current, previous_audio);
            }
            Some(_) => {}
        }
    }

    #[cfg(feature = "audio")]
    fn teardown_audio_widget(&self, previous: &DispatchedLifecycleState<VM>) {
        if let Some(audio) = audio_lifecycle_state(&previous.snapshot) {
            audio.controller.stop();
        }
    }

    fn dispatch_lifecycle_events(&mut self) {
        if self.widget_tree.is_none() {
            if self.lifecycle_event_states.is_empty() {
                return;
            }

            let mut pending = Vec::new();
            for previous in self.lifecycle_event_states.values() {
                #[cfg(feature = "audio")]
                self.teardown_audio_widget(previous);
                if let Some(command) = previous.handlers.on_unmount.clone() {
                    pending.push(PendingLifecycleEvent::Command(command));
                }
            }
            self.lifecycle_event_states.clear();
            if pending.is_empty() {
                return;
            }
            for event in pending {
                match event {
                    PendingLifecycleEvent::Command(command) => {
                        self.execute_command_without_invalidation(&command)
                    }
                }
            }
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
            return;
        }

        if self
            .cached_scene
            .as_ref()
            .map(|cached| !cached.computed_valid)
            .unwrap_or(true)
        {
            let _ = self.computed_scene();
        }

        if self
            .cached_scene
            .as_ref()
            .map(|cached| cached.lifecycle_states.is_empty())
            .unwrap_or(true)
            && self.lifecycle_event_states.is_empty()
        {
            return;
        }

        let fallback_states = self.cached_scene.is_none().then(|| {
            self.widget_tree
                .as_ref()
                .expect("widget tree should exist")
                .lifecycle_event_states(&self.theme)
                .into_iter()
                .map(|state| (state.widget_id, state))
                .collect::<HashMap<_, _>>()
        });
        let states = fallback_states
            .as_ref()
            .or_else(|| {
                self.cached_scene
                    .as_ref()
                    .map(|cached| &cached.lifecycle_states)
            })
            .expect("lifecycle states should be available");
        let current_ids: HashSet<_> = states.keys().copied().collect();

        let removed_ids: Vec<_> = self
            .lifecycle_event_states
            .keys()
            .copied()
            .filter(|widget_id| !current_ids.contains(widget_id))
            .collect();

        let mut pending = Vec::new();
        for state in states.values() {
            let previous = self.lifecycle_event_states.get(&state.widget_id);
            #[cfg(feature = "audio")]
            self.sync_audio_widget_lifecycle(state, previous);
            collect_pending_lifecycle_events(state, previous, &mut pending);
        }

        for removed_id in removed_ids {
            if let Some(previous) = self.lifecycle_event_states.remove(&removed_id) {
                #[cfg(feature = "audio")]
                self.teardown_audio_widget(&previous);
                if let Some(command) = previous.handlers.on_unmount {
                    pending.push(PendingLifecycleEvent::Command(command));
                }
            }
        }

        for state in states.values().cloned() {
            self.lifecycle_event_states.insert(
                state.widget_id,
                DispatchedLifecycleState {
                    snapshot: state.snapshot,
                    handlers: state.handlers,
                },
            );
        }

        if pending.is_empty() {
            return;
        }

        for event in pending {
            match event {
                PendingLifecycleEvent::Command(command) => {
                    self.execute_command_without_invalidation(&command)
                }
            }
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    pub(in crate::runtime) fn dispatch_lifecycle_events_if_needed(&mut self) {
        let revision = self.invalidation.revision();
        if revision != self.last_invalidation_revision {
            self.request_redraw_if_dirty(Instant::now());
        }
        if revision == self.last_lifecycle_dispatch_revision {
            return;
        }

        let cached_has_lifecycle_handlers = self
            .cached_scene
            .as_ref()
            .map(|cached| !cached.lifecycle_states.is_empty());

        if cached_has_lifecycle_handlers == Some(false) && self.lifecycle_event_states.is_empty() {
            self.last_lifecycle_dispatch_revision = revision;
            return;
        }

        self.last_lifecycle_dispatch_revision = revision;
        self.dispatch_lifecycle_events();
    }
}
