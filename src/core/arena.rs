use super::{ElementId, GenerationalId};
use std::fmt;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::{Index, IndexMut};
use std::slice;

const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
struct Slot {
    generation: u32,
    dense_index: u32,
    next_free: u32,
    retired: bool,
}

impl Slot {
    fn occupied(generation: u32, dense_index: u32) -> Self {
        Self {
            generation,
            dense_index,
            next_free: NONE,
            retired: false,
        }
    }

    fn is_occupied(self) -> bool {
        self.dense_index != NONE
    }
}

#[derive(Clone)]
struct DenseEntry<T> {
    slot: u32,
    value: T,
}

/// Non-allocating dense arena iterator.
pub struct ArenaIter<'a, T, I: GenerationalId> {
    slots: &'a [Slot],
    dense: slice::Iter<'a, DenseEntry<T>>,
    id: PhantomData<fn() -> I>,
}

impl<'a, T, I: GenerationalId> Iterator for ArenaIter<'a, T, I> {
    type Item = (I, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.dense.next()?;
        let generation = self.slots[entry.slot as usize].generation;
        Some((I::from_parts(entry.slot, generation), &entry.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.dense.size_hint()
    }
}

impl<T, I: GenerationalId> ExactSizeIterator for ArenaIter<'_, T, I> {}
impl<T, I: GenerationalId> std::iter::FusedIterator for ArenaIter<'_, T, I> {}

/// Non-allocating mutable dense arena iterator.
pub struct ArenaIterMut<'a, T, I: GenerationalId> {
    slots: &'a [Slot],
    dense: slice::IterMut<'a, DenseEntry<T>>,
    id: PhantomData<fn() -> I>,
}

impl<'a, T, I: GenerationalId> Iterator for ArenaIterMut<'a, T, I> {
    type Item = (I, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.dense.next()?;
        let generation = self.slots[entry.slot as usize].generation;
        Some((I::from_parts(entry.slot, generation), &mut entry.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.dense.size_hint()
    }
}

impl<T, I: GenerationalId> ExactSizeIterator for ArenaIterMut<'_, T, I> {}
impl<T, I: GenerationalId> std::iter::FusedIterator for ArenaIterMut<'_, T, I> {}

/// Allocation and reuse counters for a [`DenseArena`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArenaStats {
    pub live: usize,
    pub peak_live: usize,
    pub slots: usize,
    pub fresh_slot_allocations: u64,
    pub slot_reuses: u64,
    pub releases: u64,
    pub retired_slots: u64,
    pub estimated_reserved_bytes: usize,
}

/// AoS dense generational storage.
///
/// Values are contiguous in `dense`; a separate compact slot table gives IDs a
/// stable slot. Removal uses `swap_remove` and repairs the moved entry's slot.
/// Iteration order is dense order and can therefore change after removal.
pub struct DenseArena<T, I: GenerationalId = ElementId> {
    slots: Vec<Slot>,
    dense: Vec<DenseEntry<T>>,
    free_head: u32,
    counters: ArenaStats,
    id: PhantomData<fn() -> I>,
}

/// Element-specialized spelling used by the retained UI tree.
pub type ElementArena<T> = DenseArena<T, ElementId>;

impl<T, I: GenerationalId> DenseArena<T, I> {
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            dense: Vec::new(),
            free_head: NONE,
            counters: ArenaStats {
                live: 0,
                peak_live: 0,
                slots: 0,
                fresh_slot_allocations: 0,
                slot_reuses: 0,
                releases: 0,
                retired_slots: 0,
                estimated_reserved_bytes: 0,
            },
            id: PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            dense: Vec::with_capacity(capacity),
            ..Self::new()
        }
    }

    /// Allocates a value, panicking only if more than `u32::MAX` slots exist.
    pub fn insert(&mut self, value: T) -> I {
        self.try_insert(value)
            .expect("dense arena exhausted its u32 slot address space")
    }

    /// Alias emphasizing allocation semantics.
    pub fn allocate(&mut self, value: T) -> I {
        self.insert(value)
    }

