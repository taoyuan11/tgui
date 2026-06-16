use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::platform::backend::event_loop::EventLoopProxy;

use super::dependency::{DependencyId, DirtyDependencyLog, DirtyDependencySet};
#[cfg(test)]
use super::reactive::ReactiveTarget;
use super::reactive::{ReactiveDrain, ReactiveGraph, SignalId};

#[derive(Default)]
struct InvalidationWakeState {
    depth: usize,
    pending_wake: bool,
}

thread_local! {
    static INVALIDATION_WAKE_STATE: RefCell<InvalidationWakeState> = RefCell::new(InvalidationWakeState::default());
}

#[derive(Clone, Default)]
pub(crate) struct InvalidationSignal {
    revision: Arc<AtomicU64>,
    proxy: Arc<Mutex<Option<EventLoopProxy>>>,
    dirty_dependencies: Arc<Mutex<DirtyDependencyLog>>,
    dependency_revisions: Arc<Mutex<HashMap<DependencyId, u64>>>,
    media_revisions: Arc<Mutex<Vec<u64>>>,
    redraw_requested: Arc<std::sync::atomic::AtomicBool>,
    reactive_graph: ReactiveGraph,
}

impl InvalidationSignal {
    pub(crate) fn new() -> Self {
        Self {
            revision: Arc::new(AtomicU64::new(1)),
            proxy: Arc::new(Mutex::new(None)),
            dirty_dependencies: Arc::new(Mutex::new(DirtyDependencyLog::default())),
            dependency_revisions: Arc::new(Mutex::new(HashMap::new())),
            media_revisions: Arc::new(Mutex::new(Vec::new())),
            redraw_requested: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            reactive_graph: ReactiveGraph::default(),
        }
    }

    pub(crate) fn mark_dirty(&self) {
        self.mark_dirty_dependency(None);
    }

    pub(crate) fn mark_dependency_dirty(&self, dependency: DependencyId) {
        self.mark_dirty_dependency(Some(dependency));
    }

    pub(crate) fn mark_signal_dirty(&self, signal: SignalId) {
        self.reactive_graph.mark_signal_dirty(signal);
    }

    pub(crate) fn mark_media_dirty(&self) {
        let revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        let mut revisions = self
            .media_revisions
            .lock()
            .expect("media revision log lock poisoned");
        revisions.push(revision);
        if revisions.len() > 1024 {
            let overflow = revisions.len() - 1024;
            revisions.drain(0..overflow);
        }
        drop(revisions);
        if self.should_wake_now() {
            self.wake_proxy();
        }
    }

    #[cfg(feature = "video")]
    pub(crate) fn request_redraw(&self) {
        self.redraw_requested.store(true, Ordering::SeqCst);
        if self.should_wake_now() {
            self.wake_proxy();
        }
    }

    fn mark_dirty_dependency(&self, dependency: Option<DependencyId>) {
        let revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(dependency) = dependency {
            self.dependency_revisions
                .lock()
                .expect("dependency revision map lock poisoned")
                .insert(dependency, revision);
        }
        self.dirty_dependencies
            .lock()
            .expect("dirty dependency log lock poisoned")
            .push(revision, dependency);
        if self.should_wake_now() {
            self.wake_proxy();
        }
    }

    fn wake_proxy(&self) {
        if let Some(proxy) = self
            .proxy
            .lock()
            .expect("invalidation proxy lock poisoned")
            .as_ref()
            .cloned()
        {
            proxy.wake_up();
        }
    }

    fn should_wake_now(&self) -> bool {
        INVALIDATION_WAKE_STATE.with(|state| {
            let mut state = state.borrow_mut();
            if state.depth == 0 {
                true
            } else {
                state.pending_wake = true;
                false
            }
        })
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    pub(crate) fn set_proxy(&self, proxy: EventLoopProxy) {
        *self.proxy.lock().expect("invalidation proxy lock poisoned") = Some(proxy);
    }

    pub(crate) fn suppress_wakeups(&self) -> InvalidationWakeGuard {
        INVALIDATION_WAKE_STATE.with(|state| {
            state.borrow_mut().depth += 1;
        });
        InvalidationWakeGuard {
            signal: self.clone(),
        }
    }

    pub(crate) fn dirty_dependencies_since(
        &self,
        revision: u64,
    ) -> (DirtyDependencySet, HashSet<DependencyId>) {
        let current_revision = self.revision();
        self.dirty_dependencies
            .lock()
            .expect("dirty dependency log lock poisoned")
            .dirty_since(revision, current_revision)
    }

    pub(crate) fn dependency_revision(&self, dependency: DependencyId) -> Option<u64> {
        self.dependency_revisions
            .lock()
            .expect("dependency revision map lock poisoned")
            .get(&dependency)
            .copied()
    }

    pub(crate) fn media_only_since(&self, revision: u64, current_revision: u64) -> bool {
        if revision >= current_revision {
            return false;
        }
        let revisions = self
            .media_revisions
            .lock()
            .expect("media revision log lock poisoned");
        let media_count = revisions
            .iter()
            .filter(|&&media_revision| {
                media_revision > revision && media_revision <= current_revision
            })
            .count() as u64;
        media_count > 0 && media_count == current_revision.saturating_sub(revision)
    }

    pub(crate) fn take_redraw_request(&self) -> bool {
        self.redraw_requested.swap(false, Ordering::SeqCst)
    }

    pub(crate) fn reactive_graph(&self) -> ReactiveGraph {
        self.reactive_graph.clone()
    }

    #[cfg(test)]
    pub(crate) fn drain_reactive_targets(&self) -> Vec<ReactiveTarget> {
        self.reactive_graph.drain_dirty_targets()
    }

    pub(crate) fn drain_reactive_updates(&self) -> ReactiveDrain {
        self.reactive_graph.drain()
    }
}

pub(crate) struct InvalidationWakeGuard {
    signal: InvalidationSignal,
}

impl Drop for InvalidationWakeGuard {
    fn drop(&mut self) {
        let should_wake = INVALIDATION_WAKE_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.depth = state.depth.saturating_sub(1);
            if state.depth == 0 && state.pending_wake {
                state.pending_wake = false;
                true
            } else {
                false
            }
        });

        if should_wake {
            self.signal.wake_proxy();
        }
    }
}
