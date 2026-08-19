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

/// Soft and hard byte limits for one cache domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheBudgetLimits {
    pub soft_limit_bytes: u64,
    pub hard_limit_bytes: u64,
}

impl CacheBudgetLimits {
    pub const fn new(soft_limit_bytes: u64, hard_limit_bytes: u64) -> Self {
        Self {
            soft_limit_bytes,
            hard_limit_bytes,
        }
    }

    pub fn validate(self) -> Result<Self, BudgetError> {
        CacheBudget::new(
            BudgetDomain::CpuCache,
            self.soft_limit_bytes,
            self.hard_limit_bytes,
        )?;
        Ok(self)
    }
}

/// Default resource limits, optionally replaced on an individual window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceBudgetConfig {
    pub cpu_cache: CacheBudgetLimits,
    pub gpu_cache: CacheBudgetLimits,
    pub transient_gpu: CacheBudgetLimits,
}

impl ResourceBudgetConfig {
    pub const fn new(
        cpu_cache: CacheBudgetLimits,
        gpu_cache: CacheBudgetLimits,
        transient_gpu: CacheBudgetLimits,
    ) -> Self {
        Self {
            cpu_cache,
            gpu_cache,
            transient_gpu,
        }
    }

    pub fn validate(self) -> Result<Self, BudgetError> {
        self.cpu_cache.validate()?;
        self.gpu_cache.validate()?;
        self.transient_gpu.validate()?;
        Ok(self)
    }
}

impl Default for ResourceBudgetConfig {
    fn default() -> Self {
        const MIB: u64 = 1024 * 1024;
        Self::new(
            CacheBudgetLimits::new(64 * MIB, 96 * MIB),
            CacheBudgetLimits::new(128 * MIB, 192 * MIB),
            CacheBudgetLimits::new(64 * MIB, 128 * MIB),
        )
    }
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

    /// Reserves resident or transient bytes. Cache owners should evict their
    /// own unpinned entries before retrying a failed reservation.
    pub fn try_reserve(&mut self, bytes: u64) -> Result<(), BudgetError> {
        let Some(projected) = self.current_bytes.checked_add(bytes) else {
            self.failure(BudgetFailureReason::ArithmeticOverflow);
            return Err(BudgetError {
                reason: BudgetFailureReason::ArithmeticOverflow,
                requested_bytes: bytes,
                current_bytes: self.current_bytes,
                hard_limit_bytes: self.hard_limit_bytes,
            });
        };
        if projected > self.hard_limit_bytes {
            self.failure(BudgetFailureReason::HardLimitExceeded);
            return Err(BudgetError {
                reason: BudgetFailureReason::HardLimitExceeded,
                requested_bytes: bytes,
                current_bytes: self.current_bytes,
                hard_limit_bytes: self.hard_limit_bytes,
            });
        }
        self.set_current(projected);
        Ok(())
    }

    pub fn release(&mut self, bytes: u64) {
        self.set_current(self.current_bytes.saturating_sub(bytes));
    }

    pub fn record_hit(&mut self) {
        self.hits = self.hits.saturating_add(1);
    }

    pub fn record_miss(&mut self) {
        self.misses = self.misses.saturating_add(1);
    }

    pub fn record_upload(&mut self, bytes: u64) {
        self.upload_bytes = self.upload_bytes.saturating_add(bytes);
    }

    pub fn record_eviction(&mut self, bytes: u64) {
        self.evictions = self.evictions.saturating_add(1);
        self.evicted_bytes = self.evicted_bytes.saturating_add(bytes);
        self.release(bytes);
    }

    fn failure(&mut self, reason: BudgetFailureReason) {
        *self.failures.entry(reason).or_default() += 1;
    }

    fn set_current(&mut self, current: u64) {
        self.current_bytes = current;
        self.peak_bytes = self.peak_bytes.max(current);
    }
}

/// Window-owned aggregate accounting for the three resource budget domains.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceBudgets {
    cpu_cache: CacheBudget,
    gpu_cache: CacheBudget,
    transient_gpu: CacheBudget,
}

