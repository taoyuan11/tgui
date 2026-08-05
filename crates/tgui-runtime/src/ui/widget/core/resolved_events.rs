use super::resolved_freeze::lifecycle_snapshot;
use super::*;

impl<VM: 'static> ResolvedElement<VM> {
    fn inactive_scope_suppresses_runtime_events(&self) -> bool {
        self.focus
            .scope
            .as_ref()
            .is_some_and(|scope| scope.suppresses_interactions(scope.is_active_untracked()))
    }

    pub(super) fn collect_media_event_states(
        &self,
        media: &MediaManager,
        theme: &Theme,
        active_tooltip: Option<WidgetId>,
        states: &mut Vec<MediaEventState<VM>>,
    ) {
        if self.inactive_scope_suppresses_runtime_events() {
            return;
        }
        #[cfg(test)]
        media_event_walk_probe::record_visit();
        match &self.kind {
            ResolvedWidgetKind::Container { children, .. } => {
                for child in children {
                    child.collect_media_event_states(media, theme, active_tooltip, states);
                }
            }
            ResolvedWidgetKind::Virtual { children, .. } => {
                for child in children {
                    child.collect_media_event_states(media, theme, active_tooltip, states);
                }
            }
            ResolvedWidgetKind::Portal { content, open, .. } if open.resolve() => content
                .resolve(theme)
                .collect_media_event_states(media, theme, active_tooltip, states),
            #[cfg(feature = "audio")]
            ResolvedWidgetKind::Audio { audio } => {
                if self.media_events.has_any() {
                    let snapshot = audio.controller.snapshot();
                    if let Some(phase) =
                        media_event_phase(snapshot.loading, snapshot.error.as_deref())
                    {
                        states.push(MediaEventState {
                            widget_id: self.id,
                            media_phase: Some(phase),
                            handlers: self.media_events.clone(),
                        });
                    }
                }
            }
            ResolvedWidgetKind::Image { image, .. } => {
                if self.media_events.has_any() {
                    let source = image.source.resolve();
                    let snapshot = media.image_snapshot(&source, None);
                    if let Some(phase) =
                        media_event_phase(snapshot.loading, snapshot.error.as_deref())
                    {
                        states.push(MediaEventState {
                            widget_id: self.id,
                            media_phase: Some(phase),
                            handlers: self.media_events.clone(),
                        });
                    }
                }
            }
            #[cfg(feature = "video")]
            ResolvedWidgetKind::VideoSurface { video, .. } => {
                if self.media_events.has_any() {
                    let snapshot = video.controller.surface_metadata();
                    if let Some(phase) =
                        media_event_phase(snapshot.loading, snapshot.error.as_deref())
                    {
                        states.push(MediaEventState {
                            widget_id: self.id,
                            media_phase: Some(phase),
                            handlers: self.media_events.clone(),
                        });
                    }
                }
            }
            _ => {}
        }

        if active_tooltip == Some(self.id) {
            if let Some(tooltip) = self.tooltip.as_ref() {
                if let crate::ui::widget::tooltip::TooltipContent::Element(content) =
                    &tooltip.content
                {
                    content.resolve(theme).collect_media_event_states(
                        media,
                        theme,
                        active_tooltip,
                        states,
                    );
                }
            }
        }
    }

    pub(super) fn collect_lifecycle_event_states(&self, states: &mut Vec<LifecycleEventState<VM>>) {
        if self.inactive_scope_suppresses_runtime_events() {
            return;
        }
        if self.lifecycle_events.has_any() || self.requires_runtime_lifecycle() {
            states.push(LifecycleEventState {
                widget_id: self.id,
                snapshot: lifecycle_snapshot(self),
                handlers: self.lifecycle_events.clone(),
            });
        }

        if let ResolvedWidgetKind::Container { children, .. }
        | ResolvedWidgetKind::Virtual { children, .. } = &self.kind
        {
            for child in children {
                child.collect_lifecycle_event_states(states);
            }
        }
    }

    #[cfg(feature = "video")]
    pub(super) fn collect_video_controllers(
        &self,
        controllers: &mut Vec<crate::video::VideoController>,
    ) {
        if self.inactive_scope_suppresses_runtime_events() {
            return;
        }
        match &self.kind {
            ResolvedWidgetKind::Container { children, .. }
            | ResolvedWidgetKind::Virtual { children, .. } => {
                for child in children {
                    child.collect_video_controllers(controllers);
                }
            }
            ResolvedWidgetKind::VideoSurface { video, .. } => {
                if !controllers
                    .iter()
                    .any(|controller| controller == &video.controller)
                {
                    controllers.push(video.controller.clone());
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
pub(crate) mod media_event_walk_probe {
    use std::cell::Cell;

    thread_local! {
        static VISITS: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn record_visit() {
        VISITS.set(VISITS.get() + 1);
    }

    pub(crate) fn reset() {
        VISITS.set(0);
    }

    pub(crate) fn visits() -> usize {
        VISITS.get()
    }
}
