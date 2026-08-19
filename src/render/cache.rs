use std::collections::HashMap;
use std::hash::Hash;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CacheEntryKind {
    SceneChunk,
    CompiledScene,
    Pipeline,
    Vertex,
    Index,
    Instance,
    TextureBinding,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evictions: u64,
    pub bytes: u64,
    pub peak_bytes: u64,
}

struct Entry<V> {
    value: V,
    bytes: u64,
    last_used: u64,
}

/// Small deterministic LRU cache used for compiled render artifacts. Values
/// are evicted by insertion/use clock; callers retain committed/in-flight
/// references outside this cache until a submission fence completes.
pub struct RenderCache<K, V>
where
    K: Eq + Hash + Clone,
{
    entries: HashMap<K, Entry<V>>,
    limit_bytes: u64,
    clock: u64,
    stats: RenderCacheStats,
}

impl<K, V> RenderCache<K, V>
where
    K: Eq + Hash + Clone,
{
    pub fn new(limit_bytes: u64) -> Self {
        Self {
            entries: HashMap::new(),
            limit_bytes,
            clock: 0,
            stats: RenderCacheStats::default(),
        }
    }

    pub fn limit_bytes(&self) -> u64 {
        self.limit_bytes
    }
    pub fn stats(&self) -> RenderCacheStats {
        self.stats
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        let Some(entry) = self.entries.get_mut(key) else {
            self.stats.misses = self.stats.misses.saturating_add(1);
            return None;
        };
        self.clock = self.clock.saturating_add(1);
        entry.last_used = self.clock;
        self.stats.hits = self.stats.hits.saturating_add(1);
        Some(&entry.value)
    }

    pub fn insert(&mut self, key: K, value: V, bytes: u64) -> bool {
        if bytes > self.limit_bytes {
            return false;
        }
        if let Some(old) = self.entries.remove(&key) {
            self.stats.bytes = self.stats.bytes.saturating_sub(old.bytes);
        }
        while self.stats.bytes.saturating_add(bytes) > self.limit_bytes {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.stats.bytes = self.stats.bytes.saturating_sub(entry.bytes);
                self.stats.evictions = self.stats.evictions.saturating_add(1);
            }
        }
        self.clock = self.clock.saturating_add(1);
        self.entries.insert(
            key,
            Entry {
                value,
                bytes,
                last_used: self.clock,
            },
        );
        self.stats.inserts = self.stats.inserts.saturating_add(1);
        self.stats.bytes = self.stats.bytes.saturating_add(bytes);
        self.stats.peak_bytes = self.stats.peak_bytes.max(self.stats.bytes);
        true
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let entry = self.entries.remove(key)?;
        self.stats.bytes = self.stats.bytes.saturating_sub(entry.bytes);
        Some(entry.value)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.stats.bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_is_bounded_and_tracks_hits() {
        let mut cache = RenderCache::new(8);
        assert!(cache.insert(1_u32, "one", 4));
        assert!(cache.get(&1).is_some());
        assert!(cache.insert(2_u32, "two", 4));
        assert!(cache.insert(3_u32, "three", 4));
        assert_eq!(cache.len(), 2);
        assert!(cache.stats().evictions >= 1);
    }
}
