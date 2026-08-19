use crate::core::{Error, FontHandle, GlyphPageId, Rect, ResourceId, ResourceRevision, Result};
use std::collections::{BTreeSet, HashMap};

/// Font size in physical 1/64-pixel units.
///
/// Keeping this value out of logical-pixel layout makes DPI changes select a
/// different atlas without changing the shaped text representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysicalFontSize(u32);

impl PhysicalFontSize {
    pub const SUBPIXELS_PER_PIXEL: u32 = 64;

    pub fn from_pixels(pixels: f32) -> Result<Self> {
        if !pixels.is_finite() || pixels <= 0.0 {
            return Err(Error::invalid_input(
                Some("physical_font_size".to_owned()),
                "must be finite and greater than zero",
            ));
        }
        let subpixels = (pixels * Self::SUBPIXELS_PER_PIXEL as f32).round();
        if subpixels < 1.0 || subpixels > u32::MAX as f32 {
            return Err(Error::invalid_input(
                Some("physical_font_size".to_owned()),
                "is outside the supported 26.6 fixed-point range",
            ));
        }
        Ok(Self(subpixels as u32))
    }

    pub const fn from_subpixels(subpixels: u32) -> Option<Self> {
        if subpixels == 0 {
            None
        } else {
            Some(Self(subpixels))
        }
    }

    pub const fn subpixels(self) -> u32 {
        self.0
    }

    pub fn pixels(self) -> f32 {
        self.0 as f32 / Self::SUBPIXELS_PER_PIXEL as f32
    }
}

/// Stable fingerprint for raster-affecting font variation and synthesis state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlyphVariant(u64);

impl GlyphVariant {
    pub const NORMAL: Self = Self(0);

    pub const fn new(fingerprint: u64) -> Self {
        Self(fingerprint)
    }

    pub const fn fingerprint(self) -> u64 {
        self.0
    }
}

/// Pixel representation used by one atlas page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GlyphContentType {
    /// One coverage byte per pixel, tinted by the paint command.
    Mask,
    /// Premultiplied RGBA color glyph such as emoji.
    Color,
}

impl GlyphContentType {
    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::Mask => 1,
            Self::Color => 4,
        }
    }
}

/// Selects pages that can safely share texture format and raster parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlyphAtlasKey {
    pub font: FontHandle,
    pub physical_size: PhysicalFontSize,
    pub variant: GlyphVariant,
    pub content_type: GlyphContentType,
}

impl GlyphAtlasKey {
    pub const fn new(
        font: FontHandle,
        physical_size: PhysicalFontSize,
        variant: GlyphVariant,
        content_type: GlyphContentType,
    ) -> Self {
        Self {
            font,
            physical_size,
            variant,
            content_type,
        }
    }
}

/// Raster identity produced by shaping plus physical raster parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlyphKey {
    pub atlas: GlyphAtlasKey,
    pub glyph_id: u32,
}

