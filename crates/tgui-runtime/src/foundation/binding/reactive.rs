use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use smallvec::SmallVec;

use super::dependency::DependencyOwner;

static NEXT_SIGNAL_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SignalId(u64);

impl SignalId {
    pub(crate) fn next() -> Self {
        Self(NEXT_SIGNAL_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ReactiveTarget {
    Owner(DependencyOwner),
    #[cfg(test)]
    Custom(u64),
}

pub(crate) struct ReactiveDrain {
    pub(crate) processed_signals: usize,
    pub(crate) targets: Vec<ReactiveTarget>,
    #[cfg(test)]
    pub(crate) graph_lock_acquisitions: usize,
    #[cfg(test)]
    pub(crate) scratch_spilled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ReactiveSubscriber {
    Signal(SignalId),
    Target(ReactiveTarget),
}

type ReactiveRecompute = Arc<dyn Fn() -> bool + Send + Sync>;
type ReactiveRecomputeAction = (SignalId, ReactiveRecompute);

const INLINE_DRAIN_FANOUT: usize = 8;

#[derive(Default)]
struct ReactiveGraphInner {
    subscribers: HashMap<SignalId, HashSet<ReactiveSubscriber>>,
    target_sources: HashMap<ReactiveTarget, HashSet<SignalId>>,
    recomputers: HashMap<SignalId, Arc<dyn Fn() -> bool + Send + Sync>>,
    dirty_signals: VecDeque<SignalId>,
    dirty_signal_set: HashSet<SignalId>,
    dirty_targets: VecDeque<ReactiveTarget>,
    dirty_target_set: HashSet<ReactiveTarget>,
}

/// Retained fine-grained dependency graph for signal-driven updates.
///
/// The existing collect-time `DependencyGraph` is still used as a compatibility
/// bridge while retained property slots are introduced. This graph is the new
/// source of truth for direct runtime targets: a changed source signal walks only
/// its retained subscribers, recomputes memo nodes, and queues the concrete
/// targets whose value actually changed.
#[derive(Clone, Default)]
pub(crate) struct ReactiveGraph {
    inner: Arc<parking_lot::Mutex<ReactiveGraphInner>>,
}

impl ReactiveGraph {
    pub(crate) fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn create_signal(&self) -> SignalId {
        SignalId::next()
    }

    pub(crate) fn register_memo(
        &self,
        id: SignalId,
        recompute: Arc<dyn Fn() -> bool + Send + Sync>,
    ) {
        let mut inner = self.inner.lock();
        inner.recomputers.insert(id, recompute);
    }

    pub(crate) fn subscribe_signal(&self, source: SignalId, signal: SignalId) {
        self.subscribe(source, ReactiveSubscriber::Signal(signal));
    }

    pub(crate) fn subscribe_target(&self, source: SignalId, target: ReactiveTarget) {
        self.subscribe(source, ReactiveSubscriber::Target(target));
    }

    fn subscribe(&self, source: SignalId, subscriber: ReactiveSubscriber) {
        let mut inner = self.inner.lock();
        let inserted = inner
            .subscribers
            .entry(source)
            .or_default()
            .insert(subscriber);
        if inserted {
            if let ReactiveSubscriber::Target(target) = subscriber {
                inner
                    .target_sources
                    .entry(target)
                    .or_default()
                    .insert(source);
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn remove_target(&self, target: ReactiveTarget) {
        let mut inner = self.inner.lock();
        remove_target_locked(&mut inner, target);
    }

    pub(crate) fn remove_widget_targets(&self, widget_ids: &HashSet<u64>) {
        if widget_ids.is_empty() {
            return;
        }
        let mut inner = self.inner.lock();
        let targets = inner
            .target_sources
            .keys()
            .copied()
            .filter(|target| match target {
                ReactiveTarget::Owner(owner) => widget_ids.contains(&owner.widget_id),
                #[cfg(test)]
                ReactiveTarget::Custom(_) => false,
            })
            .collect::<Vec<_>>();
        for target in targets {
            remove_target_locked(&mut inner, target);
        }
    }

    pub(crate) fn remove_widget_phase_targets(
        &self,
        widget_ids: &HashSet<u64>,
        phase: super::dependency::DependencyPhase,
    ) {
        if widget_ids.is_empty() {
            return;
        }
        let mut inner = self.inner.lock();
        let targets = inner
            .target_sources
            .keys()
            .copied()
            .filter(|target| match target {
                ReactiveTarget::Owner(owner) => {
                    owner.phase == phase && widget_ids.contains(&owner.widget_id)
                }
                #[cfg(test)]
                ReactiveTarget::Custom(_) => false,
            })
            .collect::<Vec<_>>();
        for target in targets {
            remove_target_locked(&mut inner, target);
        }
    }

    #[cfg(test)]
    pub(crate) fn target_source_count(&self, target: ReactiveTarget) -> usize {
        self.inner
            .lock()
            .target_sources
            .get(&target)
            .map(HashSet::len)
            .unwrap_or(0)
    }

    pub(crate) fn mark_signal_dirty(&self, signal: SignalId) {
        let mut inner = self.inner.lock();
        if inner.subscribers.get(&signal).is_none_or(HashSet::is_empty) {
            return;
        }
        if inner.dirty_signal_set.insert(signal) {
            inner.dirty_signals.push_back(signal);
        }
    }

    #[cfg(test)]
    pub(crate) fn drain_dirty_targets(&self) -> Vec<ReactiveTarget> {
        self.drain().targets
    }

    pub(crate) fn drain(&self) -> ReactiveDrain {
        let mut processed_signals = 0;
        let mut pending_changed_signals = SmallVec::<[SignalId; INLINE_DRAIN_FANOUT]>::new();
        let mut subscribers = SmallVec::<[ReactiveSubscriber; INLINE_DRAIN_FANOUT]>::new();
        let mut recomputes = SmallVec::<[ReactiveRecomputeAction; INLINE_DRAIN_FANOUT]>::new();
        #[cfg(test)]
        let mut graph_lock_acquisitions = 0;

        loop {
            subscribers.clear();
            recomputes.clear();
            #[cfg(test)]
            {
                graph_lock_acquisitions += 1;
            }
            let Some(targets) = self.prepare_next_dirty_signal(
                &mut pending_changed_signals,
                &mut subscribers,
                &mut recomputes,
            ) else {
                processed_signals += 1;
                for (signal, recompute) in &recomputes {
                    if recompute() {
                        pending_changed_signals.push(*signal);
                    }
                }
                continue;
            };
            return ReactiveDrain {
                processed_signals,
                targets,
                #[cfg(test)]
                graph_lock_acquisitions,
                #[cfg(test)]
                scratch_spilled: pending_changed_signals.spilled()
                    || subscribers.spilled()
                    || recomputes.spilled(),
            };
        }
    }

    /// Applies changes produced by the previous recompute batch and prepares the next signal
    /// while holding the graph mutex once. Direct targets and non-memo signal edges are queued
    /// in-place; only memo closures escape the lock for execution.
    fn prepare_next_dirty_signal(
        &self,
        pending_changed_signals: &mut SmallVec<[SignalId; INLINE_DRAIN_FANOUT]>,
        subscribers: &mut SmallVec<[ReactiveSubscriber; INLINE_DRAIN_FANOUT]>,
        recomputes: &mut SmallVec<[ReactiveRecomputeAction; INLINE_DRAIN_FANOUT]>,
    ) -> Option<Vec<ReactiveTarget>> {
        let mut inner = self.inner.lock();
        for signal in pending_changed_signals.drain(..) {
            queue_dirty_signal(&mut inner, signal);
        }

        let Some(signal) = inner.dirty_signals.pop_front() else {
            inner.dirty_target_set.clear();
            return Some(inner.dirty_targets.drain(..).collect());
        };
        inner.dirty_signal_set.remove(&signal);

        if let Some(source_subscribers) = inner.subscribers.get(&signal) {
            subscribers.extend(source_subscribers.iter().copied());
        }
        for subscriber in subscribers.iter().copied() {
            match subscriber {
                ReactiveSubscriber::Signal(signal) => {
                    if let Some(recompute) = inner.recomputers.get(&signal).cloned() {
                        recomputes.push((signal, recompute));
                    } else {
                        queue_dirty_signal(&mut inner, signal);
                    }
                }
                ReactiveSubscriber::Target(target) => queue_dirty_target(&mut inner, target),
            }
        }
        None
    }
}

fn queue_dirty_signal(inner: &mut ReactiveGraphInner, signal: SignalId) {
    if inner.dirty_signal_set.insert(signal) {
        inner.dirty_signals.push_back(signal);
    }
}

fn queue_dirty_target(inner: &mut ReactiveGraphInner, target: ReactiveTarget) {
    if inner.dirty_target_set.insert(target) {
        inner.dirty_targets.push_back(target);
    }
}

fn remove_target_locked(inner: &mut ReactiveGraphInner, target: ReactiveTarget) {
    let Some(sources) = inner.target_sources.remove(&target) else {
        inner.dirty_target_set.remove(&target);
        inner.dirty_targets.retain(|dirty| *dirty != target);
        return;
    };
    let subscriber = ReactiveSubscriber::Target(target);
    for source in sources {
        if let Some(subscribers) = inner.subscribers.get_mut(&source) {
            subscribers.remove(&subscriber);
            if subscribers.is_empty() && !inner.recomputers.contains_key(&source) {
                inner.subscribers.remove(&source);
            }
        }
    }
    inner.dirty_target_set.remove(&target);
    inner.dirty_targets.retain(|dirty| *dirty != target);
}
