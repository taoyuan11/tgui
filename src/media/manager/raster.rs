use std::sync::{Arc, Mutex};

use crate::application::ResourceBudget;
use crate::foundation::binding::InvalidationSignal;
use crate::foundation::error::TguiError;

use super::super::loader::spawn_raster_texture_loader;
use super::super::types::{MediaBytes, RasterRequest, TextureFrame};

pub(in crate::media) struct RasterDocument {
    bytes: MediaBytes,
    raster_cache: Vec<RasterTextureEntry>,
    pending_rasters: Vec<PendingRasterEntry>,
    next_access_tick: u64,
}

impl RasterDocument {
    pub(in crate::media) fn new(bytes: MediaBytes) -> Self {
        Self {
            bytes,
            raster_cache: Vec::new(),
            pending_rasters: Vec::new(),
            next_access_tick: 1,
        }
    }

    pub(in crate::media) fn texture_for(
        &mut self,
        raster_request: RasterRequest,
        invalidation: &InvalidationSignal,
        budget: &ResourceBudget,
    ) -> Result<Option<Arc<TextureFrame>>, TguiError> {
        self.collect_finished_rasters(budget)?;

        let tick = self.bump_access_tick();
        if let Some(entry) = self
            .raster_cache
            .iter_mut()
            .find(|entry| entry.request == raster_request)
        {
            entry.last_used = tick;
            return Ok(Some(entry.texture.clone()));
        }

        if self
            .pending_rasters
            .iter()
            .any(|entry| entry.request == raster_request)
        {
            return Ok(self.best_cached_texture(raster_request, tick));
        }

        let slot = Arc::new(Mutex::new(None));
        spawn_raster_texture_loader(
            self.bytes.clone(),
            raster_request,
            slot.clone(),
            invalidation.clone(),
        );
        self.pending_rasters.push(PendingRasterEntry {
            request: raster_request,
            result: slot,
        });
        Ok(self.best_cached_texture(raster_request, tick))
    }

    pub(in crate::media) fn is_loading(&self, raster_request: RasterRequest) -> bool {
        self.pending_rasters
            .iter()
            .any(|entry| entry.request == raster_request)
    }

    fn collect_finished_rasters(&mut self, budget: &ResourceBudget) -> Result<(), TguiError> {
        let mut completed = Vec::new();
        for (index, entry) in self.pending_rasters.iter().enumerate() {
            let Some(result) = entry
                .result
                .lock()
                .expect("pending raster lock poisoned")
                .take()
            else {
                continue;
            };
            completed.push((index, entry.request, result));
        }

        for (index, request, result) in completed.into_iter().rev() {
            self.pending_rasters.remove(index);
            match result {
                Ok(texture) => {
                    let tick = self.bump_access_tick();
                    self.raster_cache.push(RasterTextureEntry {
                        request,
                        texture,
                        last_used: tick,
                    });
                    self.evict_if_needed(budget.image_raster_cache_entries);
                }
                Err(error) => return Err(TguiError::Media(error)),
            }
        }

        Ok(())
    }

    fn bump_access_tick(&mut self) -> u64 {
        let tick = self.next_access_tick;
        self.next_access_tick = self.next_access_tick.saturating_add(1);
        tick
    }

    fn evict_if_needed(&mut self, max_entries: usize) {
        while self.raster_cache.len() > max_entries {
            if let Some((oldest_index, _)) = self
                .raster_cache
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| entry.last_used)
            {
                self.raster_cache.remove(oldest_index);
            } else {
                break;
            }
        }
    }

    fn best_cached_texture(
        &mut self,
        raster_request: RasterRequest,
        tick: u64,
    ) -> Option<Arc<TextureFrame>> {
        let best_index = self
            .raster_cache
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                raster_request_distance(left.request, raster_request)
                    .total_cmp(&raster_request_distance(right.request, raster_request))
                    .then_with(|| {
                        let left_area = left.request.width as u64 * left.request.height as u64;
                        let right_area = right.request.width as u64 * right.request.height as u64;
                        right_area.cmp(&left_area)
                    })
            })
            .map(|(index, _)| index)?;

        let entry = &mut self.raster_cache[best_index];
        entry.last_used = tick;
        Some(entry.texture.clone())
    }
}

struct RasterTextureEntry {
    request: RasterRequest,
    texture: Arc<TextureFrame>,
    last_used: u64,
}

struct PendingRasterEntry {
    request: RasterRequest,
    result: Arc<Mutex<Option<Result<Arc<TextureFrame>, String>>>>,
}

fn raster_request_distance(left: RasterRequest, right: RasterRequest) -> f32 {
    let left_area = (left.width.max(1) as f32) * (left.height.max(1) as f32);
    let right_area = (right.width.max(1) as f32) * (right.height.max(1) as f32);
    let area_ratio = (left_area / right_area).max(right_area / left_area);

    let left_aspect = left.width.max(1) as f32 / left.height.max(1) as f32;
    let right_aspect = right.width.max(1) as f32 / right.height.max(1) as f32;
    let aspect_ratio = (left_aspect / right_aspect).max(right_aspect / left_aspect);

    area_ratio + aspect_ratio
}
