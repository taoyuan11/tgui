use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use cosmic_text::SwashCache;

use crate::ui::widget::Rect;

pub(super) const RETAINED_GLYPH_IMAGE_ENTRY_LIMIT: usize = 4_096;
const RETAINED_GLYPH_IMAGE_BYTE_LIMIT: usize = 32 * 1024 * 1024;
const RETAINED_GLYPH_OUTLINE_ENTRY_LIMIT: usize = 1_024;

#[cfg(any(test, feature = "bench-support"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TextRasterCacheStats {
    pub(crate) image_entries: usize,
    pub(crate) image_bytes: usize,
    pub(crate) outline_entries: usize,
    pub(crate) image_insertions: usize,
    pub(crate) frame_resets: usize,
    pub(crate) budget_evictions: usize,
    pub(crate) font_system_resets: usize,
}

pub(super) struct TextSystem {
    pub(super) swash_cache: SwashCache,
    font_system_identity: Option<u64>,
    image_bytes: usize,
    raster_cache_grew: bool,
    #[cfg(any(test, feature = "bench-support"))]
    retain_raster_cache: bool,
    #[cfg(any(test, feature = "bench-support"))]
    image_insertions: usize,
    #[cfg(any(test, feature = "bench-support"))]
    frame_resets: usize,
    #[cfg(any(test, feature = "bench-support"))]
    budget_evictions: usize,
    #[cfg(any(test, feature = "bench-support"))]
    font_system_resets: usize,
}

impl TextSystem {
    pub(super) fn new() -> Self {
        Self {
            swash_cache: SwashCache::new(),
            font_system_identity: None,
            image_bytes: 0,
            raster_cache_grew: false,
            #[cfg(any(test, feature = "bench-support"))]
            retain_raster_cache: true,
            #[cfg(any(test, feature = "bench-support"))]
            image_insertions: 0,
            #[cfg(any(test, feature = "bench-support"))]
            frame_resets: 0,
            #[cfg(any(test, feature = "bench-support"))]
            budget_evictions: 0,
            #[cfg(any(test, feature = "bench-support"))]
            font_system_resets: 0,
        }
    }

    /// Keeps glyph rasters isolated from a different `FontSystem`.
    ///
    /// A cosmic-text cache key contains the font-database ID, glyph ID, physical font size,
    /// subpixel bins, weight, and rendering flags. Font IDs are only unique inside one database,
    /// so switching managers must invalidate even if every visible text property is unchanged.
    pub(super) fn prepare_font_system(&mut self, identity: u64) -> bool {
        if self.font_system_identity == Some(identity) {
            return false;
        }
        let changed = self.font_system_identity.is_some();
        if changed {
            self.clear_raster_cache();
            #[cfg(any(test, feature = "bench-support"))]
            {
                self.font_system_resets = self.font_system_resets.saturating_add(1);
            }
        }
        self.font_system_identity = Some(identity);
        changed
    }

    /// Records cache growth caused by one shaped text primitive. The byte budget is recomputed
    /// once at frame end, avoiding a quadratic full-cache scan across many miss batches.
    pub(super) fn finish_raster_batch(&mut self, previous_image_entries: usize) {
        let image_entries = self.swash_cache.image_cache.len();
        if image_entries == previous_image_entries {
            return;
        }

        self.raster_cache_grew = true;

        #[cfg(any(test, feature = "bench-support"))]
        {
            self.image_insertions = self
                .image_insertions
                .saturating_add(image_entries.saturating_sub(previous_image_entries));
        }
    }

    /// Ends a frame without discarding reusable glyph images in production.
    ///
    /// The benchmark/test control preserves the former whole-cache-per-frame reset so the two
    /// paths can render the same scene with identical shaping and upload behavior.
    pub(super) fn finish_frame(&mut self) {
        if !self.retention_enabled() {
            self.clear_raster_cache();
            #[cfg(any(test, feature = "bench-support"))]
            {
                self.frame_resets = self.frame_resets.saturating_add(1);
            }
            return;
        }

        if self.raster_cache_grew {
            self.image_bytes = self
                .swash_cache
                .image_cache
                .values()
                .filter_map(Option::as_ref)
                .map(|image| image.data.len())
                .sum();
            self.raster_cache_grew = false;
        }
        if self.swash_cache.image_cache.len() > RETAINED_GLYPH_IMAGE_ENTRY_LIMIT
            || self.image_bytes > RETAINED_GLYPH_IMAGE_BYTE_LIMIT
            || self.swash_cache.outline_command_cache.len() > RETAINED_GLYPH_OUTLINE_ENTRY_LIMIT
        {
            self.clear_raster_cache();
            #[cfg(any(test, feature = "bench-support"))]
            {
                self.budget_evictions = self.budget_evictions.saturating_add(1);
            }
        }
    }

    fn retention_enabled(&self) -> bool {
        #[cfg(any(test, feature = "bench-support"))]
        {
            self.retain_raster_cache
        }
        #[cfg(not(any(test, feature = "bench-support")))]
        {
            true
        }
    }

