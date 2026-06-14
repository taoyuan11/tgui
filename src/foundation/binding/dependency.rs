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

/// Phase 2（属性级依赖归因）：场景阶段被读取的依赖归属到的**具体视觉属性**。
///
/// 当前 `DependencyOwner` 只能定位到「哪个 widget 的哪个阶段」,无法区分改的是
/// 颜色、不透明度还是 transform。`PropertySlot` 把 Scene 阶段进一步细分,使
/// `Signal` 读取在 `track_property_scope` 内被归因到对应属性。这为 Phase 4 的
/// transform-only / 单属性直写快路径提供「脏的是哪个字段」的信息。
///
/// 失效消费侧当前仅读 `widget_id` + `phase`,未被作用域包裹或未识别的属性都安全退化为
/// 整 widget 的 Scene 失效——这是「绝不漏更新」的兜底。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PropertySlot {
    Background,
    BorderColor,
    BorderWidth,
    BorderRadius,
    Opacity,
    Offset,
    Scale,
    TextColor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DependencyOwner {
    pub(crate) widget_id: u64,
    pub(crate) phase: DependencyPhase,
    /// 细分到具体视觉属性。
    pub(crate) property: Option<PropertySlot>,
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

    /// 测试辅助：收集图中出现过的所有 owner（去重前的并集）。
    #[cfg(test)]
    pub(crate) fn all_owners(&self) -> HashSet<DependencyOwner> {
        let mut owners = self.global_owners.clone();
        for set in self.dependencies.values() {
            owners.extend(set.iter().copied());
        }
        owners
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

/// Phase 2（属性级依赖归因）：在当前依赖作用域上叠加一个 `PropertySlot`。
///
/// 复制栈顶 owner（通常是某个 widget 的 `Scene` 阶段 owner）并把它的 `property`
/// 设为 `slot`,在 `f` 执行期间该属性下的 `Signal` 读取都被归因到这个属性。`f`
/// 返回后自动弹出,恢复外层 owner。栈为空时（无外层作用域）直接执行 `f`,不引入
/// 任何 owner——保持与 `record_dependency_read` 一致的「无作用域即不记录」语义。
///
pub(crate) fn track_property_scope<R>(slot: PropertySlot, f: impl FnOnce() -> R) -> R {
    struct ScopeGuard {
        pushed: bool,
    }

    impl Drop for ScopeGuard {
        fn drop(&mut self) {
            if self.pushed {
                DEPENDENCY_TRACKER.with(|tracker| {
                    tracker.borrow_mut().scopes.pop();
                });
            }
        }
    }

    let pushed = DEPENDENCY_TRACKER.with(|tracker| {
        let mut tracker = tracker.borrow_mut();
        let Some(mut owner) = tracker.scopes.last().copied() else {
            return false;
        };
        owner.property = Some(slot);
        tracker.scopes.push(owner);
        true
    });
    let _guard = ScopeGuard { pushed };
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
        let mut graph = DependencyGraph {
            global_owners: std::mem::take(&mut tracker.global_owners),
            ..Default::default()
        };
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
