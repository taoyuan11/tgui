use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::platform::backend::event_loop::EventLoopProxy;

use super::dependency::{DependencyId, DependencyPhase, DirtyDependencyLog, DirtyDependencySet};
use super::reactive::{ReactiveDrain, ReactiveGraph, ReactiveTarget, SignalId};

#[derive(Default)]
struct InvalidationWakeState {
    depth: usize,
    pending_wake: bool,
    silent_wake_keys: Vec<usize>,
}

thread_local! {
    static INVALIDATION_WAKE_STATE: RefCell<InvalidationWakeState> = RefCell::new(InvalidationWakeState::default());
}

#[derive(Clone, Default)]
pub(crate) struct InvalidationSignal {
    revisions: Arc<InvalidationRevisions>,
    proxy: Arc<Mutex<Option<EventLoopProxy>>>,
    dirty_dependencies: Arc<Mutex<DirtyDependencyLog>>,
    dependency_revisions: Arc<Mutex<HashMap<DependencyId, u64>>>,
    media_revisions: Arc<Mutex<Vec<u64>>>,
    wake_flags: Arc<InvalidationWakeFlags>,
    reactive_graph: ReactiveGraph,
}

#[derive(Default)]
struct InvalidationRevisions {
    revision: AtomicU64,
    root_rebuild_revision: AtomicU64,
}

#[derive(Default)]
struct InvalidationWakeFlags {
    redraw_requested: std::sync::atomic::AtomicBool,
    wake_queued: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    wake_dispatch_attempts: std::sync::atomic::AtomicUsize,
}

