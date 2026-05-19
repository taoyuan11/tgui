use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::application::ResourceBudget;
use crate::foundation::binding::InvalidationSignal;
use crate::foundation::error::TguiError;

use super::loader::{load_image_entry, spawn_image_loader};
use super::types::{ImageSnapshot, MediaSource, RasterRequest, TextureFrame};

mod image;
mod raster;
mod shadow;

pub(in crate::media) use image::{DocumentContent, DocumentEntry, ImageEntry, SvgDocument};
pub(in crate::media) use raster::RasterDocument;
use shadow::{canvas_shadow_texture, widget_shadow_texture, CanvasShadowEntry, WidgetShadowEntry};

pub(crate) struct MediaManager {
    invalidation: InvalidationSignal,
    budget: ResourceBudget,
    images: Mutex<HashMap<MediaSource, Arc<Mutex<ImageEntry>>>>,
    canvas_shadows: Mutex<Vec<CanvasShadowEntry>>,
    widget_shadows: Mutex<Vec<WidgetShadowEntry>>,
}

impl MediaManager {
    pub(crate) fn with_budget(invalidation: InvalidationSignal, budget: ResourceBudget) -> Self {
        Self {
            invalidation,
            budget,
            images: Mutex::new(HashMap::new()),
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
        let snapshot = entry.lock().expect("image entry lock poisoned").snapshot(
            raster_request,
            &self.invalidation,
            &self.budget,
        );
        snapshot
    }

    fn image_entry(&self, source: &MediaSource) -> Arc<Mutex<ImageEntry>> {
        if let Some(entry) = self
            .images
            .lock()
            .expect("image cache lock poisoned")
            .get(source)
            .cloned()
        {
            return entry;
        }

        let new_entry = match source {
            MediaSource::Bytes(_) => Arc::new(Mutex::new(load_image_entry(source))),
            _ => {
                let entry = Arc::new(Mutex::new(ImageEntry::loading()));
                spawn_image_loader(entry.clone(), source.clone(), self.invalidation.clone());
                entry
            }
        };

        let mut images = self.images.lock().expect("image cache lock poisoned");
        images
            .entry(source.clone())
            .or_insert_with(|| new_entry.clone())
            .clone()
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
