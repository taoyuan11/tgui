use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::application::ResourceBudget;
use crate::foundation::binding::InvalidationSignal;
use crate::foundation::error::TguiError;

use super::loader::{load_image_entry, spawn_image_loader};
use super::types::{
    AnimationClock, ImageSnapshot, MediaCompletion, MediaSource, MediaTextureKey, RasterRequest,
    TextureFrame,
};

mod image;
mod raster;
mod shadow;

pub(in crate::media) use image::{DocumentContent, DocumentEntry, ImageEntry, SvgDocument};
pub(in crate::media) use raster::RasterDocument;
use shadow::{canvas_shadow_texture, widget_shadow_texture, CanvasShadowEntry, WidgetShadowEntry};

pub(crate) struct MediaManager {
    invalidation: InvalidationSignal,
    budget: ResourceBudget,
    images: Mutex<ImageCache>,
    completions: Arc<Mutex<Vec<MediaCompletion>>>,
    canvas_shadows: Mutex<Vec<CanvasShadowEntry>>,
    widget_shadows: Mutex<Vec<WidgetShadowEntry>>,
}

struct ImageCache {
    entries: HashMap<Arc<MediaSource>, Arc<ImageCacheEntry>>,
    hot_entry: Option<HotImageCacheEntry>,
    next_access_tick: u64,
    #[cfg(test)]
    source_hash_lookups: usize,
    #[cfg(test)]
    hot_hits: usize,
    #[cfg(test)]
    entry_requests: usize,
}

struct ImageCacheEntry {
    image: Arc<Mutex<ImageEntry>>,
    last_used: AtomicU64,
}

struct HotImageCacheEntry {
    source: Arc<MediaSource>,
    entry: Arc<ImageCacheEntry>,
}

impl ImageCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            hot_entry: None,
            next_access_tick: 1,
            #[cfg(test)]
            source_hash_lookups: 0,
            #[cfg(test)]
            hot_hits: 0,
            #[cfg(test)]
            entry_requests: 0,
        }
    }

    fn bump_access_tick(&mut self) -> u64 {
        let tick = self.next_access_tick;
        self.next_access_tick = self.next_access_tick.saturating_add(1);
        tick
    }
}

impl MediaManager {
    pub(crate) fn with_budget(invalidation: InvalidationSignal, budget: ResourceBudget) -> Self {
        Self {
            invalidation,
            budget,
            images: Mutex::new(ImageCache::new()),
            completions: Arc::new(Mutex::new(Vec::new())),
            canvas_shadows: Mutex::new(Vec::new()),
            widget_shadows: Mutex::new(Vec::new()),
        }
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(crate) fn new(invalidation: InvalidationSignal) -> Self {
        Self::with_budget(invalidation, ResourceBudget::DEFAULT)
    }

    pub(crate) fn image_snapshot(
        &self,
        source: &MediaSource,
        raster_request: Option<RasterRequest>,
    ) -> ImageSnapshot {
        let entry = self.image_entry(source);
        let clock = AnimationClock {
            now: Instant::now(),
        };
        let snapshot = entry.lock().expect("image entry lock poisoned").snapshot(
            raster_request,
            clock,
            &self.invalidation,
            &self.budget,
            &self.completions,
        );
        snapshot
    }

    /// Resolves intrinsic sizing and the requested raster in one cache/entry lookup.
    ///
    /// Image rendering used to request metadata and then immediately request the raster, which
    /// repeated the source hash and both media mutex acquisitions. Holding the entry across the
    /// sizing decision also avoids cloning the intermediate error state.
    pub(crate) fn image_snapshot_for_layout(
        &self,
        source: &MediaSource,
        layout: super::types::MediaTextureLayout,
    ) -> (
        ImageSnapshot,
        crate::ui::widget::Rect,
        Option<RasterRequest>,
    ) {
        let entry = self.image_entry(source);
        let clock = AnimationClock {
            now: Instant::now(),
        };
        let mut entry = entry.lock().expect("image entry lock poisoned");
        let target_frame = layout.target_frame(entry.intrinsic_size());
        let raster_request = RasterRequest::from_frame(target_frame, layout.scale_factor);
        let snapshot = entry.snapshot(
            raster_request,
            clock,
            &self.invalidation,
            &self.budget,
            &self.completions,
        );
        (snapshot, target_frame, raster_request)
    }

    pub(crate) fn drain_completions(&self) -> Vec<MediaCompletion> {
        self.completions
            .lock()
            .expect("media completion queue lock poisoned")
            .drain(..)
            .collect()
    }

    pub(crate) fn next_animation_deadline_for_keys<'a>(
        &self,
        keys: impl IntoIterator<Item = &'a MediaTextureKey>,
    ) -> Option<Instant> {
        let mut next: Option<Instant> = None;
        for key in keys {
            let Some(deadline) = self.image_animation_deadline(&key.source, key.raster_request)
            else {
                continue;
            };
            next = Some(match next {
                Some(current) => current.min(deadline),
                None => deadline,
            });
        }
        next
    }