    pub fn try_insert(&mut self, value: T) -> Option<I> {
        let dense_index = u32::try_from(self.dense.len()).ok()?;

        let (slot_index, generation) = if self.free_head != NONE {
            let slot_index = self.free_head;
            let slot = &mut self.slots[slot_index as usize];
            debug_assert!(!slot.is_occupied() && !slot.retired);
            self.free_head = slot.next_free;
            slot.next_free = NONE;
            slot.dense_index = dense_index;
            self.counters.slot_reuses += 1;
            (slot_index, slot.generation)
        } else {
            let slot_index = u32::try_from(self.slots.len()).ok()?;
            self.slots.push(Slot::occupied(1, dense_index));
            self.counters.fresh_slot_allocations += 1;
            (slot_index, 1)
        };

        self.dense.push(DenseEntry {
            slot: slot_index,
            value,
        });
        self.refresh_stats();
        Some(I::from_parts(slot_index, generation))
    }

    pub fn contains(&self, id: I) -> bool {
        self.dense_index(id).is_some()
    }

    pub fn get(&self, id: I) -> Option<&T> {
        let index = self.dense_index(id)?;
        Some(&self.dense[index].value)
    }

    pub fn get_mut(&mut self, id: I) -> Option<&mut T> {
        let index = self.dense_index(id)?;
        Some(&mut self.dense[index].value)
    }

    /// Releases a value. A stale or foreign-generation ID is a no-op.
    pub fn remove(&mut self, id: I) -> Option<T> {
        let dense_index = self.dense_index(id)?;
        let slot_index = id.slot() as usize;
        let removed = self.dense.swap_remove(dense_index);
        debug_assert_eq!(removed.slot as usize, slot_index);

        if let Some(moved) = self.dense.get(dense_index) {
            self.slots[moved.slot as usize].dense_index = dense_index as u32;
        }

        let slot = &mut self.slots[slot_index];
        slot.dense_index = NONE;
        self.counters.releases += 1;
        if slot.generation == u32::MAX {
            slot.retired = true;
            slot.next_free = NONE;
            self.counters.retired_slots += 1;
        } else {
            slot.generation += 1;
            slot.next_free = self.free_head;
            self.free_head = id.slot();
        }

        self.refresh_stats();
        Some(removed.value)
    }

    pub fn free(&mut self, id: I) -> Option<T> {
        self.remove(id)
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    /// Number of slots ever created, including reusable and retired slots.
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn capacity(&self) -> usize {
        self.dense.capacity()
    }

    pub fn iter(&self) -> ArenaIter<'_, T, I> {
        ArenaIter {
            slots: &self.slots,
            dense: self.dense.iter(),
            id: PhantomData,
        }
    }

    pub fn iter_mut(&mut self) -> ArenaIterMut<'_, T, I> {
        ArenaIterMut {
            slots: &self.slots,
            dense: self.dense.iter_mut(),
            id: PhantomData,
        }
    }

    pub fn ids(&self) -> impl ExactSizeIterator<Item = I> + '_ {
        self.iter().map(|(id, _)| id)
    }

    pub fn values(&self) -> impl ExactSizeIterator<Item = &T> + '_ {
        self.dense.iter().map(|entry| &entry.value)
    }

    pub fn values_mut(&mut self) -> impl ExactSizeIterator<Item = &mut T> + '_ {
        self.dense.iter_mut().map(|entry| &mut entry.value)
    }

    pub fn clear(&mut self) {
        while !self.dense.is_empty() {
            let slot = self.dense[0].slot;
            let id = I::from_parts(slot, self.slots[slot as usize].generation);
            let removed = self.remove(id);
            debug_assert!(removed.is_some());
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(I, &mut T) -> bool) {
        let mut index = 0;
        while index < self.dense.len() {
            let slot_index = self.dense[index].slot;
            let generation = self.slots[slot_index as usize].generation;
            let id = I::from_parts(slot_index, generation);
            if keep(id, &mut self.dense[index].value) {
                index += 1;
            } else {
                let removed = self.remove(id);
                debug_assert!(removed.is_some());
            }
        }
    }

    pub fn stats(&self) -> ArenaStats {
        let mut stats = self.counters;
        stats.live = self.len();
        stats.slots = self.slot_count();
        stats.estimated_reserved_bytes = self.estimated_reserved_bytes();
        stats
    }

    pub fn estimated_reserved_bytes(&self) -> usize {
        self.slots
            .capacity()
            .saturating_mul(size_of::<Slot>())
            .saturating_add(
                self.dense
                    .capacity()
                    .saturating_mul(size_of::<DenseEntry<T>>()),
            )
    }

    fn dense_index(&self, id: I) -> Option<usize> {
        if !id.is_well_formed() {
            return None;
        }
        let slot = *self.slots.get(id.slot() as usize)?;
        if slot.retired || !slot.is_occupied() || slot.generation != id.generation() {
            return None;
        }
        Some(slot.dense_index as usize)
    }

    fn refresh_stats(&mut self) {
        self.counters.live = self.dense.len();
        self.counters.slots = self.slots.len();
        self.counters.peak_live = self.counters.peak_live.max(self.dense.len());
        self.counters.estimated_reserved_bytes = self.estimated_reserved_bytes();
    }
}

