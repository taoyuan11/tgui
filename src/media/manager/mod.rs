use std::collections::HashMap;
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
    entries: HashMap<MediaSource, ImageCacheEntry>,
    next_access_tick: u64,
}

struct ImageCacheEntry {
    image: Arc<Mutex<ImageEntry>>,
    last_used: u64,
}

impl ImageCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            next_access_tick: 1,
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
            let tick = cache.bump_access_tick();
            if let Some(entry) = cache.entries.get_mut(source) {
                entry.last_used = tick;
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
        let entry = images
            .entries
            .entry(source.clone())
            .or_insert_with(|| ImageCacheEntry {
                image: new_entry.clone(),
                last_used: tick,
            });
        entry.last_used = tick;
        let image = entry.image.clone();
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
            cache.entries.remove(&victim);
        }
    }

    fn image_source_cache_entries(&self) -> usize {
        self.budget
            .image_raster_cache_entries
            .max(self.budget.svg_raster_cache_entries)
            .max(1)
            .saturating_mul(8)
    }

    #[cfg(test)]
    pub(crate) fn cached_image_count(&self) -> usize {
        self.images
            .lock()
            .expect("image cache lock poisoned")
            .entries
            .len()
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
) -> Option<MediaSource> {
    cache
        .entries
        .iter()
        .filter(|(source, entry)| {
            *source != protected && (include_pending || !image_entry_has_pending_work(&entry.image))
        })
        .min_by_key(|(_, entry)| entry.last_used)
        .map(|(source, _)| source.clone())
}

fn image_entry_has_pending_work(entry: &Arc<Mutex<ImageEntry>>) -> bool {
    entry
        .lock()
        .map(|entry| entry.has_pending_work())
        .unwrap_or(true)
}
