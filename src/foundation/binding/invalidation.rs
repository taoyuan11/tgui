use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::platform::backend::event_loop::EventLoopProxy;

use super::dependency::{DependencyId, DirtyDependencyLog, DirtyDependencySet};

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
}

impl InvalidationSignal {
    pub(crate) fn new() -> Self {
        Self {
            revision: Arc::new(AtomicU64::new(1)),
            proxy: Arc::new(Mutex::new(None)),
            dirty_dependencies: Arc::new(Mutex::new(DirtyDependencyLog::default())),
            dependency_revisions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn mark_dirty(&self) {
        self.mark_dirty_dependency(None);
    }

    pub(crate) fn mark_dependency_dirty(&self, dependency: DependencyId) {
        self.mark_dirty_dependency(Some(dependency));
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
