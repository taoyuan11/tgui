//! Deterministic frame, revision, scene-cost, and cache-budget diagnostics.
//!
//! Diagnostics are observations, not a source of UI truth. The budget manager
//! is generic and headless so tests can exercise eviction and in-flight rules
//! without allocating GPU objects.

use crate::core::{ArenaStats, RevisionSet};
use std::collections::{BTreeMap, HashMap};
use std::error::Error as StdError;
use std::fmt;
use std::hash::Hash;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BudgetDomain {
    CpuCache,
    GpuCache,
    TransientGpu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BudgetFailureReason {
    InvalidLimits,
    ResourceTooLarge,
    HardLimitExceeded,
    NoEvictableResource,
    ArithmeticOverflow,
}

impl fmt::Display for BudgetFailureReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::InvalidLimits => "invalid limits",
            Self::ResourceTooLarge => "resource too large",
            Self::HardLimitExceeded => "hard limit exceeded",
            Self::NoEvictableResource => "no evictable resource",
            Self::ArithmeticOverflow => "arithmetic overflow",
        };
        formatter.write_str(name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetError {
    pub reason: BudgetFailureReason,
    pub requested_bytes: u64,
    pub current_bytes: u64,
    pub hard_limit_bytes: u64,
}

impl fmt::Display for BudgetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} (requested {}, current {}, hard {})",
            self.reason, self.requested_bytes, self.current_bytes, self.hard_limit_bytes
        )
    }
}

impl StdError for BudgetError {}

/// Mutable counters and limits for one cache domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheBudget {
    domain: BudgetDomain,
    soft_limit_bytes: u64,
    hard_limit_bytes: u64,
    current_bytes: u64,
    peak_bytes: u64,
    hits: u64,
    misses: u64,
    evictions: u64,
    evicted_bytes: u64,
    upload_bytes: u64,
    failures: BTreeMap<BudgetFailureReason, u64>,
    committed_references: u64,
    in_flight_references: u64,
    in_flight_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheBudgetSnapshot {
    pub domain: BudgetDomain,
    pub soft_limit_bytes: u64,
    pub hard_limit_bytes: u64,
    pub current_bytes: u64,
    pub peak_bytes: u64,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub evicted_bytes: u64,
    pub upload_bytes: u64,
    pub failures: BTreeMap<BudgetFailureReason, u64>,
    pub committed_references: u64,
    pub in_flight_references: u64,
    pub in_flight_bytes: u64,
}

impl CacheBudget {
    pub fn new(
        domain: BudgetDomain,
        soft_limit_bytes: u64,
        hard_limit_bytes: u64,
    ) -> Result<Self, BudgetError> {
        if soft_limit_bytes > hard_limit_bytes || hard_limit_bytes == 0 {
            return Err(BudgetError {
                reason: BudgetFailureReason::InvalidLimits,
                requested_bytes: soft_limit_bytes,
                current_bytes: 0,
                hard_limit_bytes,
            });
        }
        Ok(Self {
            domain,
            soft_limit_bytes,
            hard_limit_bytes,
            current_bytes: 0,
            peak_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            evicted_bytes: 0,
            upload_bytes: 0,
            failures: BTreeMap::new(),
            committed_references: 0,
            in_flight_references: 0,
            in_flight_bytes: 0,
        })
    }

    pub fn fixed(domain: BudgetDomain, hard_limit_bytes: u64) -> Result<Self, BudgetError> {
        Self::new(domain, hard_limit_bytes, hard_limit_bytes)
    }

    pub const fn domain(&self) -> BudgetDomain {
        self.domain
    }

    pub const fn soft_limit_bytes(&self) -> u64 {
        self.soft_limit_bytes
    }

    pub const fn hard_limit_bytes(&self) -> u64 {
        self.hard_limit_bytes
    }

    pub const fn current_bytes(&self) -> u64 {
        self.current_bytes
    }

    pub const fn peak_bytes(&self) -> u64 {
        self.peak_bytes
    }

    pub const fn hits(&self) -> u64 {
        self.hits
    }

    pub const fn misses(&self) -> u64 {
        self.misses
    }