    pub(crate) fn advance_animations_for_keys<'a>(
        &self,
        keys: impl IntoIterator<Item = &'a MediaTextureKey>,
        now: Instant,
    ) -> bool {
        let mut advanced = false;
        for key in keys {
            if self.advance_image_animation(&key.source, key.raster_request, now) {
                advanced = true;
            }
        }
        advanced
    }

    fn image_entry(&self, source: &MediaSource) -> Arc<Mutex<ImageEntry>> {
        {
            let mut cache = self.images.lock().expect("image cache lock poisoned");
            #[cfg(test)]
            {
                cache.entry_requests += 1;
            }
            let tick = cache.bump_access_tick();
            if let Some(entry) = cache
                .hot_entry
                .as_ref()
                .filter(|entry| entry.source.as_ref() == source)
                .map(|entry| Arc::clone(&entry.entry))
            {
                entry.last_used.store(tick, Ordering::Relaxed);
                #[cfg(test)]
                {
                    cache.hot_hits += 1;
                }
                return entry.image.clone();
            }
            #[cfg(test)]
            {
                cache.source_hash_lookups += 1;
            }
            if let Some((source_key, entry)) = cache
                .entries
                .get_key_value(source)
                .map(|(source, entry)| (Arc::clone(source), Arc::clone(entry)))
            {
                entry.last_used.store(tick, Ordering::Relaxed);
                cache.hot_entry = Some(HotImageCacheEntry {
                    source: source_key,
                    entry: Arc::clone(&entry),
                });
                return entry.image.clone();
            }
        }

        let new_entry = match source {
            MediaSource::Bytes(_) => Arc::new(Mutex::new(load_image_entry(source))),
            _ => {
                let entry = Arc::new(Mutex::new(ImageEntry::loading()));
                spawn_image_loader(
                    entry.clone(),
                    source.clone(),
                    self.invalidation.clone(),
                    self.completions.clone(),
                );
                entry
            }
        };

        let mut images = self.images.lock().expect("image cache lock poisoned");
        let tick = images.bump_access_tick();
        #[cfg(test)]
        {
            images.source_hash_lookups += 1;
        }
        let source_key = Arc::new(source.clone());
        let entry = images
            .entries
            .entry(Arc::clone(&source_key))
            .or_insert_with(|| {
                Arc::new(ImageCacheEntry {
                    image: new_entry.clone(),
                    last_used: AtomicU64::new(tick),
                })
            })
            .clone();
        entry.last_used.store(tick, Ordering::Relaxed);
        let image = entry.image.clone();
        images.hot_entry = Some(HotImageCacheEntry {
            source: source_key,
            entry,
        });
        self.evict_image_sources_if_needed(&mut images, source);
        image
    }

    fn image_animation_deadline(
        &self,
        source: &MediaSource,
        raster_request: RasterRequest,
    ) -> Option<Instant> {
        let entry = self.image_entry(source);
        entry
            .lock()
            .ok()
            .and_then(|entry| entry.next_animation_deadline(raster_request))
    }

    fn advance_image_animation(
        &self,
        source: &MediaSource,
        raster_request: RasterRequest,
        now: Instant,
    ) -> bool {
        let entry = self.image_entry(source);
        entry
            .lock()
            .map(|mut entry| {
                entry.advance_animation(raster_request, now, &self.completions, &self.invalidation)
            })
            .unwrap_or(false)
    }

    fn evict_image_sources_if_needed(&self, cache: &mut ImageCache, protected: &MediaSource) {
        let max_entries = self.image_source_cache_entries();
        while cache.entries.len() > max_entries {
            let Some(victim) = oldest_evictable_image_source(cache, protected, false)
                .or_else(|| oldest_evictable_image_source(cache, protected, true))
            else {
                break;
            };
            cache.entries.remove(victim.as_ref());
            if cache
                .hot_entry
                .as_ref()
                .is_some_and(|entry| entry.source == victim)
            {
                cache.hot_entry = None;
            }
        }
    }

    fn image_source_cache_entries(&self) -> usize {
        self.budget
            .image_raster_cache_entries
            .max(self.budget.svg_raster_cache_entries)
            .max(1)
            .saturating_mul(8)
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(crate) fn cached_image_count(&self) -> usize {
        self.images
            .lock()
            .expect("image cache lock poisoned")
            .entries
            .len()
    }

    #[cfg(test)]
    pub(crate) fn reset_image_lookup_stats(&self) {
        let mut cache = self.images.lock().expect("image cache lock poisoned");
        cache.source_hash_lookups = 0;
        cache.hot_hits = 0;
        cache.entry_requests = 0;
    }

    #[cfg(test)]
    pub(crate) fn image_lookup_stats(&self) -> (usize, usize, usize) {
        let cache = self.images.lock().expect("image cache lock poisoned");
        (
            cache.source_hash_lookups,
            cache.hot_hits,
            cache.entry_requests,
        )
    }

    #[cfg(test)]
    pub(crate) fn is_image_cached(&self, source: &MediaSource) -> bool {
        self.images
            .lock()
            .expect("image cache lock poisoned")
            .entries
            .contains_key(source)
    }

    pub(crate) fn canvas_shadow_texture<F>(
        &self,
        cache_key: u64,
        width: u32,
        height: u32,
        render: F,
    ) -> Result<Option<Arc<TextureFrame>>, TguiError>
    where
        F: FnOnce() -> Result<TextureFrame, TguiError>,
    {
        canvas_shadow_texture(
            &self.canvas_shadows,
            self.budget.canvas_shadow_cache_entries,
            cache_key,
            width,
            height,
            render,
        )
    }

    pub(crate) fn widget_shadow_texture<F>(
        &self,
        cache_key: u64,
        width: u32,
        height: u32,
        render: F,
    ) -> Result<Option<Arc<TextureFrame>>, TguiError>
    where
        F: FnOnce() -> Result<TextureFrame, TguiError>,
    {
        widget_shadow_texture(
            &self.widget_shadows,
            self.budget.widget_shadow_cache_entries,
            cache_key,
            width,
            height,
            render,
        )
    }
}

fn oldest_evictable_image_source(
    cache: &ImageCache,
    protected: &MediaSource,
    include_pending: bool,
) -> Option<Arc<MediaSource>> {
    cache
        .entries
        .iter()
        .filter(|(source, entry)| {
            source.as_ref() != protected
                && (include_pending || !image_entry_has_pending_work(&entry.image))
        })
        .min_by_key(|(_, entry)| entry.last_used.load(Ordering::Relaxed))
        .map(|(source, _)| Arc::clone(source))
}

fn image_entry_has_pending_work(entry: &Arc<Mutex<ImageEntry>>) -> bool {
    entry
        .lock()
        .map(|entry| entry.has_pending_work())
        .unwrap_or(true)
}