impl<T, I: GenerationalId> Default for DenseArena<T, I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone, I: GenerationalId> Clone for DenseArena<T, I> {
    fn clone(&self) -> Self {
        Self {
            slots: self.slots.clone(),
            dense: self.dense.clone(),
            free_head: self.free_head,
            counters: self.counters,
            id: PhantomData,
        }
    }
}

impl<T: fmt::Debug, I: GenerationalId> fmt::Debug for DenseArena<T, I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_map().entries(self.iter()).finish()
    }
}

impl<T, I: GenerationalId> Index<I> for DenseArena<T, I> {
    type Output = T;

    fn index(&self, id: I) -> &Self::Output {
        self.get(id).expect("stale or invalid arena ID")
    }
}

impl<T, I: GenerationalId> IndexMut<I> for DenseArena<T, I> {
    fn index_mut(&mut self, id: I) -> &mut Self::Output {
        self.get_mut(id).expect("stale or invalid arena ID")
    }
}

impl<'a, T, I: GenerationalId> IntoIterator for &'a DenseArena<T, I> {
    type Item = (I, &'a T);
    type IntoIter = ArenaIter<'a, T, I>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, I: GenerationalId> IntoIterator for &'a mut DenseArena<T, I> {
    type Item = (I, &'a mut T);
    type IntoIter = ArenaIterMut<'a, T, I>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::RenderNodeId;

    #[test]
    fn allocation_mutation_iteration_and_release_are_dense() {
        let mut arena = DenseArena::<String, ElementId>::new();
        let a = arena.insert("a".into());
        let b = arena.insert("b".into());
        let c = arena.insert("c".into());
        arena[b].push('!');

        assert_eq!(arena.len(), 3);
        assert_eq!(arena.get(b).map(String::as_str), Some("b!"));
        assert_eq!(
            arena.iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>(),
            ["a", "b!", "c"]
        );

        assert_eq!(arena.remove(b).as_deref(), Some("b!"));
        assert_eq!(arena.len(), 2);
        assert!(arena.contains(a));
        assert!(arena.contains(c));
        assert!(!arena.contains(b));
    }

    #[test]
    fn reuse_invalidates_the_old_generation() {
        let mut arena = DenseArena::<u32, RenderNodeId>::new();
        let old = arena.insert(10);
        assert_eq!(arena.remove(old), Some(10));
        let replacement = arena.insert(20);

        assert_eq!(old.slot(), replacement.slot());
        assert_ne!(old.generation(), replacement.generation());
        assert_eq!(arena.get(old), None);
        assert_eq!(arena.get(replacement), Some(&20));
        assert_eq!(arena.remove(old), None);
    }

    #[test]
    fn retain_and_clear_bump_generations() {
        let mut arena = DenseArena::<u32, ElementId>::new();
        let ids = (0..6).map(|n| arena.insert(n)).collect::<Vec<_>>();
        arena.retain(|_, value| *value % 2 == 0);
        assert_eq!(arena.values().copied().sum::<u32>(), 6);
        assert!(arena.get(ids[1]).is_none());

        let live = arena.ids().collect::<Vec<_>>();
        arena.clear();
        assert!(arena.is_empty());
        assert!(live.into_iter().all(|id| !arena.contains(id)));
    }

    #[test]
    fn stats_distinguish_fresh_slots_and_reuse() {
        let mut arena = DenseArena::<(), ElementId>::with_capacity(2);
        let first = arena.insert(());
        let _second = arena.insert(());
        arena.remove(first);
        let _third = arena.insert(());
        let stats = arena.stats();

        assert_eq!(stats.fresh_slot_allocations, 2);
        assert_eq!(stats.slot_reuses, 1);
        assert_eq!(stats.releases, 1);
        assert_eq!(stats.peak_live, 2);
        assert_eq!(stats.live, 2);
    }
}