impl ResourceBudgets {
    pub fn new(config: ResourceBudgetConfig) -> Result<Self, BudgetError> {
        config.validate()?;
        Ok(Self {
            cpu_cache: CacheBudget::new(
                BudgetDomain::CpuCache,
                config.cpu_cache.soft_limit_bytes,
                config.cpu_cache.hard_limit_bytes,
            )?,
            gpu_cache: CacheBudget::new(
                BudgetDomain::GpuCache,
                config.gpu_cache.soft_limit_bytes,
                config.gpu_cache.hard_limit_bytes,
            )?,
            transient_gpu: CacheBudget::new(
                BudgetDomain::TransientGpu,
                config.transient_gpu.soft_limit_bytes,
                config.transient_gpu.hard_limit_bytes,
            )?,
        })
    }

    pub fn get(&self, domain: BudgetDomain) -> &CacheBudget {
        match domain {
            BudgetDomain::CpuCache => &self.cpu_cache,
            BudgetDomain::GpuCache => &self.gpu_cache,
            BudgetDomain::TransientGpu => &self.transient_gpu,
        }
    }

    pub fn get_mut(&mut self, domain: BudgetDomain) -> &mut CacheBudget {
        match domain {
            BudgetDomain::CpuCache => &mut self.cpu_cache,
            BudgetDomain::GpuCache => &mut self.gpu_cache,
            BudgetDomain::TransientGpu => &mut self.transient_gpu,
        }
    }

    pub fn snapshots(&self) -> ResourceBudgetSnapshots {
        ResourceBudgetSnapshots {
            cpu_cache: self.cpu_cache.snapshot(),
            gpu_cache: self.gpu_cache.snapshot(),
            transient_gpu: self.transient_gpu.snapshot(),
        }
    }
}

impl Default for ResourceBudgets {
    fn default() -> Self {
        Self::new(ResourceBudgetConfig::default()).expect("default resource budgets are valid")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceBudgetSnapshots {
    pub cpu_cache: CacheBudgetSnapshot,
    pub gpu_cache: CacheBudgetSnapshot,
    pub transient_gpu: CacheBudgetSnapshot,
}

/// Domain-specific names used by public resource APIs.
pub type CpuCacheBudget = CacheBudget;
pub type GpuCacheBudget = CacheBudget;
pub type TransientGpuBudget = CacheBudget;

struct ResourceEntry<V> {
    value: V,
    bytes: u64,
    last_used: u64,
    insertion_order: u64,
    committed_refs: u32,
    in_flight_refs: u32,
    priority: CacheResourcePriority,
}

/// Eviction hints supplied by a cache owner. References remain authoritative:
/// committed and in-flight entries are never eviction candidates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheResourcePriority {
    /// Relative work needed to recreate the entry. Lower cost evicts first.
    pub rebuild_cost: u32,
    /// Visible entries are retained ahead of otherwise equivalent hidden ones.
    pub visible: bool,
}

impl CacheResourcePriority {
    pub const fn new(rebuild_cost: u32, visible: bool) -> Self {
        Self {
            rebuild_cost,
            visible,
        }
    }
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
        self.insert_with_priority(
            key,
            value,
            bytes,
            upload_bytes,
            CacheResourcePriority::default(),
        )
    }