impl InvalidationSignal {
    pub(crate) fn new() -> Self {
        Self {
            revisions: Arc::new(InvalidationRevisions {
                revision: AtomicU64::new(1),
                root_rebuild_revision: AtomicU64::new(0),
            }),
            proxy: Arc::new(Mutex::new(None)),
            dirty_dependencies: Arc::new(Mutex::new(DirtyDependencyLog::default())),
            dependency_revisions: Arc::new(Mutex::new(HashMap::new())),
            media_revisions: Arc::new(Mutex::new(Vec::new())),
            wake_flags: Arc::new(InvalidationWakeFlags::default()),
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
        let revision = self.revisions.revision.fetch_add(1, Ordering::SeqCst) + 1;
        let mut revisions = self.media_revisions.lock();
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

    pub(crate) fn request_redraw(&self) {
        self.wake_flags
            .redraw_requested
            .store(true, Ordering::SeqCst);
        if self.should_wake_now() {
            self.wake_proxy();
        }
    }

    fn mark_dirty_dependency(&self, dependency: Option<DependencyId>) {
        let revision = self.revisions.revision.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(dependency) = dependency {
            self.dependency_revisions
                .lock()
                .insert(dependency, revision);
        }
        self.dirty_dependencies.lock().push(revision, dependency);
        if self.should_wake_now() {
            self.wake_proxy();
        }
    }

    fn wake_proxy(&self) {
        #[cfg(test)]
        self.wake_flags
            .wake_dispatch_attempts
            .fetch_add(1, Ordering::Relaxed);
        if let Some(proxy) = self.proxy.lock().as_ref().cloned() {
            self.try_send_wake(|| proxy.wake_up());
        }
    }

    /// Coalesce invalidation bursts into one pending user event. The event-loop callback
    /// acknowledges the permit before processing work, so an update racing with that callback
    /// can enqueue the next wake instead of being lost.
    fn try_send_wake(&self, send: impl FnOnce() -> bool) -> bool {
        if self
            .wake_flags
            .wake_queued
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }

        if send() {
            true
        } else {
            // A closed/replaced event loop did not consume the permit. Roll it back so a
            // subsequently installed proxy can receive a fresh wake.
            self.wake_flags.wake_queued.store(false, Ordering::Release);
            false
        }
    }

    pub(crate) fn acknowledge_wake(&self) {
        self.wake_flags.wake_queued.store(false, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn debug_try_send_wake(&self, send: impl FnOnce() -> bool) -> bool {
        self.try_send_wake(send)
    }

    #[cfg(test)]
    pub(crate) fn debug_wake_queued(&self) -> bool {
        self.wake_flags.wake_queued.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn debug_wake_dispatch_attempts(&self) -> usize {
        self.wake_flags
            .wake_dispatch_attempts
            .load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn debug_reset_wake_dispatch_attempts(&self) {
        self.wake_flags
            .wake_dispatch_attempts
            .store(0, Ordering::Relaxed);
    }

    fn should_wake_now(&self) -> bool {
        let wake_key = Arc::as_ptr(&self.wake_flags) as usize;
        INVALIDATION_WAKE_STATE.with(|state| {
            let mut state = state.borrow_mut();
            if state.silent_wake_keys.contains(&wake_key) {
                return false;
            }
            if state.depth == 0 {
                true
            } else {
                state.pending_wake = true;
                false
            }
        })
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revisions.revision.load(Ordering::SeqCst)
    }

    pub(crate) fn request_root_rebuild(&self) {
        self.revisions
            .root_rebuild_revision
            .fetch_add(1, Ordering::SeqCst);
        // A cloned/scoped CommandContext may outlive the synchronous command dispatch. Make the
        // rebuild request self-waking instead of relying on the executor's trailing invalidation.
        self.mark_dirty();
    }

    pub(crate) fn root_rebuild_revision(&self) -> u64 {
        self.revisions.root_rebuild_revision.load(Ordering::SeqCst)
    }

    pub(crate) fn set_proxy(&self, proxy: EventLoopProxy) {
        *self.proxy.lock() = Some(proxy);
    }

    pub(crate) fn suppress_wakeups(&self) -> InvalidationWakeGuard {
        INVALIDATION_WAKE_STATE.with(|state| {
            state.borrow_mut().depth += 1;
        });
        InvalidationWakeGuard {
            signal: self.clone(),
            silent_wake_key: None,
        }
    }

    /// Suppress event-loop proxy wakeups for a synchronous batch that is already running on the
    /// event-loop thread. Revision counters, dependency logs and the reactive graph are still
    /// updated normally; the caller is responsible for requesting a redraw or otherwise draining
    /// the resulting work before returning to the event loop.
    pub(crate) fn suppress_wakeups_without_dispatch(&self) -> InvalidationWakeGuard {
        let wake_key = Arc::as_ptr(&self.wake_flags) as usize;
        INVALIDATION_WAKE_STATE.with(|state| {
            state.borrow_mut().silent_wake_keys.push(wake_key);
        });
        InvalidationWakeGuard {
            signal: self.clone(),
            silent_wake_key: Some(wake_key),
        }
    }

    pub(crate) fn dirty_dependencies_since(
        &self,
        revision: u64,
    ) -> (DirtyDependencySet, HashSet<DependencyId>) {
        let current_revision = self.revision();
        self.dirty_dependencies
            .lock()
            .dirty_since(revision, current_revision)
    }

    pub(crate) fn dependency_revision(&self, dependency: DependencyId) -> Option<u64> {
        self.dependency_revisions.lock().get(&dependency).copied()
    }

    pub(crate) fn media_only_since(&self, revision: u64, current_revision: u64) -> bool {
        if revision >= current_revision {
            return false;
        }
        let revisions = self.media_revisions.lock();
        let media_count = revisions
            .iter()
            .filter(|&&media_revision| {
                media_revision > revision && media_revision <= current_revision
            })
            .count() as u64;
        media_count > 0 && media_count == current_revision.saturating_sub(revision)
    }

    pub(crate) fn take_redraw_request(&self) -> bool {
        self.wake_flags
            .redraw_requested
            .swap(false, Ordering::SeqCst)
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

    #[allow(dead_code)]
    pub(crate) fn remove_reactive_target(&self, target: ReactiveTarget) {
        self.reactive_graph.remove_target(target);
    }

    #[allow(dead_code)]
    pub(crate) fn replace_reactive_target<R>(
        &self,
        target: ReactiveTarget,
        collect: impl FnOnce() -> R,
    ) -> R {
        self.remove_reactive_target(target);
        collect()
    }

    pub(crate) fn remove_reactive_targets_for_widgets(&self, widget_ids: &HashSet<u64>) {
        self.reactive_graph.remove_widget_targets(widget_ids);
    }

    pub(crate) fn remove_reactive_targets_for_widget_phase(
        &self,
        widget_ids: &HashSet<u64>,
        phase: DependencyPhase,
    ) {
        self.reactive_graph
            .remove_widget_phase_targets(widget_ids, phase);
    }

    #[cfg(test)]
    pub(crate) fn reactive_target_source_count(&self, target: ReactiveTarget) -> usize {
        self.reactive_graph.target_source_count(target)
    }
}

pub(crate) struct InvalidationWakeGuard {
    signal: InvalidationSignal,
    silent_wake_key: Option<usize>,
}

impl Drop for InvalidationWakeGuard {
    fn drop(&mut self) {
        if let Some(wake_key) = self.silent_wake_key {
            INVALIDATION_WAKE_STATE.with(|state| {
                let mut state = state.borrow_mut();
                let index = state
                    .silent_wake_keys
                    .iter()
                    .rposition(|candidate| *candidate == wake_key)
                    .expect("silent invalidation wake scope must remain registered");
                state.silent_wake_keys.swap_remove(index);
            });
            return;
        }

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
