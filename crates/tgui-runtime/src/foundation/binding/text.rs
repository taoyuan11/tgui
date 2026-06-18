use std::sync::Arc;

use super::dependency::{current_dependency_owner, record_dependency_read, DependencyId};
use super::invalidation::InvalidationSignal;
use super::reactive::{ReactiveTarget, SignalId};

/// 表示某一时刻的文本快照。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSnapshot {
    /// 当前完整文本内容。
    pub text: String,
    /// 对应的文本修订号。
    pub revision: u64,
}

/// 表示一次文本替换变更。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextChange {
    /// 被替换的字节区间。
    pub range_bytes: (usize, usize),
    /// 插入的新文本内容。
    pub inserted_text: String,
}

impl TextChange {
    /// 创建一条文本变更。
    ///
    /// 参数:
    /// - `range_bytes`: 被替换的 UTF-8 字节区间。
    /// - `inserted_text`: 要插入的新文本。
    ///
    /// 返回值: 构造好的文本变更对象。
    pub fn new(range_bytes: (usize, usize), inserted_text: impl Into<String>) -> Self {
        Self {
            range_bytes,
            inserted_text: inserted_text.into(),
        }
    }
}

/// 表示由多次文本替换组成的批量变更。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextChangeSet {
    /// 批量变更开始前的修订号。
    pub start_revision: u64,
    /// 批量变更结束后的修订号。
    pub end_revision: u64,
    /// 按顺序记录的变更列表。
    pub changes: Vec<TextChange>,
}

/// 提供保留式文本读写能力的控制器。
#[derive(Clone)]
pub struct TextController {
    state: Arc<parking_lot::Mutex<TextControllerState>>,
    invalidation: InvalidationSignal,
    dependency: DependencyId,
    signal_id: SignalId,
}

#[derive(Debug)]
struct TextControllerState {
    text: String,
    revision: u64,
}

impl TextController {
    pub(crate) fn new(initial_text: impl Into<String>, invalidation: InvalidationSignal) -> Self {
        let signal_id = invalidation.reactive_graph().create_signal();
        Self {
            state: Arc::new(parking_lot::Mutex::new(TextControllerState {
                text: initial_text.into(),
                revision: 1,
            })),
            invalidation,
            dependency: DependencyId::next(),
            signal_id,
        }
    }

    pub(crate) fn new_legacy(initial_text: impl Into<crate::ui::layout::Value<String>>) -> Self {
        Self::new(initial_text.into().resolve(), InvalidationSignal::new())
    }

    /// 读取当前完整文本。
    ///
    /// 返回值: 当前文本内容的克隆副本。
    pub fn text(&self) -> String {
        self.track_read();
        self.state.lock().text.clone()
    }

    /// 以借用方式读取当前完整文本。
    pub fn with_text<R>(&self, reader: impl FnOnce(&str) -> R) -> R {
        self.track_read();
        let state = self.state.lock();
        reader(&state.text)
    }

    /// 读取当前文本快照。
    ///
    /// 返回值: 包含文本内容和修订号的快照。
    pub fn snapshot(&self) -> TextSnapshot {
        self.track_read();
        let state = self.state.lock();
        TextSnapshot {
            text: state.text.clone(),
            revision: state.revision,
        }
    }

    /// 读取当前文本修订号。
    ///
    /// 返回值: 当前修订号。
    pub fn revision(&self) -> u64 {
        self.track_read();
        self.state.lock().revision
    }

    /// 用新文本替换当前内容。
    ///
    /// 参数:
    /// - `text`: 新的完整文本内容。
    pub fn set_text(&self, text: impl Into<String>) {
        if self.set_text_silent(text) {
            self.invalidation.mark_signal_dirty(self.signal_id);
            self.invalidation.mark_dependency_dirty(self.dependency);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_text_assuming_changed(&self, text: impl Into<String>) -> u64 {
        let mut state = self.state.lock();
        state.text = text.into();
        state.revision = state.revision.wrapping_add(1).max(1);
        self.invalidation.mark_signal_dirty(self.signal_id);
        self.invalidation.mark_dependency_dirty(self.dependency);
        state.revision
    }

    pub(crate) fn set_text_local_assuming_changed(&self, text: impl Into<String>) -> u64 {
        let mut state = self.state.lock();
        state.text = text.into();
        state.revision = state.revision.wrapping_add(1).max(1);
        state.revision
    }

    /// 将当前内容整体替换为指定文本。
    ///
    /// 参数:
    /// - `text`: 新的完整文本内容。
    pub fn replace_all(&self, text: impl Into<String>) {
        self.set_text(text);
    }

    pub(crate) fn replace_text_silent(&self, text: impl Into<String>) -> u64 {
        let text = text.into();
        let mut state = self.state.lock();
        if state.text != text {
            state.text = text;
            state.revision = state.revision.wrapping_add(1).max(1);
        }
        state.revision
    }

    pub(crate) fn set_text_silent(&self, text: impl Into<String>) -> bool {
        let previous = self.state.lock().revision;
        let next = self.replace_text_silent(text);
        next != previous
    }

    fn track_read(&self) {
        record_dependency_read(Some(self.dependency));
        if let Some(owner) = current_dependency_owner() {
            self.invalidation
                .reactive_graph()
                .subscribe_target(self.signal_id, ReactiveTarget::Owner(owner));
        }
    }
}

impl From<String> for TextController {
    fn from(value: String) -> Self {
        TextController::new_legacy(value)
    }
}

impl From<&str> for TextController {
    fn from(value: &str) -> Self {
        TextController::new_legacy(value)
    }
}

impl From<super::signal_state::Signal<String>> for TextController {
    fn from(value: super::signal_state::Signal<String>) -> Self {
        TextController::new_legacy(crate::ui::layout::Value::Signal(value))
    }
}

impl From<crate::ui::layout::Value<String>> for TextController {
    fn from(value: crate::ui::layout::Value<String>) -> Self {
        TextController::new_legacy(value)
    }
}