    pub const fn evictions(&self) -> u64 {
        self.evictions
    }

    pub const fn upload_bytes(&self) -> u64 {
        self.upload_bytes
    }

    pub fn snapshot(&self) -> CacheBudgetSnapshot {
        CacheBudgetSnapshot {
            domain: self.domain,
            soft_limit_bytes: self.soft_limit_bytes,
            hard_limit_bytes: self.hard_limit_bytes,
            current_bytes: self.current_bytes,
            peak_bytes: self.peak_bytes,
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            evicted_bytes: self.evicted_bytes,
            upload_bytes: self.upload_bytes,
            failures: self.failures.clone(),
            committed_references: self.committed_references,
            in_flight_references: self.in_flight_references,
            in_flight_bytes: self.in_flight_bytes,
        }
    }

    fn failure(&mut self, reason: BudgetFailureReason) {
        *self.failures.entry(reason).or_default() += 1;
    }

    fn set_current(&mut self, current: u64) {
        self.current_bytes = current;
        self.peak_bytes = self.peak_bytes.max(current);
    }
}

struct ResourceEntry<V> {
    value: V,
    bytes: u64,
    last_used: u64,
    insertion_order: u64,
    committed_refs: u32,
    in_flight_refs: u32,
}

impl<V> ResourceEntry<V> {
    fn pinned(&self) -> bool {
        self.committed_refs != 0 || self.in_flight_refs != 0
    }
}

/// Fixed-budget LRU/clock-equivalent resource manager used by P0 tests.
pub struct FixedBudgetResourceManager<K, V>
where
    K: Eq + Hash + Clone,
{
    entries: HashMap<K, ResourceEntry<V>>,
    budget: CacheBudget,
    clock: u64,
    insertion_counter: u64,
}

impl<K, V> fmt::Debug for FixedBudgetResourceManager<K, V>
where
    K: Eq + Hash + Clone + fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FixedBudgetResourceManager")
            .field("len", &self.entries.len())
            .field("budget", &self.budget)
            .finish()
    }
}

