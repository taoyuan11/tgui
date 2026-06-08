use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::foundation::view_model::Command;
use crate::ui::layout::Value;

use super::dependency::{record_dependency_read, DependencyId};
use super::invalidation::InvalidationSignal;
use super::{Signal, State, ViewModelContext};

const DEFAULT_TOAST_DURATION: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ToastId(u64);

impl ToastId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Warning,
    Info,
}

pub struct ToastAction<VM> {
    pub label: Value<String>,
    pub on_click: Command<VM>,
}

impl<VM> Clone for ToastAction<VM> {
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            on_click: self.on_click.clone(),
        }
    }
}

impl<VM> ToastAction<VM> {
    pub fn new(label: impl Into<Value<String>>, on_click: Command<VM>) -> Self {
        Self {
            label: label.into(),
            on_click,
        }
    }

    #[allow(dead_code)] // Preserved for nested ViewModel toast actions.
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> ToastAction<RootVm>
    where
        VM: 'static,
    {
        ToastAction {
            label: self.label,
            on_click: self.on_click.scope(selector),
        }
    }
}

pub struct Toast<VM> {
    pub(crate) title: Option<Value<String>>,
    pub(crate) message: Value<String>,
    pub(crate) kind: ToastKind,
    pub(crate) action: Option<ToastAction<VM>>,
    pub(crate) duration: Duration,
    pub(crate) persistent: bool,
    pub(crate) show_close_button: bool,
}

impl<VM> Clone for Toast<VM> {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            message: self.message.clone(),
            kind: self.kind,
            action: self.action.clone(),
            duration: self.duration,
            persistent: self.persistent,
            show_close_button: self.show_close_button,
        }
    }
}

impl<VM> Toast<VM> {
    pub fn new(message: impl Into<Value<String>>) -> Self {
        Self {
            title: None,
            message: message.into(),
            kind: ToastKind::Info,
            action: None,
            duration: DEFAULT_TOAST_DURATION,
            persistent: false,
            show_close_button: true,
        }
    }

    pub fn title(mut self, title: impl Into<Value<String>>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn kind(mut self, kind: ToastKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn action(mut self, action: ToastAction<VM>) -> Self {
        self.action = Some(action);
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn persistent(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }

    pub fn show_close_button(mut self, show: bool) -> Self {
        self.show_close_button = show;
        self
    }

    #[allow(dead_code)] // Preserved for nested ViewModel toast queues.
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> Toast<RootVm>
    where
        VM: 'static,
    {
        Toast {
            title: self.title,
            message: self.message,
            kind: self.kind,
            action: self.action.map(|action| action.scope(selector)),
            duration: self.duration,
            persistent: self.persistent,
            show_close_button: self.show_close_button,
        }
    }
}

pub struct ToastEntry<VM> {
    pub id: ToastId,
    pub toast: Toast<VM>,
    pub created_at: Instant,
    pub deadline: Option<Instant>,
    pub paused_remaining: Option<Duration>,
    pub paused: bool,
}

impl<VM> Clone for ToastEntry<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            toast: self.toast.clone(),
            created_at: self.created_at,
            deadline: self.deadline,
            paused_remaining: self.paused_remaining,
            paused: self.paused,
        }
    }
}

impl<VM> ToastEntry<VM> {
    pub(crate) fn new(id: ToastId, toast: Toast<VM>, now: Instant) -> Self {
        let deadline = (!toast.persistent).then_some(now + toast.duration);
        Self {
            id,
            toast,
            created_at: now,
            deadline,
            paused_remaining: None,
            paused: false,
        }
    }

