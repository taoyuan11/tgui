use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ReactiveSubscriber {
    Signal(SignalId),
    Target(ReactiveTarget),
}

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
    pub(crate) fn create_signal(&self) -> SignalId {
        let id = SignalId::next();
        self.inner.lock().subscribers.entry(id).or_default();
        id
    }

    pub(crate) fn register_memo(
        &self,
        id: SignalId,
        recompute: Arc<dyn Fn() -> bool + Send + Sync>,
    ) {
        let mut inner = self.inner.lock();
        inner.subscribers.entry(id).or_default();
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
        loop {
            let Some(signal) = self.pop_dirty_signal() else {
                break;
            };
            processed_signals += 1;
            let subscribers = self
                .inner
                .lock()
                .subscribers
                .get(&signal)
                .cloned()
                .unwrap_or_default();

            for subscriber in subscribers {
                match subscriber {
                    ReactiveSubscriber::Signal(signal) => {
                        let recompute = self.inner.lock().recomputers.get(&signal).cloned();
                        let changed = recompute.map(|recompute| recompute()).unwrap_or(true);
                        if changed {
                            self.mark_signal_dirty(signal);
                        }
                    }
                    ReactiveSubscriber::Target(target) => self.mark_target_dirty(target),
                }
            }
        }

        let mut inner = self.inner.lock();
        inner.dirty_target_set.clear();
        let targets = inner.dirty_targets.drain(..).collect();
        ReactiveDrain {
            processed_signals,
            targets,
        }
    }

    fn pop_dirty_signal(&self) -> Option<SignalId> {
        let mut inner = self.inner.lock();
        let signal = inner.dirty_signals.pop_front()?;
        inner.dirty_signal_set.remove(&signal);
        Some(signal)
    }

    fn mark_target_dirty(&self, target: ReactiveTarget) {
        let mut inner = self.inner.lock();
        if inner.dirty_target_set.insert(target) {
            inner.dirty_targets.push_back(target);
        }
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
