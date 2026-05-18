use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DEPENDENCY_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DependencyId(u64);

impl DependencyId {
    pub(crate) fn next() -> Self {
        Self(NEXT_DEPENDENCY_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum DependencyPhase {
    Structure,
    Layout,
    Scene,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DependencyOwner {
    pub(crate) widget_id: u64,
    pub(crate) phase: DependencyPhase,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DependencyGraph {
    dependencies: HashMap<DependencyId, HashSet<DependencyOwner>>,
    global_owners: HashSet<DependencyOwner>,
}

impl DependencyGraph {
    pub(crate) fn owners_for(&self, dependency: DependencyId) -> Option<&HashSet<DependencyOwner>> {
        self.dependencies.get(&dependency)
    }

    pub(crate) fn has_global_dependency(&self) -> bool {
        !self.global_owners.is_empty()
    }

    #[allow(dead_code)]
    pub(crate) fn dependency_count(&self) -> usize {
        self.dependencies.len()
    }

    #[cfg(test)]
    pub(crate) fn contains_owner(&self, owner: DependencyOwner) -> bool {
        self.dependencies
            .values()
            .any(|owners| owners.contains(&owner))
    }

    pub(crate) fn merge_from(&mut self, other: &DependencyGraph) {
        self.global_owners
            .extend(other.global_owners.iter().copied());
        for (dependency, owners) in &other.dependencies {
            self.dependencies
                .entry(*dependency)
                .or_default()
                .extend(owners.iter().copied());
        }
    }

    pub(crate) fn remove_widget_owners(&mut self, widget_ids: &HashSet<u64>) {
        if widget_ids.is_empty() {
            return;
        }
        self.global_owners
            .retain(|owner| !widget_ids.contains(&owner.widget_id));
        self.dependencies.retain(|_, owners| {
            owners.retain(|owner| !widget_ids.contains(&owner.widget_id));
            !owners.is_empty()
        });
    }

    pub(crate) fn remove_widget_phase_owners(
        &mut self,
        widget_ids: &HashSet<u64>,
        phase: DependencyPhase,
    ) {
        if widget_ids.is_empty() {
            return;
        }
        self.global_owners
            .retain(|owner| !(owner.phase == phase && widget_ids.contains(&owner.widget_id)));
        self.dependencies.retain(|_, owners| {
            owners.retain(|owner| !(owner.phase == phase && widget_ids.contains(&owner.widget_id)));
            !owners.is_empty()
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirtyDependencySet {
    Clean,
    Global,
    Dependencies {
        from_revision: u64,
        to_revision: u64,
    },
}

#[derive(Debug)]
struct DirtyDependencyEntry {
    revision: u64,
    dependency: Option<DependencyId>,
}

#[derive(Debug, Default)]
pub(super) struct DirtyDependencyLog {
    entries: Vec<DirtyDependencyEntry>,
}

const MAX_DIRTY_DEPENDENCY_ENTRIES: usize = 1024;

impl DirtyDependencyLog {
    pub(super) fn push(&mut self, revision: u64, dependency: Option<DependencyId>) {
        self.entries.push(DirtyDependencyEntry {
            revision,
            dependency,
        });
        if self.entries.len() > MAX_DIRTY_DEPENDENCY_ENTRIES {
            let overflow = self.entries.len() - MAX_DIRTY_DEPENDENCY_ENTRIES;
            self.entries.drain(0..overflow);
        }
    }

    pub(super) fn dirty_since(
        &self,
        revision: u64,
        current_revision: u64,
    ) -> (DirtyDependencySet, HashSet<DependencyId>) {
        if revision == current_revision {
            return (DirtyDependencySet::Clean, HashSet::new());
        }
        let Some(first) = self.entries.first() else {
            return (DirtyDependencySet::Global, HashSet::new());
        };
        if revision != 0 && first.revision > revision.saturating_add(1) {
            return (DirtyDependencySet::Global, HashSet::new());
        }

        let mut dependencies = HashSet::new();
        for entry in self
            .entries
            .iter()
            .filter(|entry| entry.revision > revision)
        {
            let Some(dependency) = entry.dependency else {
                return (DirtyDependencySet::Global, HashSet::new());
            };
            dependencies.insert(dependency);
        }

        (
            DirtyDependencySet::Dependencies {
                from_revision: revision,
                to_revision: current_revision,
            },
            dependencies,
        )
    }
}

#[derive(Default)]
struct DependencyTracker {
    scopes: Vec<DependencyOwner>,
    records: Vec<(DependencyId, DependencyOwner)>,
    global_owners: HashSet<DependencyOwner>,
}

thread_local! {
    static DEPENDENCY_TRACKER: RefCell<DependencyTracker> = RefCell::new(DependencyTracker::default());
}

pub(crate) fn track_dependency_scope<R>(owner: DependencyOwner, f: impl FnOnce() -> R) -> R {
    struct ScopeGuard;

    impl Drop for ScopeGuard {
        fn drop(&mut self) {
            DEPENDENCY_TRACKER.with(|tracker| {
                tracker.borrow_mut().scopes.pop();
            });
        }
    }

    DEPENDENCY_TRACKER.with(|tracker| {
        tracker.borrow_mut().scopes.push(owner);
    });
    let _guard = ScopeGuard;
    f()
}

pub(crate) fn with_dependency_collection<R>(f: impl FnOnce() -> R) -> (R, DependencyGraph) {
    DEPENDENCY_TRACKER.with(|tracker| {
        let mut tracker = tracker.borrow_mut();
        tracker.records.clear();
        tracker.global_owners.clear();
    });

    let result = f();
    let graph = DEPENDENCY_TRACKER.with(|tracker| {
        let mut tracker = tracker.borrow_mut();
        let mut graph = DependencyGraph::default();
        graph.global_owners = std::mem::take(&mut tracker.global_owners);
        for (dependency, owner) in tracker.records.drain(..) {
            graph
                .dependencies
                .entry(dependency)
                .or_default()
                .insert(owner);
        }
        graph
    });
    (result, graph)
}

pub(crate) fn record_dependency_read(dependency: Option<DependencyId>) {
    DEPENDENCY_TRACKER.with(|tracker| {
        let mut tracker = tracker.borrow_mut();
        let Some(owner) = tracker.scopes.last().copied() else {
            return;
        };
        if let Some(dependency) = dependency {
            tracker.records.push((dependency, owner));
        } else {
            tracker.global_owners.insert(owner);
        }
    });
}