    pub fn remaining_at(&self, now: Instant) -> Option<Duration> {
        if self.toast.persistent {
            return None;
        }
        if self.paused {
            return self.paused_remaining;
        }
        self.deadline.map(|deadline| {
            if deadline <= now {
                Duration::ZERO
            } else {
                deadline.saturating_duration_since(now)
            }
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastPlacement {
    Adaptive,
    TopStart,
    TopCenter,
    TopEnd,
    BottomStart,
    BottomCenter,
    BottomEnd,
}

pub struct ToastQueue<VM> {
    entries: State<Vec<ToastEntry<VM>>>,
    next_id: State<u64>,
    stack_expanded: State<bool>,
    invalidation: InvalidationSignal,
    dependency: DependencyId,
}

impl<VM> Clone for ToastQueue<VM> {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            next_id: self.next_id.clone(),
            stack_expanded: self.stack_expanded.clone(),
            invalidation: self.invalidation.clone(),
            dependency: self.dependency,
        }
    }
}

impl<VM> ToastQueue<VM> {
    pub fn new(ctx: &ViewModelContext) -> Self {
        Self {
            entries: ctx.state(Vec::new()),
            next_id: ctx.state(1),
            stack_expanded: ctx.state(false),
            invalidation: ctx.invalidation().clone(),
            dependency: DependencyId::next(),
        }
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub fn new_detached() -> Self {
        let invalidation = InvalidationSignal::new();
        Self {
            entries: State::new(Vec::new(), invalidation.clone()),
            next_id: State::new(1, invalidation.clone()),
            stack_expanded: State::new(false, invalidation.clone()),
            invalidation,
            dependency: DependencyId::next(),
        }
    }

    pub fn push(&self, toast: Toast<VM>) -> ToastId {
        let now = Instant::now();
        self.push_at(toast, now)
    }

    pub(crate) fn push_at(&self, toast: Toast<VM>, now: Instant) -> ToastId {
        let id = ToastId::new(self.next_id.update(|next| {
            let id = *next;
            *next += 1;
            id
        }));
        self.entries
            .mutate(|entries| entries.push(ToastEntry::new(id, toast, now)));
        self.invalidation.mark_dependency_dirty(self.dependency);
        id
    }

    pub fn success(&self, message: impl Into<Value<String>>) -> ToastId {
        self.push(Toast::new(message).kind(ToastKind::Success))
    }

    pub fn error(&self, message: impl Into<Value<String>>) -> ToastId {
        self.push(Toast::new(message).kind(ToastKind::Error))
    }

    pub fn warning(&self, message: impl Into<Value<String>>) -> ToastId {
        self.push(Toast::new(message).kind(ToastKind::Warning))
    }

    pub fn info(&self, message: impl Into<Value<String>>) -> ToastId {
        self.push(Toast::new(message).kind(ToastKind::Info))
    }

    pub fn dismiss(&self, id: ToastId) -> bool {
        self.dismiss_at(id, Instant::now())
    }

    pub(crate) fn dismiss_at(&self, id: ToastId, now: Instant) -> bool {
        let changed = self.entries.mutate(|entries| {
            let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) else {
                return false;
            };
            if entry.deadline.is_some_and(|deadline| deadline <= now) && !entry.paused {
                return false;
            }
            entry.deadline = Some(now);
            entry.paused = false;
            entry.paused_remaining = None;
            true
        });
        if changed {
            self.invalidation.mark_dependency_dirty(self.dependency);
        }
        changed
    }

    pub fn clear(&self) {
        self.clear_at(Instant::now());
    }

    pub(crate) fn clear_at(&self, now: Instant) {
        let changed = self.entries.mutate(|entries| {
            let mut changed = false;
            for entry in entries.iter_mut() {
                if entry.deadline.is_some_and(|deadline| deadline <= now) && !entry.paused {
                    continue;
                }
                entry.deadline = Some(now);
                entry.paused = false;
                entry.paused_remaining = None;
                changed = true;
            }
            changed
        });
        if changed {
            self.invalidation.mark_dependency_dirty(self.dependency);
        }
    }

    pub fn is_empty(&self) -> Signal<bool>
    where
        VM: Send + Sync + 'static,
    {
        let queue = self.clone();
        Signal::new_tracked(
            move || queue.entries.read(|entries| entries.is_empty()),
            self.invalidation.clone(),
            Some(self.dependency),
        )
    }

    pub fn entries(&self) -> Signal<Vec<ToastEntry<VM>>>
    where
        VM: Send + Sync + 'static,
    {
        let queue = self.clone();
        Signal::new_tracked(
            move || queue.entries.read(|entries| entries.clone()),
            self.invalidation.clone(),
            Some(self.dependency),
        )
    }

    pub fn snapshot(&self) -> Vec<ToastEntry<VM>> {
        record_dependency_read(Some(self.dependency));
        self.entries.read(|entries| entries.clone())
    }

    pub(crate) fn stack_expanded(&self) -> bool {
        record_dependency_read(Some(self.dependency));
        self.stack_expanded.get()
    }

    pub(crate) fn set_stack_expanded(&self, expanded: bool) {
        let changed = self.stack_expanded.update(|current| {
            let changed = *current != expanded;
            *current = expanded;
            changed
        });
        if changed {
            self.invalidation.mark_dependency_dirty(self.dependency);
        }
    }

    pub fn pause(&self, id: ToastId) -> bool {
        self.pause_at(id, Instant::now())
    }

    pub(crate) fn pause_at(&self, id: ToastId, now: Instant) -> bool {
        let changed = self.entries.mutate(|entries| {
            let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) else {
                return false;
            };
            if entry.toast.persistent
                || entry.paused
                || entry.deadline.is_some_and(|deadline| deadline <= now)
            {
                return false;
            }
            entry.paused_remaining = entry.remaining_at(now);
            entry.deadline = None;
            entry.paused = true;
            true
        });
        changed
    }

    pub fn resume(&self, id: ToastId) -> bool {
        self.resume_at(id, Instant::now())
    }

    pub(crate) fn resume_at(&self, id: ToastId, now: Instant) -> bool {
        let changed = self.entries.mutate(|entries| {
            let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) else {
                return false;
            };
            if entry.toast.persistent || !entry.paused {
                return false;
            }
            let remaining = entry.paused_remaining.unwrap_or(entry.toast.duration);
            entry.deadline = Some(now + remaining);
            entry.paused_remaining = None;
            entry.paused = false;
            true
        });
        changed
    }

    pub fn earliest_deadline(&self) -> Option<Instant> {
        record_dependency_read(Some(self.dependency));
        self.entries.read(|entries| {
            entries
                .iter()
                .filter(|entry| !entry.paused)
                .filter_map(|entry| entry.deadline)
                .min()
        })
    }

    pub fn flush_expired(&self, now: Instant) -> bool {
        /// 退场动画时长，与collect/toast.rs中的EXIT_DURATION_MS保持一致
        const EXIT_DURATION_MS: u64 = 300;

        let changed = self.entries.mutate(|entries| {
            let before = entries.len();
            entries.retain(|entry| match entry.deadline {
                Some(deadline) => {
                    if deadline > now {
                        // deadline未到，保留
                        true
                    } else if entry.paused {
                        // 暂停中，保留
                        true
                    } else {
                        // deadline已到，但需要等待退场动画完成后才移除
                        let exit_elapsed = now.saturating_duration_since(deadline);
                        exit_elapsed.as_millis() < EXIT_DURATION_MS as u128
                    }
                }
                None => true,
            });
            before != entries.len()
        });
        changed
    }
}
