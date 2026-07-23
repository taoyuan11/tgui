use std::sync::{Arc, Mutex};

use resvg::usvg;

use crate::application::ResourceBudget;
use crate::foundation::binding::InvalidationSignal;
use crate::foundation::error::TguiError;

use super::super::svg::rasterize_svg_tree;
use super::super::types::{
    clamp_raster_request, AnimationClock, ImageSnapshot, IntrinsicSize, MediaCompletion,
    RasterRequest, TextureFrame,
};
use super::RasterDocument;

pub(in crate::media) struct ImageEntry {
    document: Option<DocumentEntry>,
    loading: bool,
    error: Option<String>,
}

impl ImageEntry {
    pub(in crate::media) fn loading() -> Self {
        Self {
            document: None,
            loading: true,
            error: None,
        }
    }

    pub(in crate::media) fn ready(document: DocumentEntry) -> Self {
        Self {
            document: Some(document),
            loading: false,
            error: None,
        }
    }

    pub(in crate::media) fn failed(error: TguiError) -> Self {
        Self {
            document: None,
            loading: false,
            error: Some(error.to_string()),
        }
    }

    pub(in crate::media) fn snapshot(
        &mut self,
        raster_request: Option<RasterRequest>,
        clock: AnimationClock,
        invalidation: &InvalidationSignal,
        budget: &ResourceBudget,
        completions: &Arc<Mutex<Vec<MediaCompletion>>>,
    ) -> ImageSnapshot {
        let intrinsic_size = self
            .document
            .as_ref()
            .map(|document| document.intrinsic_size)
            .unwrap_or(IntrinsicSize::ZERO);

        let mut loading = self.loading;
        let texture = if self.loading || self.error.is_some() {
            None
        } else if let (Some(document), Some(request)) = (self.document.as_mut(), raster_request) {
            match document.texture_for(request, clock, invalidation, budget, completions) {
                Ok(texture) => {
                    loading |= document.is_loading(request);
                    texture
                }
                Err(error) => {
                    self.error = Some(error.to_string());
                    invalidation.mark_media_dirty();
                    None
                }
            }
        } else {
            None
        };

        ImageSnapshot {
            intrinsic_size,
            texture,
            loading,
            error: self.error.clone(),
        }
    }

    pub(in crate::media) fn intrinsic_size(&self) -> IntrinsicSize {
        self.document
            .as_ref()
            .map(|document| document.intrinsic_size)
            .unwrap_or(IntrinsicSize::ZERO)
    }

    pub(in crate::media) fn has_pending_work(&self) -> bool {
        self.loading
            || self
                .document
                .as_ref()
                .map(DocumentEntry::has_pending_work)
                .unwrap_or(false)
    }

    pub(in crate::media) fn next_animation_deadline(
        &self,
        raster_request: RasterRequest,
    ) -> Option<std::time::Instant> {
        self.document
            .as_ref()
            .and_then(|document| document.next_animation_deadline(raster_request))
    }

    pub(in crate::media) fn advance_animation(
        &mut self,
        raster_request: RasterRequest,
        now: std::time::Instant,
        completions: &Arc<Mutex<Vec<MediaCompletion>>>,
        invalidation: &InvalidationSignal,
    ) -> bool {
        self.document
            .as_mut()
            .map(|document| {
                document.advance_animation(raster_request, now, completions, invalidation)
            })
            .unwrap_or(false)
    }
}

pub(in crate::media) struct DocumentEntry {
    pub(in crate::media) intrinsic_size: IntrinsicSize,
    pub(in crate::media) content: DocumentContent,
}

