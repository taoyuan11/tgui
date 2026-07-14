use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::application::ResourceBudget;
use crate::foundation::binding::InvalidationSignal;
use crate::foundation::error::TguiError;

use super::super::loader::spawn_raster_texture_loader;
use super::super::raster::{AnimatedRasterAsset, DecodedRasterAsset};
use super::super::types::{
    AnimationClock, MediaBytes, MediaCompletion, MediaSource, RasterRequest, TextureFrame,
};

pub(in crate::media) struct RasterDocument {
    source: MediaSource,
    bytes: MediaBytes,
    raster_cache: Vec<RasterTextureEntry>,
    pending_rasters: Vec<PendingRasterEntry>,
    last_exact_request: Option<(RasterRequest, usize)>,
    next_access_tick: u64,
    #[cfg(test)]
    exact_lookup_visits: usize,
    #[cfg(test)]
    exact_hot_hits: usize,
}

impl RasterDocument {
    pub(in crate::media) fn new(source: MediaSource, bytes: MediaBytes) -> Self {
        Self {
            source,
            bytes,
            raster_cache: Vec::new(),
            pending_rasters: Vec::new(),
            last_exact_request: None,
            next_access_tick: 1,
            #[cfg(test)]
            exact_lookup_visits: 0,
            #[cfg(test)]
            exact_hot_hits: 0,
        }
    }

    pub(in crate::media) fn texture_for(
        &mut self,
        raster_request: RasterRequest,
        clock: AnimationClock,
        invalidation: &InvalidationSignal,
        budget: &ResourceBudget,
        completions: &Arc<Mutex<Vec<MediaCompletion>>>,
    ) -> Result<Option<Arc<TextureFrame>>, TguiError> {
        self.collect_finished_rasters(budget)?;
        let tick = self.bump_access_tick();
        if let Some(index) = self.exact_raster_index(raster_request) {
            let entry = &mut self.raster_cache[index];
            if let Some(animation) = entry.animation.as_mut() {
                animation.ensure_started(clock.now);
            }
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
            self.source.clone(),
            raster_request,
            slot.clone(),
            invalidation.clone(),
            completions.clone(),
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

    pub(in crate::media) fn has_pending_work(&self) -> bool {
        !self.pending_rasters.is_empty()
    }

    pub(in crate::media) fn next_animation_deadline(
        &self,
        raster_request: RasterRequest,
    ) -> Option<Instant> {
        self.raster_cache
            .iter()
            .find(|entry| entry.request == raster_request)
            .and_then(|entry| entry.animation.as_ref())
            .and_then(RasterAnimationState::next_deadline)
    }

    pub(in crate::media) fn advance_animation(
        &mut self,
        raster_request: RasterRequest,
        now: Instant,
        completions: &Arc<Mutex<Vec<MediaCompletion>>>,
        invalidation: &InvalidationSignal,
    ) -> bool {
        let Some(entry) = self
            .raster_cache
            .iter_mut()
            .find(|entry| entry.request == raster_request)
        else {
            return false;
        };
        let Some(animation) = entry.animation.as_mut() else {
            return false;
        };
        if !animation.advance_to(now) {
            return false;
        }
        entry.texture = Arc::new(animation.current_frame().clone());
        completions
            .lock()
            .expect("media completion queue lock poisoned")
            .push(MediaCompletion::RasterFinished {
                key: crate::media::MediaTextureKey::new(self.source.clone(), raster_request),
            });
        invalidation.mark_media_dirty();
        true
    }

    fn collect_finished_rasters(&mut self, budget: &ResourceBudget) -> Result<(), TguiError> {
        if self.pending_rasters.is_empty() {
            return Ok(());
        }
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
                Ok(decoded) => {
                    let tick = self.bump_access_tick();
                    let (texture, animation) = match decoded {
                        DecodedRasterAsset::Still(texture) => (Arc::new(texture), None),
                        DecodedRasterAsset::Animated(animated) => {
                            let animation = RasterAnimationState::new(animated);
                            (Arc::new(animation.current_frame().clone()), Some(animation))
                        }
                    };
                    self.raster_cache.push(RasterTextureEntry {
                        request,
                        texture,
                        animation,
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

    fn exact_raster_index(&mut self, request: RasterRequest) -> Option<usize> {
        if let Some((cached_request, index)) = self.last_exact_request {
            if cached_request == request
                && self
                    .raster_cache
                    .get(index)
                    .is_some_and(|entry| entry.request == request)
            {
                #[cfg(test)]
                {
                    self.exact_hot_hits += 1;
                }
                return Some(index);
            }
        }

        let mut found = None;
        for (index, entry) in self.raster_cache.iter().enumerate() {
            #[cfg(test)]
            {
                self.exact_lookup_visits += 1;
            }
            if entry.request == request {
                found = Some(index);
                break;
            }
        }
        self.last_exact_request = found.map(|index| (request, index));
        found
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
                self.last_exact_request = None;
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

    #[cfg(test)]
    pub(in crate::media) fn reset_exact_lookup_stats(&mut self) {
        self.exact_lookup_visits = 0;
        self.exact_hot_hits = 0;
    }

    #[cfg(test)]
    pub(in crate::media) fn exact_lookup_stats(&self) -> (usize, usize) {
        (self.exact_lookup_visits, self.exact_hot_hits)
    }
}

struct RasterTextureEntry {
    request: RasterRequest,
    texture: Arc<TextureFrame>,
    animation: Option<RasterAnimationState>,
    last_used: u64,
}

struct PendingRasterEntry {
    request: RasterRequest,
    result: Arc<Mutex<Option<Result<DecodedRasterAsset, String>>>>,
}

struct RasterAnimationState {
    frames: Vec<AnimationFrame>,
    current_index: usize,
    next_deadline: Option<Instant>,
}

#[derive(Clone)]
struct AnimationFrame {
    texture: TextureFrame,
    delay: Duration,
}

impl RasterAnimationState {
    fn new(animated: AnimatedRasterAsset) -> Self {
        let frames = animated
            .frames
            .into_iter()
            .map(|frame| AnimationFrame {
                texture: frame.texture,
                delay: frame.delay,
            })
            .collect::<Vec<_>>();
        Self {
            frames,
            current_index: 0,
            next_deadline: None,
        }
    }

    fn current_frame(&self) -> &TextureFrame {
        &self.frames[self.current_index].texture
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.next_deadline
    }

    fn ensure_started(&mut self, now: Instant) {
        if self.frames.len() <= 1 || self.next_deadline.is_some() {
            return;
        }
        self.next_deadline = Some(now + self.frames[self.current_index].delay);
    }

    fn advance_to(&mut self, now: Instant) -> bool {
        if self.frames.len() <= 1 {
            self.next_deadline = None;
            return false;
        }

        self.ensure_started(now);
        let mut changed = false;
        match self.next_deadline {
            None => {}
            Some(mut deadline) => {
                while deadline <= now {
                    self.current_index = (self.current_index + 1) % self.frames.len();
                    deadline += self.frames[self.current_index].delay;
                    changed = true;
                }
                self.next_deadline = Some(deadline);
            }
        }
        changed
    }
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
