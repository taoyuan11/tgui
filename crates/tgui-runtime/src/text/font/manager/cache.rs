use std::cell::Cell;
use std::hash::Hash;

use hashbrown::{Equivalent, HashMap};

struct Entry<V> {
    value: V,
    touched: Cell<u64>,
    bytes: usize,
}

/// Bounded, sampled-LRU cache. On overflow evict the older of two sampled entries.
/// Unlike a strict LRU, a sequential scan of capacity + 1 keys does not evict the
/// next key on every access. Hits remain allocation-free and eviction is O(1),
/// without scanning the map or dropping an entire working set at once.
pub(super) struct TextCache<K, V> {
    entries: HashMap<K, Entry<V>>,
    keys: Vec<K>,
    capacity: usize,
    byte_budget: usize,
    bytes: usize,
    clock: Cell<u64>,
    random: u64,
}

impl<K: Clone + Eq + Hash, V> TextCache<K, V> {
    pub(super) fn new(capacity: usize, byte_budget: usize) -> Self {
        assert!(capacity > 0);
        Self {
            entries: HashMap::new(),
            keys: Vec::new(),
            capacity,
            byte_budget,
            bytes: 0,
            clock: Cell::new(0),
            random: 0x9e37_79b9_7f4a_7c15,
        }
    }

    fn tick(&self) -> u64 {
        let next = self.clock.get().saturating_add(1);
        self.clock.set(next);
        next
    }

    pub(super) fn get<Q: Hash + Equivalent<K> + ?Sized>(&self, key: &Q) -> Option<&V> {
        let entry = self.entries.get(key)?;
        entry.touched.set(self.tick());
        Some(&entry.value)
    }

    /// `bytes` accounts for the owned key and value's heap storage. Oversized
    /// results are returned to the caller but do not displace the working set.
    pub(super) fn insert(&mut self, key: K, value: V, bytes: usize) {
        let bytes = bytes.saturating_add(std::mem::size_of::<(K, Entry<V>)>());
        if bytes > self.byte_budget || self.entries.contains_key(&key) {
            return;
        }
        while self.entries.len() >= self.capacity || self.bytes > self.byte_budget - bytes {
            let a = self.sample();
            let b = self.sample();
            let victim = if self.entries[&self.keys[a]].touched.get()
                <= self.entries[&self.keys[b]].touched.get()
            {
                a
            } else {
                b
            };
            let key = self.keys.swap_remove(victim);
            self.bytes -= self.entries.remove(&key).expect("resident cache key").bytes;
        }
        self.keys.push(key.clone());
        let touched = Cell::new(self.tick());
        self.entries.insert(
            key,
            Entry {
                value,
                touched,
                bytes,
            },
        );
        self.bytes += bytes;
    }

    fn sample(&mut self) -> usize {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        (self.random as usize) % self.keys.len()
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(feature = "bench-support")]
    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.keys.clear();
        self.bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyclic_overflow_retains_hits_and_stays_bounded() {
        for count in [65, 128] {
            let mut cache = TextCache::new(64, usize::MAX);
            let mut hits = 0;
            for _ in 0..10 {
                for key in 0..count {
                    if let Some(value) = cache.get(&key) {
                        assert_eq!(*value, key * 3);
                        hits += 1;
                    } else {
                        cache.insert(key, key * 3, 0);
                    }
                    assert!(cache.len() <= 64);
                }
            }
            assert!(hits > 0, "a cyclic scan must not continuously miss");
            if count == 65 {
                assert!(hits > 500, "near-capacity scans must retain most entries");
            }
        }
    }

    #[test]
    fn heap_budget_and_oversized_results_do_not_flush_hot_entries() {
        let mut cache = TextCache::new(64, 1024);
        for key in 0..100 {
            cache.insert(key, key, 200);
            assert!(cache.bytes <= 1024);
        }
        assert!(cache.len() < 64);
        cache.insert(100, 100, 2048);
        assert_eq!(cache.get(&99), Some(&99));
        assert_eq!(cache.get(&100), None);
    }
}
