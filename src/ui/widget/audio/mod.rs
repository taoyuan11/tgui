use crate::audio::AudioController;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::ui::layout::{LayoutStyle, Value};

use super::common::{
    LifecycleEventHandlers, MediaEventHandlers, VisualStyle, WidgetId, WidgetKey, WidgetKind,
};
use super::core::Element;

/// 音频播放占位组件。
///
/// 该组件用于把 `AudioController` 挂接到组件树中，并接收生命周期与媒体事件回调。
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
    /// 创建音频组件。
    ///
    /// # 参数
    /// - `controller`：音频控制器。
    ///
    /// # 返回值
    /// 返回新的音频组件。
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

    /// 设置组件 key。
    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// 设置是否自动播放。
    pub fn autoplay(mut self, autoplay: impl Into<Value<bool>>) -> Self {
        self.autoplay = autoplay.into();
        self
    }

    /// 设置是否循环播放。
    pub fn looping(mut self, looping: impl Into<Value<bool>>) -> Self {
        self.looping = looping.into();
        self
    }

    /// 设置挂载命令。
    pub fn on_mount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_mount: Some(command),
            ..Default::default()
        })
    }

    /// 设置卸载命令。
    pub fn on_unmount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_unmount: Some(command),
            ..Default::default()
        })
    }

    /// 设置更新命令。
    pub fn on_update<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_update: Some(command),
            ..Default::default()
        })
    }

    /// 设置加载中命令。
    pub fn on_loading<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_loading: Some(command),
            ..Default::default()
        })
    }

    /// 设置加载成功命令。
    pub fn on_success<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_success: Some(command),
            ..Default::default()
        })
    }

    /// 设置加载失败命令。
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
            focus: Default::default(),
            visual: self.visual.clone(),
            interactions: Default::default(),
            lifecycle_events,
            media_events: MediaEventHandlers::default(),
            background: None,
            tooltip: None,
            menu: None,
            context_menu: None,
            modal: None,
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
            focus: Default::default(),
            visual: self.visual.clone(),
            interactions: Default::default(),
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events,
            background: None,
            tooltip: None,
            menu: None,
            context_menu: None,
            modal: None,
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
            focus: Default::default(),
            visual: value.visual.clone(),
            interactions: Default::default(),
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            background: None,
            tooltip: None,
            menu: None,
            context_menu: None,
            modal: None,
            kind: WidgetKind::Audio { audio: value },
        }
    }
}