impl<K, V> FixedBudgetResourceManager<K, V>
where
    K: Eq + Hash + Clone,
{
    pub fn new(
        domain: BudgetDomain,
        soft_limit_bytes: u64,
        hard_limit_bytes: u64,
    ) -> Result<Self, BudgetError> {
        Ok(Self {
            entries: HashMap::new(),
            budget: CacheBudget::new(domain, soft_limit_bytes, hard_limit_bytes)?,
            clock: 0,
            insertion_counter: 0,
        })
    }

    pub fn budget(&self) -> &CacheBudget {
        &self.budget
    }

    pub fn budget_snapshot(&self) -> CacheBudgetSnapshot {
        self.budget.snapshot()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.keys()
    }

    pub fn insert(&mut self, key: K, value: V, bytes: u64) -> Result<(), BudgetError> {
        self.insert_with_upload(key, value, bytes, 0)
    }

    pub fn insert_with_upload(
        &mut self,
        key: K,
        value: V,
        bytes: u64,
        upload_bytes: u64,
    ) -> Result<(), BudgetError> {
        if self.entries.get(&key).is_some_and(ResourceEntry::pinned) {
            self.budget
                .failure(BudgetFailureReason::NoEvictableResource);
            return Err(BudgetError {
                reason: BudgetFailureReason::NoEvictableResource,
                requested_bytes: bytes,
                current_bytes: self.budget.current_bytes,
                hard_limit_bytes: self.budget.hard_limit_bytes,
            });
        }
        if bytes > self.budget.hard_limit_bytes {
            self.budget.failure(BudgetFailureReason::ResourceTooLarge);
            return Err(BudgetError {
                reason: BudgetFailureReason::ResourceTooLarge,
                requested_bytes: bytes,
                current_bytes: self.budget.current_bytes,
                hard_limit_bytes: self.budget.hard_limit_bytes,
            });
        }

        let old_bytes = self.entries.get(&key).map_or(0, |entry| entry.bytes);
        let projected = self
            .budget
            .current_bytes
            .checked_sub(old_bytes)
            .and_then(|value| value.checked_add(bytes))
            .ok_or_else(|| {
                self.budget.failure(BudgetFailureReason::ArithmeticOverflow);
                BudgetError {
                    reason: BudgetFailureReason::ArithmeticOverflow,
                    requested_bytes: bytes,
                    current_bytes: self.budget.current_bytes,
                    hard_limit_bytes: self.budget.hard_limit_bytes,
                }
            })?;

        // Plan evictions before mutating anything, so a hard-limit failure is
        // atomic and cannot discard usable old resources.
        let mut candidates = self
            .entries
            .iter()
            .filter(|(candidate, entry)| *candidate != &key && !entry.pinned())
            .map(|(candidate, entry)| {
                (
                    candidate.clone(),
                    entry.last_used,
                    entry.insertion_order,
                    entry.bytes,
                )
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_, last_used, insertion_order, _)| (*last_used, *insertion_order));

        let mut after = projected;
        let mut plan = Vec::new();
        if after > self.budget.hard_limit_bytes {
            for (candidate, _, _, candidate_bytes) in &candidates {
                after = after.saturating_sub(*candidate_bytes);
                plan.push(candidate.clone());
                if after <= self.budget.hard_limit_bytes {
                    break;
                }
            }
            if after > self.budget.hard_limit_bytes {
                self.budget
                    .failure(BudgetFailureReason::NoEvictableResource);
                return Err(BudgetError {
                    reason: BudgetFailureReason::HardLimitExceeded,
                    requested_bytes: bytes,
                    current_bytes: self.budget.current_bytes,
                    hard_limit_bytes: self.budget.hard_limit_bytes,
                });
            }
        } else if after > self.budget.soft_limit_bytes {
            for (candidate, _, _, candidate_bytes) in &candidates {
                if after <= self.budget.soft_limit_bytes {
                    break;
                }
                after = after.saturating_sub(*candidate_bytes);
                plan.push(candidate.clone());
            }
        }

        for candidate in plan {
            self.evict_unpinned(&candidate);
        }
        if let Some(old) = self.entries.remove(&key) {
            self.remove_reference_counters(&old);
            self.budget
                .set_current(self.budget.current_bytes.saturating_sub(old.bytes));
        }

        self.clock = self.clock.saturating_add(1);
        self.insertion_counter = self.insertion_counter.saturating_add(1);
        self.entries.insert(
            key,
            ResourceEntry {
                value,
                bytes,
                last_used: self.clock,
                insertion_order: self.insertion_counter,
                committed_refs: 0,
                in_flight_refs: 0,
            },
        );
        self.budget
            .set_current(self.budget.current_bytes.saturating_add(bytes));
        self.budget.upload_bytes = self.budget.upload_bytes.saturating_add(upload_bytes);
        Ok(())
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        if self.entries.contains_key(key) {
            self.budget.hits = self.budget.hits.saturating_add(1);
            self.touch(key);
            self.entries.get(key).map(|entry| &entry.value)
        } else {
            self.budget.misses = self.budget.misses.saturating_add(1);
            None
        }
    }

    pub fn peek(&self, key: &K) -> Option<&V> {
        self.entries.get(key).map(|entry| &entry.value)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        if self.entries.get(key).is_some_and(ResourceEntry::pinned) {
            self.budget
                .failure(BudgetFailureReason::NoEvictableResource);
            return None;
        }
        let entry = self.entries.remove(key)?;
        self.budget
            .set_current(self.budget.current_bytes - entry.bytes);
        Some(entry.value)
    }

    pub fn mark_committed(&mut self, key: &K) -> bool {
        let Some(entry) = self.entries.get_mut(key) else {
            return false;
        };
        entry.committed_refs = entry.committed_refs.saturating_add(1);
        self.budget.committed_references = self.budget.committed_references.saturating_add(1);
        true
    }

    pub fn release_committed(&mut self, key: &K) -> bool {
        let Some(entry) = self.entries.get_mut(key) else {
            return false;
        };
        if entry.committed_refs == 0 {
            return false;
        }
        entry.committed_refs -= 1;
        self.budget.committed_references = self.budget.committed_references.saturating_sub(1);
        true
    }

    pub fn mark_in_flight(&mut self, key: &K) -> bool {
        let Some(entry) = self.entries.get_mut(key) else {
            return false;
        };
        entry.in_flight_refs = entry.in_flight_refs.saturating_add(1);
        self.budget.in_flight_references = self.budget.in_flight_references.saturating_add(1);
        if entry.in_flight_refs == 1 {
            self.budget.in_flight_bytes = self.budget.in_flight_bytes.saturating_add(entry.bytes);
        }
        true
    }

    pub fn release_in_flight(&mut self, key: &K) -> bool {
        let Some(entry) = self.entries.get_mut(key) else {
            return false;
        };
        if entry.in_flight_refs == 0 {
            return false;
        }
        entry.in_flight_refs -= 1;
        self.budget.in_flight_references = self.budget.in_flight_references.saturating_sub(1);
        if entry.in_flight_refs == 0 {
            self.budget.in_flight_bytes = self.budget.in_flight_bytes.saturating_sub(entry.bytes);
        }
        true
    }

    pub fn evict_to_soft_limit(&mut self) -> usize {
        let mut removed = 0;
        while self.budget.current_bytes > self.budget.soft_limit_bytes {
            let candidate = self.oldest_unpinned_key();
            let Some(candidate) = candidate else { break };
            if self.evict_unpinned(&candidate) {
                removed += 1;
            } else {
                break;
            }
        }
        removed
    }

    fn touch(&mut self, key: &K) {
        self.clock = self.clock.saturating_add(1);
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_used = self.clock;
        }
    }

    fn oldest_unpinned_key(&self) -> Option<K> {
        self.entries
            .iter()
            .filter(|(_, entry)| !entry.pinned())
            .min_by_key(|(_, entry)| (entry.last_used, entry.insertion_order))
            .map(|(key, _)| key.clone())
    }

    fn evict_unpinned(&mut self, key: &K) -> bool {
        let Some(entry) = self.entries.get(key) else {
            return false;
        };
        if entry.pinned() {
            return false;
        }
        let entry = self.entries.remove(key).expect("entry checked above");
        self.budget
            .set_current(self.budget.current_bytes - entry.bytes);
        self.budget.evictions = self.budget.evictions.saturating_add(1);
        self.budget.evicted_bytes = self.budget.evicted_bytes.saturating_add(entry.bytes);
        true
    }

    fn remove_reference_counters<V2>(&mut self, entry: &ResourceEntry<V2>) {
        self.budget.committed_references = self
            .budget
            .committed_references
            .saturating_sub(u64::from(entry.committed_refs));
        self.budget.in_flight_references = self
            .budget
            .in_flight_references
            .saturating_sub(u64::from(entry.in_flight_refs));
        if entry.in_flight_refs != 0 {
            self.budget.in_flight_bytes = self.budget.in_flight_bytes.saturating_sub(entry.bytes);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RevisionReport {
    pub layout: u64,
    pub scene: u64,
    pub resource: u64,
    pub semantic: u64,
}

impl From<RevisionSet> for RevisionReport {
    fn from(revisions: RevisionSet) -> Self {
        Self {
            layout: revisions.layout.get(),
            scene: revisions.scene.get(),
            resource: revisions.resource.get(),
            semantic: revisions.semantic.get(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhaseMetrics {
    pub update: Duration,
    pub layout: Duration,
    pub paint: Duration,
    pub compile: Duration,
    pub submit: Duration,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneCostReport {
    pub paint_commands: u64,
    pub render_chunks: u64,
    pub batches: u64,
    pub passes: u64,
    pub chunk_rebuilds: u64,
    pub gpu_upload_bytes: u64,
    pub transient_vram_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceCostReport {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub upload_bytes: u64,
    pub failures: u64,
    pub in_flight_references: u64,
}

/// Per-frame allocation sample. Exact heap counters may be supplied by a test
/// allocator; P0 always fills the arena allocation/release fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameAllocationMetrics {
    pub arena_allocations: u64,
    pub arena_releases: u64,
    pub heap_allocations: u64,
    pub allocated_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameMetrics {
    pub frame_index: u64,
    pub revisions: RevisionReport,
    pub phases: PhaseMetrics,
    pub dirty_elements: u64,
    pub full_rebuilds: u64,
    pub incremental_rebuilds: u64,
    pub scene: SceneCostReport,
    pub resources: ResourceCostReport,
    pub arena: ArenaStats,
    pub allocations: FrameAllocationMetrics,
    pub cpu_budget: Option<CacheBudgetSnapshot>,
    pub gpu_budget: Option<CacheBudgetSnapshot>,
    pub transient_budget: Option<CacheBudgetSnapshot>,
}

impl FrameMetrics {
    pub fn empty(frame_index: u64, revisions: RevisionSet) -> Self {
        Self {
            frame_index,
            revisions: revisions.into(),
            phases: PhaseMetrics::default(),
            dirty_elements: 0,
            full_rebuilds: 0,
            incremental_rebuilds: 0,
            scene: SceneCostReport::default(),
            resources: ResourceCostReport::default(),
            arena: ArenaStats::default(),
            allocations: FrameAllocationMetrics::default(),
            cpu_budget: None,
            gpu_budget: None,
            transient_budget: None,
        }
    }

    pub fn with_budget(mut self, budget: &CacheBudget) -> Self {
        match budget.domain() {
            BudgetDomain::CpuCache => self.cpu_budget = Some(budget.snapshot()),
            BudgetDomain::GpuCache => self.gpu_budget = Some(budget.snapshot()),
            BudgetDomain::TransientGpu => self.transient_budget = Some(budget.snapshot()),
        }
        self
    }

    pub fn record_arena(&mut self, stats: ArenaStats) {
        self.arena = stats;
    }

    pub fn record_frame_allocations(&mut self, allocations: FrameAllocationMetrics) {
        self.allocations = allocations;
    }
}

impl Default for FrameMetrics {
    fn default() -> Self {
        Self::empty(0, RevisionSet::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_budget_evicts_lru_but_respects_in_flight_references() {
        let mut cache =
            FixedBudgetResourceManager::<u32, u32>::new(BudgetDomain::CpuCache, 8, 10).unwrap();
        cache.insert(1, 10, 4).unwrap();
        cache.insert(2, 20, 4).unwrap();
        assert_eq!(cache.get(&1), Some(&10));
        cache.mark_in_flight(&1);
        cache.insert(3, 30, 4).unwrap();

        assert!(cache.contains_key(&1));
        assert!(!cache.contains_key(&2));
        assert!(cache.contains_key(&3));
        assert_eq!(cache.budget().evictions(), 1);
        assert_eq!(cache.budget().snapshot().in_flight_references, 1);
    }

    #[test]
    fn too_large_resources_fail_without_mutating_the_cache() {
        let mut cache =
            FixedBudgetResourceManager::<u32, u32>::new(BudgetDomain::GpuCache, 4, 4).unwrap();
        assert!(cache.insert(1, 1, 5).is_err());
        assert!(cache.is_empty());
        assert_eq!(
            cache.budget().snapshot().failures[&BudgetFailureReason::ResourceTooLarge],
            1
        );
    }

    #[test]
    fn committed_or_in_flight_entries_cannot_be_replaced() {
        let mut cache =
            FixedBudgetResourceManager::<u32, u32>::new(BudgetDomain::GpuCache, 8, 8).unwrap();
        cache.insert(1, 10, 4).unwrap();
        assert!(cache.mark_committed(&1));
        assert!(cache.insert(1, 20, 4).is_err());
        assert_eq!(cache.peek(&1), Some(&10));
        assert!(cache.release_committed(&1));
        assert!(cache.mark_in_flight(&1));
        assert!(cache.insert(1, 30, 4).is_err());
        assert_eq!(cache.peek(&1), Some(&10));
    }

    #[test]
    fn frame_metrics_start_as_a_zero_cost_sample() {
        let metrics = FrameMetrics::empty(7, RevisionSet::ZERO);
        assert_eq!(metrics.frame_index, 7);
        assert_eq!(metrics.scene.paint_commands, 0);
        assert!(metrics.cpu_budget.is_none());
    }
}