impl DocumentEntry {
    pub(in crate::media) fn texture_for(
        &mut self,
        raster_request: RasterRequest,
        clock: AnimationClock,
        invalidation: &InvalidationSignal,
        budget: &ResourceBudget,
        completions: &Arc<Mutex<Vec<MediaCompletion>>>,
    ) -> Result<Option<Arc<TextureFrame>>, TguiError> {
        let raster_request = clamp_raster_request(raster_request.width, raster_request.height);
        match &mut self.content {
            DocumentContent::Raster(raster) => {
                raster.texture_for(raster_request, clock, invalidation, budget, completions)
            }
            DocumentContent::Svg(svg) => svg.texture_for(raster_request, budget),
        }
    }

    pub(in crate::media) fn next_animation_deadline(
        &self,
        raster_request: RasterRequest,
    ) -> Option<std::time::Instant> {
        match &self.content {
            DocumentContent::Raster(raster) => raster.next_animation_deadline(raster_request),
            DocumentContent::Svg(_) => None,
        }
    }

    pub(in crate::media) fn advance_animation(
        &mut self,
        raster_request: RasterRequest,
        now: std::time::Instant,
        completions: &Arc<Mutex<Vec<MediaCompletion>>>,
        invalidation: &InvalidationSignal,
    ) -> bool {
        match &mut self.content {
            DocumentContent::Raster(raster) => {
                raster.advance_animation(raster_request, now, completions, invalidation)
            }
            DocumentContent::Svg(_) => false,
        }
    }

    pub(in crate::media) fn is_loading(&self, raster_request: RasterRequest) -> bool {
        match &self.content {
            DocumentContent::Raster(raster) => raster.is_loading(raster_request),
            DocumentContent::Svg(_) => false,
        }
    }

    pub(in crate::media) fn has_pending_work(&self) -> bool {
        match &self.content {
            DocumentContent::Raster(raster) => raster.has_pending_work(),
            DocumentContent::Svg(_) => false,
        }
    }
}

pub(in crate::media) enum DocumentContent {
    Raster(RasterDocument),
    Svg(SvgDocument),
}

pub(in crate::media) struct SvgDocument {
    tree: Arc<usvg::Tree>,
    raster_cache: Vec<SvgRasterEntry>,
    last_exact_request: Option<(RasterRequest, usize)>,
    next_access_tick: u64,
    #[cfg(test)]
    exact_lookup_visits: usize,
    #[cfg(test)]
    exact_hot_hits: usize,
}

impl SvgDocument {
    pub(in crate::media) fn new(tree: usvg::Tree) -> Self {
        Self {
            tree: Arc::new(tree),
            raster_cache: Vec::new(),
            last_exact_request: None,
            next_access_tick: 1,
            #[cfg(test)]
            exact_lookup_visits: 0,
            #[cfg(test)]
            exact_hot_hits: 0,
        }
    }

    #[cfg(test)]
    pub(in crate::media) fn font_database(&self) -> &Arc<usvg::fontdb::Database> {
        self.tree.fontdb()
    }

    fn texture_for(
        &mut self,
        raster_request: RasterRequest,
        budget: &ResourceBudget,
    ) -> Result<Option<Arc<TextureFrame>>, TguiError> {
        let max_entries = budget.svg_raster_cache_entries;
        let tick = self.bump_access_tick();
        if let Some(index) = self.exact_raster_index(raster_request) {
            let entry = &mut self.raster_cache[index];
            entry.last_used = tick;
            return Ok(Some(entry.texture.clone()));
        }

        let texture = Arc::new(rasterize_svg_tree(&self.tree, raster_request)?);
        if max_entries == 0 {
            return Ok(Some(texture));
        }
        self.raster_cache.push(SvgRasterEntry {
            request: raster_request,
            texture: texture.clone(),
            last_used: tick,
        });
        self.last_exact_request = Some((raster_request, self.raster_cache.len() - 1));
        self.evict_if_needed(max_entries);
        Ok(Some(texture))
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
                self.last_exact_request = None;
            } else {
                break;
            }
        }
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

struct SvgRasterEntry {
    request: RasterRequest,
    texture: Arc<TextureFrame>,
    last_used: u64,
}
