use std::sync::Arc;

use resvg::usvg;

use crate::foundation::binding::InvalidationSignal;
use crate::foundation::error::TguiError;

use super::super::svg::rasterize_svg_tree;
use super::super::types::{
    clamp_raster_request, ImageSnapshot, IntrinsicSize, RasterRequest, TextureFrame,
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
        invalidation: &InvalidationSignal,
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
            match document.texture_for(request, invalidation) {
                Ok(texture) => {
                    loading |= document.is_loading(request);
                    texture
                }
                Err(error) => {
                    self.error = Some(error.to_string());
                    invalidation.mark_dirty();
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
}

pub(in crate::media) struct DocumentEntry {
    pub(in crate::media) intrinsic_size: IntrinsicSize,
    pub(in crate::media) content: DocumentContent,
}

impl DocumentEntry {
    pub(in crate::media) fn texture_for(
        &mut self,
        raster_request: RasterRequest,
        invalidation: &InvalidationSignal,
    ) -> Result<Option<Arc<TextureFrame>>, TguiError> {
        let raster_request = clamp_raster_request(raster_request.width, raster_request.height);
        match &mut self.content {
            DocumentContent::Raster(raster) => raster.texture_for(raster_request, invalidation),
            DocumentContent::Svg(svg) => svg.texture_for(raster_request),
        }
    }

    pub(in crate::media) fn is_loading(&self, raster_request: RasterRequest) -> bool {
        match &self.content {
            DocumentContent::Raster(raster) => raster.is_loading(raster_request),
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
    next_access_tick: u64,
}

impl SvgDocument {
    pub(in crate::media) fn new(tree: usvg::Tree) -> Self {
        Self {
            tree: Arc::new(tree),
            raster_cache: Vec::new(),
            next_access_tick: 1,
        }
    }

    fn texture_for(
        &mut self,
        raster_request: RasterRequest,
    ) -> Result<Option<Arc<TextureFrame>>, TguiError> {
        let tick = self.bump_access_tick();
        if let Some(entry) = self
            .raster_cache
            .iter_mut()
            .find(|entry| entry.request == raster_request)
        {
            entry.last_used = tick;
            return Ok(Some(entry.texture.clone()));
        }

        let texture = Arc::new(rasterize_svg_tree(&self.tree, raster_request)?);
        self.raster_cache.push(SvgRasterEntry {
            request: raster_request,
            texture: texture.clone(),
            last_used: tick,
        });
        self.evict_if_needed();
        Ok(Some(texture))
    }

    fn bump_access_tick(&mut self) -> u64 {
        let tick = self.next_access_tick;
        self.next_access_tick = self.next_access_tick.saturating_add(1);
        tick
    }

    fn evict_if_needed(&mut self) {
        const MAX_SVG_RASTER_CACHE_ENTRIES: usize = 4;

        while self.raster_cache.len() > MAX_SVG_RASTER_CACHE_ENTRIES {
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
}

struct SvgRasterEntry {
    request: RasterRequest,
    texture: Arc<TextureFrame>,
    last_used: u64,
}