    pub fn insert_with_priority(
        &mut self,
        key: K,
        value: V,
        bytes: u64,
        upload_bytes: u64,
        priority: CacheResourcePriority,
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
                    entry.priority,
                    entry.bytes,
                )
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_, last_used, insertion_order, priority, _)| {
            (
                priority.visible,
                priority.rebuild_cost,
                *last_used,
                *insertion_order,
            )
        });

        let mut after = projected;
        let mut plan = Vec::new();
        if after > self.budget.hard_limit_bytes {
            for (candidate, _, _, _, candidate_bytes) in &candidates {
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
            for (candidate, _, _, _, candidate_bytes) in &candidates {
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
                priority,
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

    pub fn set_priority(&mut self, key: &K, priority: CacheResourcePriority) -> bool {
        let Some(entry) = self.entries.get_mut(key) else {
            return false;
        };
        entry.priority = priority;
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
            .min_by_key(|(_, entry)| {
                (
                    entry.priority.visible,
                    entry.priority.rebuild_cost,
                    entry.last_used,
                    entry.insertion_order,
                )
            })
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
    pub compiled_cache_hits: u64,
    pub compiled_cache_misses: u64,
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

/// Deduplicated scheduler roots selected from one Dirty Tree epoch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirtyRootMetrics {
    pub structure: u64,
    pub layout: u64,
    pub paint: u64,
    pub hit_test: u64,
    pub semantics: u64,
    pub resource: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameMetrics {
    pub frame_index: u64,
    pub revisions: RevisionReport,
    pub phases: PhaseMetrics,
    pub dirty_elements: u64,
    pub dirty_roots: DirtyRootMetrics,
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
            dirty_roots: DirtyRootMetrics::default(),
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

    pub fn with_resource_budgets(mut self, budgets: &ResourceBudgets) -> Self {
        self.cpu_budget = Some(budgets.cpu_cache.snapshot());
        self.gpu_budget = Some(budgets.gpu_cache.snapshot());
        self.transient_budget = Some(budgets.transient_gpu.snapshot());
        self.resources = ResourceCostReport {
            hits: budgets
                .cpu_cache
                .hits
                .saturating_add(budgets.gpu_cache.hits),
            misses: budgets
                .cpu_cache
                .misses
                .saturating_add(budgets.gpu_cache.misses),
            evictions: budgets
                .cpu_cache
                .evictions
                .saturating_add(budgets.gpu_cache.evictions),
            upload_bytes: budgets
                .cpu_cache
                .upload_bytes
                .saturating_add(budgets.gpu_cache.upload_bytes)
                .saturating_add(budgets.transient_gpu.upload_bytes),
            failures: budgets
                .cpu_cache
                .failures
                .values()
                .chain(budgets.gpu_cache.failures.values())
                .chain(budgets.transient_gpu.failures.values())
                .copied()
                .fold(0_u64, u64::saturating_add),
            in_flight_references: budgets
                .cpu_cache
                .in_flight_references
                .saturating_add(budgets.gpu_cache.in_flight_references)
                .saturating_add(budgets.transient_gpu.in_flight_references),
        };
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
    fn eviction_combines_visibility_rebuild_cost_and_references() {
        let mut cache =
            FixedBudgetResourceManager::<u32, &str>::new(BudgetDomain::GpuCache, 8, 8).unwrap();
        cache
            .insert_with_priority(
                1,
                "visible cheap",
                4,
                0,
                CacheResourcePriority::new(1, true),
            )
            .unwrap();
        cache
            .insert_with_priority(
                2,
                "hidden expensive",
                4,
                0,
                CacheResourcePriority::new(100, false),
            )
            .unwrap();
        assert!(cache.mark_in_flight(&2));
        cache
            .insert_with_priority(3, "new", 4, 0, CacheResourcePriority::default())
            .unwrap();

        assert!(!cache.contains_key(&1));
        assert!(cache.contains_key(&2));
        assert!(cache.contains_key(&3));
        assert_eq!(cache.budget().snapshot().in_flight_bytes, 4);
    }

    #[test]
    fn aggregate_resource_budgets_preserve_domain_snapshots() {
        let config = ResourceBudgetConfig::new(
            CacheBudgetLimits::new(4, 8),
            CacheBudgetLimits::new(8, 16),
            CacheBudgetLimits::new(2, 4),
        );
        let mut budgets = ResourceBudgets::new(config).unwrap();
        budgets
            .get_mut(BudgetDomain::CpuCache)
            .try_reserve(6)
            .unwrap();
        budgets.get_mut(BudgetDomain::GpuCache).record_upload(32);
        let frame = FrameMetrics::default().with_resource_budgets(&budgets);

        assert_eq!(frame.cpu_budget.unwrap().current_bytes, 6);
        assert_eq!(frame.gpu_budget.unwrap().upload_bytes, 32);
        assert_eq!(frame.resources.upload_bytes, 32);
        assert_eq!(frame.transient_budget.unwrap().hard_limit_bytes, 4);
    }

    #[test]
    fn frame_metrics_start_as_a_zero_cost_sample() {
        let metrics = FrameMetrics::empty(7, RevisionSet::ZERO);
        assert_eq!(metrics.frame_index, 7);
        assert_eq!(metrics.scene.paint_commands, 0);
        assert!(metrics.cpu_budget.is_none());
    }
}
