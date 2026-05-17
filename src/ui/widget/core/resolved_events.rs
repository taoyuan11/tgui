use super::resolved_freeze::lifecycle_snapshot;
use super::*;

impl<VM> ResolvedElement<VM> {
    pub(super) fn collect_media_event_states(
        &self,
        media: &MediaManager,
        states: &mut Vec<MediaEventState<VM>>,
    ) {
        match &self.kind {
            ResolvedWidgetKind::Container { children, .. } => {
                for child in children {
                    child.collect_media_event_states(media, states);
                }
            }
            #[cfg(feature = "audio")]
            ResolvedWidgetKind::Audio { audio } => {
                if !self.media_events.has_any() {
                    return;
                }
                let snapshot = audio.controller.snapshot();
                if let Some(phase) = media_event_phase(snapshot.loading, snapshot.error.as_deref())
                {
                    states.push(MediaEventState {
                        widget_id: self.id,
                        media_phase: Some(phase),
                        handlers: self.media_events.clone(),
                    });
                }
            }
            ResolvedWidgetKind::Image { image } => {
                if !self.media_events.has_any() {
                    return;
                }
                let source = image.source.resolve();
                let snapshot = media.image_snapshot(&source, None);
                if let Some(phase) = media_event_phase(snapshot.loading, snapshot.error.as_deref())
                {
                    states.push(MediaEventState {
                        widget_id: self.id,
                        media_phase: Some(phase),
                        handlers: self.media_events.clone(),
                    });
                }
            }
            #[cfg(feature = "video")]
            ResolvedWidgetKind::VideoSurface { video, .. } => {
                if !self.media_events.has_any() {
                    return;
                }
                let snapshot = video.controller.surface_snapshot();
                if let Some(phase) = media_event_phase(snapshot.loading, snapshot.error.as_deref())
                {
                    states.push(MediaEventState {
                        widget_id: self.id,
                        media_phase: Some(phase),
                        handlers: self.media_events.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    pub(super) fn collect_lifecycle_event_states(&self, states: &mut Vec<LifecycleEventState<VM>>) {
        if self.lifecycle_events.has_any() || self.requires_runtime_lifecycle() {
            states.push(LifecycleEventState {
                widget_id: self.id,
                snapshot: lifecycle_snapshot(self),
                handlers: self.lifecycle_events.clone(),
            });
        }

        if let ResolvedWidgetKind::Container { children, .. } = &self.kind {
            for child in children {
                child.collect_lifecycle_event_states(states);
            }
        }
    }
}
