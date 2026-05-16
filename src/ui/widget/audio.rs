use crate::audio::AudioController;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::ui::layout::{LayoutStyle, Value};

use super::common::{
    LifecycleEventHandlers, MediaEventHandlers, VisualStyle, WidgetId, WidgetKey, WidgetKind,
};
use super::core::Element;

#[derive(Clone)]
pub struct Audio {
    pub(crate) key: Option<WidgetKey>,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) controller: AudioController,
    pub(crate) autoplay: Value<bool>,
    pub(crate) looping: Value<bool>,
}

impl Audio {
    pub fn new(controller: AudioController) -> Self {
        Self {
            key: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            controller,
            autoplay: Value::Static(false),
            looping: Value::Static(false),
        }
    }

    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn autoplay(mut self, autoplay: impl Into<Value<bool>>) -> Self {
        self.autoplay = autoplay.into();
        self
    }

    pub fn looping(mut self, looping: impl Into<Value<bool>>) -> Self {
        self.looping = looping.into();
        self
    }

    pub fn on_mount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_mount: Some(command),
            ..Default::default()
        })
    }

    pub fn on_unmount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_unmount: Some(command),
            ..Default::default()
        })
    }

    pub fn on_update<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_update: Some(command),
            ..Default::default()
        })
    }

    pub fn on_loading<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_loading: Some(command),
            ..Default::default()
        })
    }

    pub fn on_success<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_success: Some(command),
            ..Default::default()
        })
    }

    pub fn on_error<VM>(self, command: ValueCommand<VM, String>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_error: Some(command),
            ..Default::default()
        })
    }

    fn into_element_with_lifecycle_events<VM>(
        self,
        lifecycle_events: LifecycleEventHandlers<VM>,
    ) -> Element<VM> {
        Element {
            id: WidgetId::next(),
            key: self.key.clone(),
            layout: self.layout.clone(),
            visual: self.visual.clone(),
            interactions: Default::default(),
            lifecycle_events,
            media_events: MediaEventHandlers::default(),
            background: None,
            kind: WidgetKind::Audio { audio: self },
        }
    }

    fn into_element_with_media_events<VM>(
        self,
        media_events: MediaEventHandlers<VM>,
    ) -> Element<VM> {
        Element {
            id: WidgetId::next(),
            key: self.key.clone(),
            layout: self.layout.clone(),
            visual: self.visual.clone(),
            interactions: Default::default(),
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events,
            background: None,
            kind: WidgetKind::Audio { audio: self },
        }
    }
}

impl<VM> From<Audio> for Element<VM> {
    fn from(value: Audio) -> Self {
        Element {
            id: WidgetId::next(),
            key: value.key.clone(),
            layout: value.layout.clone(),
            visual: value.visual.clone(),
            interactions: Default::default(),
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            background: None,
            kind: WidgetKind::Audio { audio: value },
        }
    }
}