    fn clear_raster_cache(&mut self) {
        if self.swash_cache.image_cache.is_empty()
            && self.swash_cache.outline_command_cache.is_empty()
        {
            self.image_bytes = 0;
            return;
        }
        self.swash_cache = SwashCache::new();
        self.image_bytes = 0;
        self.raster_cache_grew = false;
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(super) fn set_retention(&mut self, enabled: bool) {
        self.retain_raster_cache = enabled;
        if !enabled {
            self.clear_raster_cache();
        }
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(super) fn reset_stats(&mut self) {
        self.image_insertions = 0;
        self.frame_resets = 0;
        self.budget_evictions = 0;
        self.font_system_resets = 0;
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(super) fn stats(&self) -> TextRasterCacheStats {
        TextRasterCacheStats {
            image_entries: self.swash_cache.image_cache.len(),
            image_bytes: self.image_bytes,
            outline_entries: self.swash_cache.outline_command_cache.len(),
            image_insertions: self.image_insertions,
            frame_resets: self.frame_resets,
            budget_evictions: self.budget_evictions,
            font_system_resets: self.font_system_resets,
        }
    }
}

#[derive(Clone)]
pub(super) struct OffscreenTarget {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) single_texture: wgpu::Texture,
    pub(super) single_view: wgpu::TextureView,
    pub(super) _msaa_texture: Option<wgpu::Texture>,
    pub(super) msaa_view: Option<wgpu::TextureView>,
}

pub(super) struct PresentResources {
    pub(super) bind_group: wgpu::BindGroup,
    pub(super) vertex_buffer: wgpu::Buffer,
    pub(super) vertex_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SpriteBindGroupId(pub(super) u64);

#[derive(Clone)]
pub(super) struct SpriteBindGroup {
    pub(super) id: SpriteBindGroupId,
    pub(super) bind_group: wgpu::BindGroup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MeshClipBindGroupId(pub(super) u64);

#[derive(Clone)]
pub(super) struct MeshClipBindGroup {
    pub(super) id: MeshClipBindGroupId,
    pub(super) bind_group: wgpu::BindGroup,
}

pub(super) struct TextCacheEntry {
    pub(super) draw: TextDrawBinding,
    pub(super) storage: TextTextureStorage,
}

#[derive(Clone)]
pub(super) struct TextDrawBinding {
    pub(super) binding: SpriteBindGroup,
    pub(super) uv_rect: Option<Rect>,
    /// The cached texture contains a white RGBA coverage mask whose RGB is supplied by the
    /// per-vertex tint. Color glyphs, subpixel masks, and rich text keep their baked RGBA data.
    pub(super) tintable_mask: bool,
    /// The texture stores coverage in the red channel of an `R8Unorm` atlas page.
    /// Dedicated textures and non-mask atlas entries remain RGBA.
    pub(super) r8_coverage: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TextAtlasFormat {
    R8Coverage,
    Rgba,
}

impl TextAtlasFormat {
    pub(super) const fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::R8Coverage => 1,
            Self::Rgba => 4,
        }
    }

    pub(super) const fn texture_format(self) -> wgpu::TextureFormat {
        match self {
            Self::R8Coverage => wgpu::TextureFormat::R8Unorm,
            Self::Rgba => wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TextAtlasAllocation {
    pub(super) page_id: u64,
    pub(super) format: TextAtlasFormat,
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) enum TextTextureStorage {
    Atlas(TextAtlasAllocation),
    Dedicated { _texture: wgpu::Texture },
}

pub(super) struct TextureCacheEntry {
    pub(super) revision: u64,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) binding: SpriteBindGroup,
    pub(super) texture: wgpu::Texture,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct TextCacheKey {
    pub(super) content: Arc<str>,
    pub(super) content_hash: u64,
    pub(super) rich_spans: Option<Arc<[TextRichSpanCacheKey]>>,
    pub(super) font_family: Option<Arc<str>>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) color: [u8; 4],
    pub(super) tintable_mask: bool,
    pub(super) force_color: bool,
    pub(super) font_size_bits: u32,
    pub(super) line_height_bits: u32,
    pub(super) letter_spacing_bits: u32,
    pub(super) font_weight: u16,
    pub(super) wrap_mode: u8,
    pub(super) overflow_mode: u8,
    pub(super) horizontal_align: u8,
    pub(super) vertical_align: u8,
}

impl std::hash::Hash for TextCacheKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.content_hash.hash(state);
        self.rich_spans.hash(state);
        self.font_family.hash(state);
        self.width.hash(state);
        self.height.hash(state);
        self.color.hash(state);
        self.tintable_mask.hash(state);
        self.force_color.hash(state);
        self.font_size_bits.hash(state);
        self.line_height_bits.hash(state);
        self.letter_spacing_bits.hash(state);
        self.font_weight.hash(state);
        self.wrap_mode.hash(state);
        self.overflow_mode.hash(state);
        self.horizontal_align.hash(state);
        self.vertical_align.hash(state);
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct TextRichSpanCacheKey {
    pub(super) content: Arc<str>,
    pub(super) content_hash: u64,
    pub(super) font_family: Option<Arc<str>>,
    pub(super) color: [u8; 4],
    pub(super) font_size_bits: u32,
    pub(super) font_weight: u16,
    pub(super) line_height_bits: Option<u32>,
    pub(super) letter_spacing_bits: u32,
}

impl std::hash::Hash for TextRichSpanCacheKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.content_hash.hash(state);
        self.font_family.hash(state);
        self.color.hash(state);
        self.font_size_bits.hash(state);
        self.font_weight.hash(state);
        self.line_height_bits.hash(state);
        self.letter_spacing_bits.hash(state);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct BlurUniform {
    pub(super) direction: [f32; 2],
    pub(super) texel_size: [f32; 2],
    pub(super) radius: f32,
    pub(super) _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct CompositeUniform {
    pub(super) data0: [f32; 4],
    pub(super) data1: [f32; 4],
    pub(super) data2: [f32; 4],
    pub(super) data3: [f32; 4],
    pub(super) data4: [f32; 4],
}