impl GlyphKey {
    pub const fn new(atlas: GlyphAtlasKey, glyph_id: u32) -> Self {
        Self { atlas, glyph_id }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AtlasRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl AtlasRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn right(self) -> u32 {
        self.x.saturating_add(self.width)
    }

    pub const fn bottom(self) -> u32 {
        self.y.saturating_add(self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphPlacement {
    pub glyph: GlyphKey,
    pub page: GlyphPageId,
    pub pixels: AtlasRect,
    /// Normalized texture coordinates suitable for `DrawGlyphAtlas`.
    pub uv: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphRasterRequest {
    pub glyph: GlyphKey,
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphRaster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl GlyphRaster {
    pub fn new(width: u32, height: u32, pixels: impl Into<Vec<u8>>) -> Self {
        Self {
            width,
            height,
            pixels: pixels.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GlyphLookup {
    Resident(GlyphPlacement),
    Rasterize(GlyphRasterRequest),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlyphInvalidationPhases(u8);

impl GlyphInvalidationPhases {
    const RESOURCE_BIT: u8 = 1 << 0;
    const PAINT_BIT: u8 = 1 << 1;

    pub const NONE: Self = Self(0);
    pub const RESOURCE_PAINT: Self = Self(Self::RESOURCE_BIT | Self::PAINT_BIT);

    pub const fn resource(self) -> bool {
        self.0 & Self::RESOURCE_BIT != 0
    }

    pub const fn paint(self) -> bool {
        self.0 & Self::PAINT_BIT != 0
    }

    /// Glyph residency never changes logical text measurement or line layout.
    pub const fn layout(self) -> bool {
        false
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphInvalidation {
    pub runs: Vec<ResourceId>,
    pub phases: GlyphInvalidationPhases,
    pub resource_revision: ResourceRevision,
}

impl GlyphInvalidation {
    fn new(runs: BTreeSet<ResourceId>, resource_revision: ResourceRevision) -> Self {
        Self {
            phases: if runs.is_empty() {
                GlyphInvalidationPhases::NONE
            } else {
                GlyphInvalidationPhases::RESOURCE_PAINT
            },
            runs: runs.into_iter().collect(),
            resource_revision,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRasterCompletion {
    pub placement: GlyphPlacement,
    pub invalidation: GlyphInvalidation,
    /// Page generation and texel region that the renderer must upload.
    pub upload_page: GlyphPageId,
    pub upload_rect: AtlasRect,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GlyphCompletionOutcome {
    Ready(GlyphRasterCompletion),
    /// The request was superseded or its glyph became resident already.
    Stale(GlyphRasterRequest),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphAtlasConfig {
    pub page_width: u32,
    pub page_height: u32,
    pub padding: u32,
    pub max_pages: usize,
    pub max_bytes: u64,
}

impl GlyphAtlasConfig {
    pub const fn new(page_width: u32, page_height: u32, max_pages: usize) -> Self {
        Self {
            page_width,
            page_height,
            padding: 1,
            max_pages,
            max_bytes: u64::MAX,
        }
    }

    pub const fn with_padding(mut self, padding: u32) -> Self {
        self.padding = padding;
        self
    }

    pub const fn with_max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_bytes = max_bytes;
        self
    }
}

impl Default for GlyphAtlasConfig {
    fn default() -> Self {
        Self::new(1024, 1024, 8).with_max_bytes(32 * 1024 * 1024)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlyphAtlasStats {
    pub hits: u64,
    pub misses: u64,
    pub coalesced_requests: u64,
    pub raster_requests: u64,
    pub raster_completions: u64,
    pub stale_completions: u64,
    pub page_evictions: u64,
    pub glyph_evictions: u64,
    pub active_pages: usize,
    pub resident_glyphs: usize,
    pub resident_bytes: u64,
    pub peak_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphPageDescriptor {
    pub id: GlyphPageId,
    pub key: GlyphAtlasKey,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub glyphs: usize,
}

enum GlyphState {
    Pending { generation: u64 },
    Resident(GlyphPlacement),
}

struct GlyphEntry {
    state: GlyphState,
    runs: BTreeSet<ResourceId>,
}

#[derive(Clone, Debug)]
struct FreeRectAllocator {
    free: Vec<AtlasRect>,
}

impl FreeRectAllocator {
    fn new(width: u32, height: u32) -> Self {
        Self {
            free: vec![AtlasRect::new(0, 0, width, height)],
        }
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<AtlasRect> {
        let (index, chosen) = self
            .free
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, rect)| rect.width >= width && rect.height >= height)
            .min_by_key(|(_, rect)| {
                (
                    u64::from(rect.width) * u64::from(rect.height)
                        - u64::from(width) * u64::from(height),
                    rect.y,
                    rect.x,
                )
            })?;
        self.free.swap_remove(index);

        let right_width = chosen.width - width;
        if right_width > 0 {
            self.free.push(AtlasRect::new(
                chosen.x + width,
                chosen.y,
                right_width,
                height,
            ));
        }
        let bottom_height = chosen.height - height;
        if bottom_height > 0 {
            self.free.push(AtlasRect::new(
                chosen.x,
                chosen.y + height,
                chosen.width,
                bottom_height,
            ));
        }
        Some(AtlasRect::new(chosen.x, chosen.y, width, height))
    }
}

struct GlyphPage {
    key: GlyphAtlasKey,
    allocator: FreeRectAllocator,
    pixels: Vec<u8>,
    glyphs: BTreeSet<GlyphKey>,
    last_used: u64,
}

struct PageSlot {
    generation: u32,
    page: Option<GlyphPage>,
}

/// UI-thread atlas metadata and CPU page backing store.
///
/// Raster work may happen on a worker. Only `complete_raster` mutates page
/// residency, and it rejects superseded request generations before touching a
/// page. GPU executors can upload `page_pixels` or the returned upload region.
pub struct GlyphAtlas {
    config: GlyphAtlasConfig,
    pages: Vec<PageSlot>,
    glyphs: HashMap<GlyphKey, GlyphEntry>,
    request_generation: u64,
    clock: u64,
    revision: ResourceRevision,
    stats: GlyphAtlasStats,
}

impl GlyphAtlas {
    pub fn new(config: GlyphAtlasConfig) -> Result<Self> {
        if config.page_width == 0 || config.page_height == 0 {
            return Err(Error::invalid_input(
                Some("glyph_atlas.page_size".to_owned()),
                "page dimensions must be greater than zero",
            ));
        }
        if config.max_pages == 0 {
            return Err(Error::invalid_input(
                Some("glyph_atlas.max_pages".to_owned()),
                "must be greater than zero",
            ));
        }
        if config.max_bytes == 0 {
            return Err(Error::invalid_input(
                Some("glyph_atlas.max_bytes".to_owned()),
                "must be greater than zero",
            ));
        }
        let doubled_padding = config.padding.checked_mul(2).ok_or_else(|| {
            Error::invalid_input(Some("glyph_atlas.padding".to_owned()), "is too large")
        })?;
        if doubled_padding >= config.page_width || doubled_padding >= config.page_height {
            return Err(Error::invalid_input(
                Some("glyph_atlas.padding".to_owned()),
                "must leave room for glyph pixels in each page",
            ));
        }
        Ok(Self {
            config,
            pages: Vec::new(),
            glyphs: HashMap::new(),
            request_generation: 0,
            clock: 0,
            revision: ResourceRevision::ZERO,
            stats: GlyphAtlasStats::default(),
        })
    }

    pub const fn config(&self) -> GlyphAtlasConfig {
        self.config
    }

    pub const fn resource_revision(&self) -> ResourceRevision {
        self.revision
    }

    pub const fn stats(&self) -> GlyphAtlasStats {
        self.stats
    }

    /// Looks up a glyph and records exactly which logical run depends on it.
    pub fn lookup(&mut self, glyph: GlyphKey, run: ResourceId) -> Result<GlyphLookup> {
        if !glyph.atlas.font.is_well_formed() {
            return Err(Error::invalid_input(
                Some("glyph.font".to_owned()),
                "font handle generation must be non-zero",
            ));
        }
        if !run.is_well_formed() {
            return Err(Error::invalid_input(
                Some("glyph.run".to_owned()),
                "text run generation must be non-zero",
            ));
        }

        self.clock = self.clock.saturating_add(1);
        if let Some(entry) = self.glyphs.get_mut(&glyph) {
            entry.runs.insert(run);
            match entry.state {
                GlyphState::Pending { generation } => {
                    self.stats.coalesced_requests = self.stats.coalesced_requests.saturating_add(1);
                    return Ok(GlyphLookup::Rasterize(GlyphRasterRequest {
                        glyph,
                        generation,
                    }));
                }
                GlyphState::Resident(placement) => {
                    if let Some(page) = self
                        .pages
                        .get_mut(placement.page.slot() as usize)
                        .filter(|slot| slot.generation == placement.page.generation())
                        .and_then(|slot| slot.page.as_mut())
                    {
                        page.last_used = self.clock;
                        self.stats.hits = self.stats.hits.saturating_add(1);
                        return Ok(GlyphLookup::Resident(placement));
                    }
                }
            }
        }

        // A defensive cleanup also guarantees that a stale page can never be
        // returned if a renderer completion and eviction are interleaved.
        self.glyphs.remove(&glyph);
        self.request_generation = self.request_generation.checked_add(1).ok_or_else(|| {
            Error::resource(None, "glyph request generation exhausted u64", false)
        })?;
        let request = GlyphRasterRequest {
            glyph,
            generation: self.request_generation,
        };
        self.glyphs.insert(
            glyph,
            GlyphEntry {
                state: GlyphState::Pending {
                    generation: request.generation,
                },
                runs: BTreeSet::from([run]),
            },
        );
        self.stats.misses = self.stats.misses.saturating_add(1);
        self.stats.raster_requests = self.stats.raster_requests.saturating_add(1);
        Ok(GlyphLookup::Rasterize(request))
    }

    pub fn complete_raster(
        &mut self,
        request: GlyphRasterRequest,
        raster: GlyphRaster,
    ) -> Result<GlyphCompletionOutcome> {
        let current = self.glyphs.get(&request.glyph).is_some_and(|entry| {
            matches!(
                entry.state,
                GlyphState::Pending { generation } if generation == request.generation
            )
        });
        if !current {
            self.stats.stale_completions = self.stats.stale_completions.saturating_add(1);
            return Ok(GlyphCompletionOutcome::Stale(request));
        }
        self.validate_raster(request.glyph.atlas.content_type, &raster)?;

        let allocation_width = raster
            .width
            .checked_add(self.config.padding.saturating_mul(2))
            .ok_or_else(|| Error::resource(None, "glyph width and padding overflow", true))?;
        let allocation_height = raster
            .height
            .checked_add(self.config.padding.saturating_mul(2))
            .ok_or_else(|| Error::resource(None, "glyph height and padding overflow", true))?;
        if allocation_width > self.config.page_width || allocation_height > self.config.page_height
        {
            return Err(Error::resource(
                None,
                "glyph including atlas padding is larger than a page",
                true,
            ));
        }

        self.clock = self.clock.saturating_add(1);
        let mut invalidated = BTreeSet::new();
        let (page, allocated) = self.allocate(
            request.glyph.atlas,
            allocation_width,
            allocation_height,
            &mut invalidated,
        )?;
        let pixels = AtlasRect::new(
            allocated.x + self.config.padding,
            allocated.y + self.config.padding,
            raster.width,
            raster.height,
        );
        self.write_raster(
            page,
            pixels,
            request.glyph.atlas.content_type,
            &raster.pixels,
        );
        let uv = Rect::from_xywh(
            pixels.x as f32 / self.config.page_width as f32,
            pixels.y as f32 / self.config.page_height as f32,
            pixels.width as f32 / self.config.page_width as f32,
            pixels.height as f32 / self.config.page_height as f32,
        );
        let placement = GlyphPlacement {
            glyph: request.glyph,
            page,
            pixels,
            uv,
        };
        let entry = self
            .glyphs
            .get_mut(&request.glyph)
            .expect("pending entry was validated before allocation");
        invalidated.extend(entry.runs.iter().copied());
        entry.state = GlyphState::Resident(placement);
        let page_data = self.pages[page.slot() as usize]
            .page
            .as_mut()
            .expect("newly allocated page remains resident");
        page_data.glyphs.insert(request.glyph);
        page_data.last_used = self.clock;

        self.revision.advance().map_err(|error| {
            Error::resource(
                None,
                format!("failed to advance resource revision: {error}"),
                false,
            )
        })?;
        self.stats.raster_completions = self.stats.raster_completions.saturating_add(1);
        self.refresh_stats();
        Ok(GlyphCompletionOutcome::Ready(GlyphRasterCompletion {
            placement,
            invalidation: GlyphInvalidation::new(invalidated, self.revision),
            upload_page: page,
            upload_rect: pixels,
        }))
    }

    pub fn placement(&self, glyph: &GlyphKey) -> Option<GlyphPlacement> {
        let GlyphState::Resident(placement) = self.glyphs.get(glyph)?.state else {
            return None;
        };
        self.is_placement_current(placement).then_some(placement)
    }

    pub fn is_placement_current(&self, placement: GlyphPlacement) -> bool {
        self.is_page_current(placement.page)
            && self.glyphs.get(&placement.glyph).is_some_and(|entry| {
                matches!(entry.state, GlyphState::Resident(current) if current == placement)
            })
    }

    pub fn is_page_current(&self, page: GlyphPageId) -> bool {
        self.pages
            .get(page.slot() as usize)
            .is_some_and(|slot| slot.generation == page.generation() && slot.page.is_some())
    }

    pub fn page_descriptor(&self, page: GlyphPageId) -> Option<GlyphPageDescriptor> {
        let slot = self.pages.get(page.slot() as usize)?;
        if slot.generation != page.generation() {
            return None;
        }
        let data = slot.page.as_ref()?;
        Some(GlyphPageDescriptor {
            id: page,
            key: data.key,
            width: self.config.page_width,
            height: self.config.page_height,
            bytes: self.page_bytes(data.key.content_type),
            glyphs: data.glyphs.len(),
        })
    }

    pub fn page_pixels(&self, page: GlyphPageId) -> Option<&[u8]> {
        let slot = self.pages.get(page.slot() as usize)?;
        (slot.generation == page.generation()).then_some(slot.page.as_ref()?.pixels.as_slice())
    }

    /// Evicts the least recently used page and returns only runs that used it.
    pub fn evict_lru_page(&mut self) -> Result<Option<GlyphInvalidation>> {
        let Some(slot) = self.oldest_page_slot() else {
            return Ok(None);
        };
        let runs = self.evict_slot(slot);
        self.revision.advance().map_err(|error| {
            Error::resource(
                None,
                format!("failed to advance resource revision: {error}"),
                false,
            )
        })?;
        self.refresh_stats();
        Ok(Some(GlyphInvalidation::new(runs, self.revision)))
    }

    /// Stops reporting a no-longer-live run from future raster completions or evictions.
    pub fn detach_run(&mut self, run: ResourceId) {
        for entry in self.glyphs.values_mut() {
            entry.runs.remove(&run);
        }
    }

    fn validate_raster(&self, content_type: GlyphContentType, raster: &GlyphRaster) -> Result<()> {
        if raster.width == 0 || raster.height == 0 {
            return Err(Error::invalid_input(
                Some("glyph_raster.size".to_owned()),
                "dimensions must be greater than zero",
            ));
        }
        let expected = u64::from(raster.width)
            .checked_mul(u64::from(raster.height))
            .and_then(|pixels| pixels.checked_mul(u64::from(content_type.bytes_per_pixel())))
            .ok_or_else(|| Error::resource(None, "glyph raster byte length overflow", true))?;
        if usize::try_from(expected).ok() != Some(raster.pixels.len()) {
            return Err(Error::invalid_input(
                Some("glyph_raster.pixels".to_owned()),
                format!("expected {expected} bytes for {content_type:?}"),
            ));
        }
        Ok(())
    }

    fn allocate(
        &mut self,
        key: GlyphAtlasKey,
        width: u32,
        height: u32,
        invalidated: &mut BTreeSet<ResourceId>,
    ) -> Result<(GlyphPageId, AtlasRect)> {
        for (slot_index, slot) in self.pages.iter_mut().enumerate() {
            let Some(page) = slot.page.as_mut().filter(|page| page.key == key) else {
                continue;
            };
            if let Some(rect) = page.allocator.allocate(width, height) {
                let id = GlyphPageId::from_parts(slot_index as u32, slot.generation);
                return Ok((id, rect));
            }
        }

        let page_bytes = self.page_bytes(key.content_type);
        if page_bytes > self.config.max_bytes {
            return Err(Error::resource(
                None,
                "one glyph atlas page exceeds the configured byte budget",
                true,
            ));
        }
        while self.stats.active_pages >= self.config.max_pages
            || self.stats.resident_bytes.saturating_add(page_bytes) > self.config.max_bytes
        {
            let Some(slot) = self.oldest_page_slot() else {
                return Err(Error::resource(
                    None,
                    "glyph atlas budget cannot admit another page",
                    true,
                ));
            };
            invalidated.extend(self.evict_slot(slot));
            self.refresh_stats();
        }

        let page = self.create_page(key)?;
        let rect = self.pages[page.slot() as usize]
            .page
            .as_mut()
            .and_then(|data| data.allocator.allocate(width, height))
            .expect("validated glyph fits an empty page");
        Ok((page, rect))
    }

    fn create_page(&mut self, key: GlyphAtlasKey) -> Result<GlyphPageId> {
        let byte_len = usize::try_from(self.page_bytes(key.content_type)).map_err(|_| {
            Error::resource(None, "glyph atlas page does not fit address space", true)
        })?;
        let page = GlyphPage {
            key,
            allocator: FreeRectAllocator::new(self.config.page_width, self.config.page_height),
            pixels: vec![0; byte_len],
            glyphs: BTreeSet::new(),
            last_used: self.clock,
        };
        if let Some(slot_index) = self
            .pages
            .iter()
            .position(|slot| slot.page.is_none() && slot.generation < u32::MAX)
        {
            let generation = self.pages[slot_index].generation + 1;
            self.pages[slot_index].generation = generation;
            self.pages[slot_index].page = Some(page);
            self.refresh_stats();
            return Ok(GlyphPageId::from_parts(slot_index as u32, generation));
        }
        let slot_index = u32::try_from(self.pages.len())
            .map_err(|_| Error::resource(None, "glyph page slots exhausted u32", false))?;
        self.pages.push(PageSlot {
            generation: 1,
            page: Some(page),
        });
        self.refresh_stats();
        Ok(GlyphPageId::from_parts(slot_index, 1))
    }

    fn oldest_page_slot(&self) -> Option<usize> {
        self.pages
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| slot.page.as_ref().map(|page| (index, page.last_used)))
            .min_by_key(|(index, last_used)| (*last_used, *index))
            .map(|(index, _)| index)
    }

    fn evict_slot(&mut self, slot_index: usize) -> BTreeSet<ResourceId> {
        let Some(page) = self.pages[slot_index].page.take() else {
            return BTreeSet::new();
        };
        let mut runs = BTreeSet::new();
        for glyph in page.glyphs {
            if let Some(entry) = self.glyphs.remove(&glyph) {
                runs.extend(entry.runs);
                self.stats.glyph_evictions = self.stats.glyph_evictions.saturating_add(1);
            }
        }
        self.stats.page_evictions = self.stats.page_evictions.saturating_add(1);
        runs
    }

    fn write_raster(
        &mut self,
        page: GlyphPageId,
        rect: AtlasRect,
        content_type: GlyphContentType,
        source: &[u8],
    ) {
        let bytes_per_pixel = content_type.bytes_per_pixel() as usize;
        let page_width = self.config.page_width as usize;
        let row_bytes = rect.width as usize * bytes_per_pixel;
        let target = &mut self.pages[page.slot() as usize]
            .page
            .as_mut()
            .expect("allocation page remains resident")
            .pixels;
        for row in 0..rect.height as usize {
            let source_start = row * row_bytes;
            let target_start =
                ((rect.y as usize + row) * page_width + rect.x as usize) * bytes_per_pixel;
            target[target_start..target_start + row_bytes]
                .copy_from_slice(&source[source_start..source_start + row_bytes]);
        }
    }

    fn page_bytes(&self, content_type: GlyphContentType) -> u64 {
        u64::from(self.config.page_width)
            .saturating_mul(u64::from(self.config.page_height))
            .saturating_mul(u64::from(content_type.bytes_per_pixel()))
    }

    fn refresh_stats(&mut self) {
        self.stats.active_pages = 0;
        self.stats.resident_bytes = 0;
        for page in self.pages.iter().filter_map(|slot| slot.page.as_ref()) {
            self.stats.active_pages += 1;
            self.stats.resident_bytes = self
                .stats
                .resident_bytes
                .saturating_add(self.page_bytes(page.key.content_type));
        }
        self.stats.resident_glyphs = self
            .glyphs
            .values()
            .filter(|entry| matches!(entry.state, GlyphState::Resident(_)))
            .count();
        self.stats.peak_bytes = self.stats.peak_bytes.max(self.stats.resident_bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atlas_key(content_type: GlyphContentType) -> GlyphAtlasKey {
        GlyphAtlasKey::new(
            FontHandle::from_parts(1, 1),
            PhysicalFontSize::from_pixels(16.0).unwrap(),
            GlyphVariant::NORMAL,
            content_type,
        )
    }

    fn request(lookup: GlyphLookup) -> GlyphRasterRequest {
        let GlyphLookup::Rasterize(request) = lookup else {
            panic!("expected raster request")
        };
        request
    }

    fn ready(outcome: GlyphCompletionOutcome) -> GlyphRasterCompletion {
        let GlyphCompletionOutcome::Ready(completion) = outcome else {
            panic!("expected ready completion")
        };
        completion
    }

    #[test]
    fn free_rect_allocator_never_overlaps_allocations() {
        let mut allocator = FreeRectAllocator::new(8, 8);
        let a = allocator.allocate(5, 3).unwrap();
        let b = allocator.allocate(3, 5).unwrap();
        let c = allocator.allocate(3, 3).unwrap();
        let d = allocator.allocate(5, 5).unwrap();
        assert_eq!(a, AtlasRect::new(0, 0, 5, 3));
        assert_eq!(b, AtlasRect::new(0, 3, 3, 5));
        assert_eq!(c, AtlasRect::new(5, 0, 3, 3));
        assert_eq!(d, AtlasRect::new(3, 3, 5, 5));
        assert!(allocator.allocate(1, 1).is_none());
    }

    #[test]
    fn raster_bytes_are_written_to_the_selected_page_format() {
        let mut atlas = GlyphAtlas::new(
            GlyphAtlasConfig::new(8, 8, 2)
                .with_padding(0)
                .with_max_bytes(512),
        )
        .unwrap();
        let run = ResourceId::from_parts(2, 1);
        let mask = GlyphKey::new(atlas_key(GlyphContentType::Mask), 10);
        let mask_request = request(atlas.lookup(mask, run).unwrap());
        let mask_ready = ready(
            atlas
                .complete_raster(mask_request, GlyphRaster::new(2, 1, vec![7, 9]))
                .unwrap(),
        );
        assert_eq!(
            &atlas.page_pixels(mask_ready.placement.page).unwrap()[0..2],
            &[7, 9]
        );

        let color = GlyphKey::new(atlas_key(GlyphContentType::Color), 11);
        let color_request = request(atlas.lookup(color, run).unwrap());
        let color_ready = ready(
            atlas
                .complete_raster(color_request, GlyphRaster::new(1, 1, vec![1, 2, 3, 4]))
                .unwrap(),
        );
        assert_ne!(mask_ready.placement.page, color_ready.placement.page);
        assert_eq!(
            &atlas.page_pixels(color_ready.placement.page).unwrap()[0..4],
            &[1, 2, 3, 4]
        );
    }
}
