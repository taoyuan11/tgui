use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use cosmic_text::{Attrs, Buffer, Color, Family, Metrics, Shaping, SwashContent, Weight};
use unicode_segmentation::UnicodeSegmentation;

use crate::foundation::color::Color as TguiColor;
use crate::foundation::error::TguiError;
use crate::text::font::{FontManager, FontWeight};
use crate::ui::widget::{Rect, TextPrimitive};

use super::{
    Renderer, SpriteBindGroup, TextAtlasAllocation, TextAtlasFormat, TextCacheEntry, TextCacheKey,
    TextDrawBinding, TextRichSpanCacheKey, TextTextureStorage,
};

impl Renderer {
    pub(super) fn text_cache_key(&self, text: &TextPrimitive) -> Option<TextCacheKey> {
        if text.rich_spans.is_none() {
            let mask_key = self.text_cache_key_for_mode(text, true)?;
            if self.text_cache.contains_key(&mask_key) {
                return Some(mask_key);
            }
        }
        self.text_cache_key_for_mode(text, false)
    }

    fn text_cache_key_for_mode(
        &self,
        text: &TextPrimitive,
        tintable_mask: bool,
    ) -> Option<TextCacheKey> {
        let frame = self.snap_text_rect(text.frame);
        let width = self.logical_to_physical(frame.width.get()).round().max(1.0) as u32;
        let height = self
            .logical_to_physical(frame.height.get())
            .round()
            .max(1.0) as u32;
        if width == 0 || height == 0 || text.content.is_empty() {
            return None;
        }

        let font_size = self.logical_to_physical(text.font_size);
        let line_height = self.logical_to_physical(text.line_height);
        let letter_spacing = self.logical_to_physical(text.letter_spacing);

        Some(TextCacheKey {
            content: Arc::clone(&text.content),
            content_hash: text_content_hash(&text.content),
            rich_spans: text.rich_spans.as_deref().map(rich_span_cache_keys),
            font_family: text.font_family.clone(),
            width,
            height,
            color: if tintable_mask {
                [255; 4]
            } else {
                self.text_raster_cache_color(text.color)
            },
            tintable_mask,
            force_color: text.force_color,
            font_size_bits: font_size.to_bits(),
            line_height_bits: line_height.to_bits(),
            letter_spacing_bits: letter_spacing.to_bits(),
            font_weight: text.font_weight.to_raw(),
            wrap_mode: wrap_mode_key(text),
            overflow_mode: overflow_mode_key(text),
            horizontal_align: horizontal_align_key(text),
            vertical_align: vertical_align_key(text),
        })
    }

    pub(super) fn text_bind_group_for(
        &mut self,
        text: &TextPrimitive,
        font_manager: &FontManager,
    ) -> Result<Option<TextDrawBinding>, TguiError> {
        let Some(key) = self.text_cache_key(text) else {
            return Ok(None);
        };

        if let Some(entry) = self.text_cache.get(&key) {
            #[cfg(any(test, feature = "bench-support"))]
            {
                self.text_cache_hits = self.text_cache_hits.saturating_add(1);
            }
            return Ok(Some(entry.draw.clone()));
        }

        #[cfg(any(test, feature = "bench-support"))]
        {
            self.text_cache_misses = self.text_cache_misses.saturating_add(1);
        }

        let entry = match self.rasterize_text(text, font_manager)? {
            Some(entry) => entry,
            None => return Ok(None),
        };
        let draw = entry.draw.clone();
        let key = self
            .text_cache_key_for_mode(text, draw.tintable_mask)
            .expect("a successfully rasterized text primitive must retain a cache key");
        self.text_cache.insert(key, entry);

        Ok(Some(draw))
    }

    pub(super) fn rasterize_text(
        &mut self,
        text: &TextPrimitive,
        font_manager: &FontManager,
    ) -> Result<Option<TextCacheEntry>, TguiError> {
        let frame = self.snap_text_rect(text.frame);
        let width = self.logical_to_physical(frame.width.get()).round().max(1.0) as u32;
        let height = self
            .logical_to_physical(frame.height.get())
            .round()
            .max(1.0) as u32;
        if width == 0 || height == 0 || text.content.is_empty() {
            return Ok(None);
        }
        let font_size = self.logical_to_physical(text.font_size);
        let line_height = self.logical_to_physical(text.line_height);
        let letter_spacing = self.logical_to_physical(text.letter_spacing);

        let mut pixels = std::mem::take(&mut self.text_upload_scratch);
        let r8_atlas_candidate =
            self.text_r8_atlas_enabled() && self.text_atlas.page_size_for(width, height).is_some();

        let (upload_bounds, tintable_mask, r8_coverage, mut raster_bytes_per_row) = font_manager
            .with_font_system(|font_system| -> Result<_, TguiError> {
                let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
                buffer.set_size(Some(width as f32), Some(height as f32));
                buffer.set_wrap(text_wrap(text));
                let attrs = attrs_for_text(text, font_size, letter_spacing);
                if let Some(rich_spans) = text.rich_spans.as_ref() {
                    buffer.set_rich_text(
                        rich_spans.iter().map(|span| {
                            (
                                span.content.as_ref(),
                                attrs_for_span(span, font_size, line_height, letter_spacing),
                            )
                        }),
                        &attrs,
                        Shaping::Advanced,
                        None,
                    );
                } else {
                    let content = overflow_content(text, &mut buffer, font_system, &attrs);
                    buffer.set_text(&content, &attrs, Shaping::Advanced, None);
                }
                buffer.shape_until_scroll(font_system, false);

                let tintable_mask = self.text_mask_tint_enabled()
                    && text.rich_spans.is_none()
                    && buffer_uses_only_mask_glyphs(
                        &buffer,
                        font_system,
                        &mut self.text_system.swash_cache,
                    );
                let r8_coverage = tintable_mask && r8_atlas_candidate;
                let raster_bytes_per_row = if r8_coverage {
                    aligned_bytes_per_row(width, 1)
                } else {
                    aligned_rgba_bytes_per_row(width)
                }
                .ok_or_else(|| {
                    TguiError::TextRender(format!(
                        "text raster row is too wide for an upload: {width}px"
                    ))
                })?;
                let raster_len = usize::try_from(raster_bytes_per_row)
                    .ok()
                    .and_then(|stride| stride.checked_mul(height as usize))
                    .ok_or_else(|| {
                        TguiError::TextRender(format!(
                            "text raster dimensions overflow addressable memory: {width}x{height}"
                        ))
                    })?;
                prepare_text_upload_scratch(&mut pixels, raster_len);

                let (offset_x, offset_y) = text_offsets(text, &buffer, width as f32, height as f32);

                let mut upload_bounds = RgbaUploadBounds::default();
                let raster_color = if tintable_mask {
                    TguiColor::WHITE
                } else {
                    TguiColor {
                        a: 255,
                        ..text.color
                    }
                };
                let requested_rgba = raster_color.to_rgba8();
                #[cfg(any(test, feature = "bench-support"))]
                let blend_fast_path_enabled = self.text_blend_fast_path_enabled;
                #[cfg(any(test, feature = "bench-support"))]
                let blend_stats = &mut self.text_blend_stats;
                buffer.draw(
                    font_system,
                    &mut self.text_system.swash_cache,
                    color_to_text(raster_color),
                    |x, y, w, h, color| {
                        let x = x + offset_x;
                        let y = y + offset_y;
                        let mut rgba = color.as_rgba();
                        if text.force_color {
                            rgba[0] = requested_rgba[0];
                            rgba[1] = requested_rgba[1];
                            rgba[2] = requested_rgba[2];
                            rgba[3] = ((rgba[3] as u16 * requested_rgba[3] as u16) / 255) as u8;
                        }
                        if rgba[3] != 0 {
                            upload_bounds.include_clipped_rect(x, y, w, h, width, height);
                        }
                        let start_x = x.max(0);
                        let start_y = y.max(0);
                        let end_x = x.saturating_add(w as i32).min(width as i32);
                        let end_y = y.saturating_add(h as i32).min(height as i32);
                        if start_x >= end_x || start_y >= end_y {
                            return;
                        }
                        let src = [rgba[0], rgba[1], rgba[2], rgba[3]];
                        if src[3] == 0 {
                            #[cfg(any(test, feature = "bench-support"))]
                            if blend_fast_path_enabled {
                                blend_stats.transparent_source_pixels +=
                                    (end_x - start_x) as usize * (end_y - start_y) as usize;
                                return;
                            }
                            #[cfg(not(any(test, feature = "bench-support")))]
                            return;
                        }
                        for py in start_y as u32..end_y as u32 {
                            for px in start_x as u32..end_x as u32 {
                                if r8_coverage {
                                    #[cfg(any(test, feature = "bench-support"))]
                                    blend_coverage_controlled(
                                        &mut pixels,
                                        raster_bytes_per_row,
                                        px,
                                        py,
                                        src[3],
                                        blend_fast_path_enabled,
                                        blend_stats,
                                    );
                                    #[cfg(not(any(test, feature = "bench-support")))]
                                    blend_coverage(
                                        &mut pixels,
                                        raster_bytes_per_row,
                                        px,
                                        py,
                                        src[3],
                                    );
                                } else {
                                    #[cfg(any(test, feature = "bench-support"))]
                                    blend_pixel_controlled(
                                        &mut pixels,
                                        raster_bytes_per_row,
                                        px,
                                        py,
                                        src,
                                        blend_fast_path_enabled,
                                        blend_stats,
                                    );
                                    #[cfg(not(any(test, feature = "bench-support")))]
                                    blend_pixel(&mut pixels, raster_bytes_per_row, px, py, src);
                                }
                            }
                        }
                    },
                );
                Ok((
                    upload_bounds,
                    tintable_mask,
                    r8_coverage,
                    raster_bytes_per_row,
                ))
            })?;

        if !upload_bounds.has_ink {
            self.text_upload_scratch = pixels;
            return Ok(None);
        }

        let atlas_format = if r8_coverage {
            TextAtlasFormat::R8Coverage
        } else {
            TextAtlasFormat::Rgba
        };
        if let Some(mut reservation) = self.reserve_text_atlas(width, height, atlas_format) {
            reservation.draw.tintable_mask = tintable_mask;
            reservation.draw.r8_coverage = atlas_format == TextAtlasFormat::R8Coverage;
            let upload_plan = match atlas_format {
                TextAtlasFormat::R8Coverage => {
                    pad_r8_for_atlas_in_place(&mut pixels, raster_bytes_per_row, width, height)
                }
                TextAtlasFormat::Rgba => {
                    pad_rgba_for_atlas_in_place(&mut pixels, raster_bytes_per_row, width, height)
                }
            };
            let Some(upload_plan) = upload_plan else {
                self.text_atlas.release(reservation.allocation);
                self.text_upload_scratch = pixels;
                return Err(TguiError::TextRender(format!(
                    "text atlas upload dimensions overflow addressable memory: {width}x{height}"
                )));
            };
            let staged = self.text_atlas_deferred_upload_enabled()
                && self.text_atlas.stage_upload(
                    reservation.allocation,
                    &pixels[..upload_plan.data_len],
                    upload_plan,
                );
            if !staged {
                let page = self
                    .text_atlas
                    .page(reservation.allocation.page_id)
                    .expect("reserved text atlas page must remain alive until upload");
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &page._texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: reservation.allocation.x,
                            y: reservation.allocation.y,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &pixels[..upload_plan.data_len],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(upload_plan.bytes_per_row),
                        rows_per_image: Some(upload_plan.height),
                    },
                    wgpu::Extent3d {
                        width: upload_plan.width,
                        height: upload_plan.height,
                        depth_or_array_layers: 1,
                    },
                );
                #[cfg(any(test, feature = "bench-support"))]
                self.text_atlas
                    .record_immediate_upload(upload_plan.data_len, atlas_format);
            }
            self.text_upload_scratch = pixels;
            return Ok(Some(TextCacheEntry {
                draw: reservation.draw,
                storage: TextTextureStorage::Atlas(reservation.allocation),
            }));
        }

        // A fresh page should always fit after `page_size_for` admitted the R8
        // raster. Keep a real RGBA dedicated-texture fallback nevertheless, so
        // an allocator regression can never reinterpret one-byte coverage rows
        // as four-byte pixels.
        if r8_coverage {
            let Some(rgba_stride) =
                expand_r8_mask_to_rgba_in_place(&mut pixels, raster_bytes_per_row, width, height)
            else {
                self.text_upload_scratch = pixels;
                return Err(TguiError::TextRender(format!(
                    "text R8 fallback dimensions overflow addressable memory: {width}x{height}"
                )));
            };
            raster_bytes_per_row = rgba_stride;
        }

        let Some(upload_plan) = pack_rgba_upload_in_place(
            &mut pixels,
            raster_bytes_per_row,
            width,
            height,
            upload_bounds,
        ) else {
            self.text_upload_scratch = pixels;
            return Ok(None);
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-text-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: upload_plan.origin_x,
                    y: upload_plan.origin_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &pixels[..upload_plan.data_len],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(upload_plan.bytes_per_row),
                rows_per_image: Some(upload_plan.height),
            },
            wgpu::Extent3d {
                width: upload_plan.width,
                height: upload_plan.height,
                depth_or_array_layers: 1,
            },
        );

        let binding = self.create_text_sprite_binding(&texture, "tgui-text-bind-group");

        self.text_upload_scratch = pixels;

        Ok(Some(TextCacheEntry {
            draw: TextDrawBinding {
                binding,
                uv_rect: None,
                tintable_mask,
                r8_coverage: false,
            },
            storage: TextTextureStorage::Dedicated { _texture: texture },
        }))
    }

    fn reserve_text_atlas(
        &mut self,
        width: u32,
        height: u32,
        format: TextAtlasFormat,
    ) -> Option<TextDrawAllocation> {
        let (page_width, page_height) = self.text_atlas.page_size_for(width, height)?;
        if let Some(allocation) = self.text_atlas.allocate_existing(width, height, format) {
            return Some(allocation);
        }

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(match format {
                TextAtlasFormat::R8Coverage => "tgui-text-r8-atlas-page",
                TextAtlasFormat::Rgba => "tgui-text-rgba-atlas-page",
            }),
            size: wgpu::Extent3d {
                width: page_width,
                height: page_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format.texture_format(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let binding = self.create_text_sprite_binding(&texture, "tgui-text-atlas-bind-group");
        self.text_atlas.add_page(texture, binding, format);
        self.text_atlas.allocate_existing(width, height, format)
    }

    fn create_text_sprite_binding(
        &mut self,
        texture: &wgpu::Texture,
        label: &'static str,
    ) -> SpriteBindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &self.text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
            ],
        });
        SpriteBindGroup {
            id: self.allocate_sprite_bind_group_id(),
            bind_group,
        }
    }

    pub(super) fn snap_text_rect(&self, rect: Rect) -> Rect {
        let x = self.logical_to_physical(rect.x.get()).round();
        let y = self.logical_to_physical(rect.y.get()).round();
        let width = self.logical_to_physical(rect.width.get()).ceil().max(1.0);
        let height = self.logical_to_physical(rect.height.get()).ceil().max(1.0);
        Rect::new(
            x / self.scale_factor,
            y / self.scale_factor,
            width / self.scale_factor,
            height / self.scale_factor,
        )
    }

    fn text_raster_cache_color(&self, color: TguiColor) -> [u8; 4] {
        #[cfg(any(test, feature = "bench-support"))]
        if !self.text_alpha_cache_normalization_enabled {
            return color.to_rgba8();
        }
        text_raster_cache_color(color)
    }

    #[inline]
    fn text_atlas_deferred_upload_enabled(&self) -> bool {
        #[cfg(any(test, feature = "bench-support"))]
        {
            self.text_atlas_deferred_upload_enabled
        }
        #[cfg(not(any(test, feature = "bench-support")))]
        {
            true
        }
    }

    #[inline]
    fn text_mask_tint_enabled(&self) -> bool {
        #[cfg(any(test, feature = "bench-support"))]
        {
            self.text_mask_tint_enabled
        }
        #[cfg(not(any(test, feature = "bench-support")))]
        {
            true
        }
    }

    #[inline]
    fn text_r8_atlas_enabled(&self) -> bool {
        #[cfg(any(test, feature = "bench-support"))]
        {
            self.text_r8_atlas_enabled
        }
        #[cfg(not(any(test, feature = "bench-support")))]
        {
            true
        }
    }
}

fn buffer_uses_only_mask_glyphs(
    buffer: &Buffer,
    font_system: &mut cosmic_text::FontSystem,
    swash_cache: &mut cosmic_text::SwashCache,
) -> bool {
    buffer.layout_runs().all(|run| {
        run.glyphs.iter().all(|glyph| {
            let physical = glyph.physical((0.0, run.line_y), 1.0);
            match swash_cache.get_image(font_system, physical.cache_key) {
                None => true,
                Some(image) => image.content == SwashContent::Mask,
            }
        })
    })
}

const RGBA_BYTES_PER_PIXEL: u32 = 4;
const TEXT_ATLAS_PADDING: u32 = 1;
const TEXT_ATLAS_PAGE_WIDTH: u32 = 2048;
const TEXT_ATLAS_PAGE_HEIGHT: u32 = 256;
/// Bounds the extra CPU residency used by page shadows. Pages beyond the budget
/// remain valid atlas pages and fall back to the legacy immediate upload path.
const TEXT_ATLAS_SHADOW_BUDGET_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct AtlasRect {
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
}

impl AtlasRect {
    fn area(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    fn right(self) -> u32 {
        self.x + self.width
    }

    fn bottom(self) -> u32 {
        self.y + self.height
    }

    fn contains(self, other: Self) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.right() >= other.right()
            && self.bottom() >= other.bottom()
    }
}

/// Non-overlapping guillotine allocator used by each text-atlas page.
///
/// Unlike a skyline allocator, released rectangles can be merged and reused. The
/// free list always remains a disjoint partition, which makes cache eviction safe
/// to reason about and keeps allocation O(page fragments) with very small pages.
#[derive(Debug)]
pub(super) struct TextAtlasAllocator {
    width: u32,
    height: u32,
    free: Vec<AtlasRect>,
    live_allocations: usize,
}

impl TextAtlasAllocator {
    pub(super) fn new(width: u32, height: u32) -> Self {
        let free = (width > 0 && height > 0)
            .then_some(AtlasRect {
                x: 0,
                y: 0,
                width,
                height,
            })
            .into_iter()
            .collect();
        Self {
            width,
            height,
            free,
            live_allocations: 0,
        }
    }

    pub(super) fn allocate(&mut self, width: u32, height: u32) -> Option<AtlasRect> {
        if width == 0 || height == 0 || width > self.width || height > self.height {
            return None;
        }

        let (index, free_rect) = self
            .free
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, rect)| rect.width >= width && rect.height >= height)
            .min_by_key(|(_, rect)| {
                (
                    rect.area() - u64::from(width) * u64::from(height),
                    (rect.width - width).min(rect.height - height),
                )
            })?;
        self.free.swap_remove(index);

        let allocation = AtlasRect {
            x: free_rect.x,
            y: free_rect.y,
            width,
            height,
        };
        let remaining_width = free_rect.width - width;
        let remaining_height = free_rect.height - height;

        // Split along the larger remainder so the second rectangle keeps the
        // longer uninterrupted edge. The two emitted rectangles never overlap.
        if remaining_width > remaining_height {
            self.push_free(AtlasRect {
                x: free_rect.x + width,
                y: free_rect.y,
                width: remaining_width,
                height: free_rect.height,
            });
            self.push_free(AtlasRect {
                x: free_rect.x,
                y: free_rect.y + height,
                width,
                height: remaining_height,
            });
        } else {
            self.push_free(AtlasRect {
                x: free_rect.x + width,
                y: free_rect.y,
                width: remaining_width,
                height,
            });
            self.push_free(AtlasRect {
                x: free_rect.x,
                y: free_rect.y + height,
                width: free_rect.width,
                height: remaining_height,
            });
        }

        self.live_allocations += 1;
        Some(allocation)
    }

    pub(super) fn release(&mut self, allocation: AtlasRect) {
        debug_assert!(allocation.width > 0 && allocation.height > 0);
        debug_assert!(allocation.right() <= self.width);
        debug_assert!(allocation.bottom() <= self.height);
        debug_assert!(self.live_allocations > 0);
        self.live_allocations = self.live_allocations.saturating_sub(1);
        if self.live_allocations == 0 {
            self.free.clear();
            self.push_free(AtlasRect {
                x: 0,
                y: 0,
                width: self.width,
                height: self.height,
            });
            return;
        }
        self.free.push(allocation);
        self.coalesce_free_rects();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.live_allocations == 0
    }

    fn push_free(&mut self, rect: AtlasRect) {
        if rect.width > 0 && rect.height > 0 {
            self.free.push(rect);
        }
    }

    fn coalesce_free_rects(&mut self) {
        loop {
            let mut merged = false;
            'outer: for first in 0..self.free.len() {
                for second in first + 1..self.free.len() {
                    let a = self.free[first];
                    let b = self.free[second];
                    let combined = if a.y == b.y
                        && a.height == b.height
                        && (a.right() == b.x || b.right() == a.x)
                    {
                        Some(AtlasRect {
                            x: a.x.min(b.x),
                            y: a.y,
                            width: a.width + b.width,
                            height: a.height,
                        })
                    } else if a.x == b.x
                        && a.width == b.width
                        && (a.bottom() == b.y || b.bottom() == a.y)
                    {
                        Some(AtlasRect {
                            x: a.x,
                            y: a.y.min(b.y),
                            width: a.width,
                            height: a.height + b.height,
                        })
                    } else {
                        None
                    };

                    if let Some(combined) = combined {
                        self.free.swap_remove(second);
                        self.free[first] = combined;
                        merged = true;
                        break 'outer;
                    }
                }
            }
            if !merged {
                break;
            }
        }

        // Exact-edge coalescing normally removes all redundancy. Keep this
        // defensive prune so corrupted/doubly released rectangles cannot turn
        // into allocator growth in release builds.
        let mut index = 0;
        while index < self.free.len() {
            let contained = self
                .free
                .iter()
                .enumerate()
                .any(|(other, rect)| other != index && rect.contains(self.free[index]));
            if contained {
                self.free.swap_remove(index);
            } else {
                index += 1;
            }
        }
    }
}

pub(super) struct TextAtlasPage {
    id: u64,
    format: TextAtlasFormat,
    _texture: wgpu::Texture,
    binding: SpriteBindGroup,
    allocator: TextAtlasAllocator,
    /// CPU mirror of the complete page. Reused slots overwrite their full padded
    /// rectangle, so stale glyph pixels can never leak through the one-pixel gutter.
    shadow: Vec<u8>,
    /// Horizontal dirty span for each page row, stored as an exclusive x range.
    dirty_rows: Vec<Option<(u32, u32)>>,
}

pub(super) struct TextAtlas {
    page_width: u32,
    page_height: u32,
    next_page_id: u64,
    pages: Vec<TextAtlasPage>,
    upload_scratch: Vec<u8>,
    shadow_budget_bytes: usize,
    #[cfg(any(test, feature = "bench-support"))]
    upload_stats: TextAtlasUploadStats,
}

#[cfg(any(test, feature = "bench-support"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TextAtlasUploadStats {
    pub(super) write_calls: usize,
    pub(super) uploaded_bytes: usize,
    pub(super) r8_uploaded_bytes: usize,
    pub(super) rgba_uploaded_bytes: usize,
    pub(super) shadow_bytes: usize,
    pub(super) r8_shadow_bytes: usize,
    pub(super) rgba_shadow_bytes: usize,
    pub(super) shadow_budget_bytes: usize,
}

impl TextAtlas {
    pub(super) fn new(max_texture_dimension_2d: u32) -> Self {
        let (page_width, page_height) =
            text_atlas_page_size(max_texture_dimension_2d).unwrap_or((0, 0));
        Self {
            page_width,
            page_height,
            next_page_id: 1,
            pages: Vec::new(),
            upload_scratch: Vec::new(),
            shadow_budget_bytes: TEXT_ATLAS_SHADOW_BUDGET_BYTES,
            #[cfg(any(test, feature = "bench-support"))]
            upload_stats: TextAtlasUploadStats {
                shadow_budget_bytes: TEXT_ATLAS_SHADOW_BUDGET_BYTES,
                ..TextAtlasUploadStats::default()
            },
        }
    }

    #[cfg(all(test, feature = "bench-support"))]
    fn with_shadow_budget(max_texture_dimension_2d: u32, shadow_budget_bytes: usize) -> Self {
        let mut atlas = Self::new(max_texture_dimension_2d);
        atlas.shadow_budget_bytes = shadow_budget_bytes;
        atlas.upload_stats.shadow_budget_bytes = shadow_budget_bytes;
        atlas
    }

    fn page_size_for(&self, width: u32, height: u32) -> Option<(u32, u32)> {
        let (padded_width, padded_height) = padded_atlas_extent(width, height)?;
        (padded_width <= self.page_width && padded_height <= self.page_height)
            .then_some((self.page_width, self.page_height))
    }

    fn allocate_existing(
        &mut self,
        width: u32,
        height: u32,
        format: TextAtlasFormat,
    ) -> Option<TextDrawAllocation> {
        let (padded_width, padded_height) = padded_atlas_extent(width, height)?;
        for page in self.pages.iter_mut().filter(|page| page.format == format) {
            let Some(rect) = page.allocator.allocate(padded_width, padded_height) else {
                continue;
            };
            return Some(TextDrawAllocation {
                draw: TextDrawBinding {
                    binding: page.binding.clone(),
                    uv_rect: Some(atlas_content_uv(rect, self.page_width, self.page_height)),
                    tintable_mask: format == TextAtlasFormat::R8Coverage,
                    r8_coverage: format == TextAtlasFormat::R8Coverage,
                },
                allocation: TextAtlasAllocation {
                    page_id: page.id,
                    format,
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                },
            });
        }
        None
    }

    fn add_page(
        &mut self,
        texture: wgpu::Texture,
        binding: SpriteBindGroup,
        format: TextAtlasFormat,
    ) {
        let id = self.next_page_id;
        self.next_page_id = self
            .next_page_id
            .checked_add(1)
            .expect("text atlas page id overflow");
        let page_shadow_bytes = self.page_width as usize
            * self.page_height as usize
            * format.bytes_per_pixel() as usize;
        let shadow_bytes = self
            .pages
            .iter()
            .map(|page| page.shadow.len())
            .sum::<usize>();
        let has_shadow =
            shadow_fits_budget(shadow_bytes, page_shadow_bytes, self.shadow_budget_bytes);
        self.pages.push(TextAtlasPage {
            id,
            format,
            _texture: texture,
            binding,
            allocator: TextAtlasAllocator::new(self.page_width, self.page_height),
            shadow: if has_shadow {
                vec![0; page_shadow_bytes]
            } else {
                Vec::new()
            },
            dirty_rows: if has_shadow {
                vec![None; self.page_height as usize]
            } else {
                Vec::new()
            },
        });
        #[cfg(any(test, feature = "bench-support"))]
        self.refresh_shadow_stats();
    }

    fn page(&self, page_id: u64) -> Option<&TextAtlasPage> {
        self.pages.iter().find(|page| page.id == page_id)
    }

    fn stage_upload(
        &mut self,
        allocation: TextAtlasAllocation,
        pixels: &[u8],
        upload: RgbaUploadPlan,
    ) -> bool {
        debug_assert_eq!(upload.origin_x, 0);
        debug_assert_eq!(upload.origin_y, 0);
        debug_assert_eq!(upload.width, allocation.width);
        debug_assert_eq!(upload.height, allocation.height);
        let Some(page) = self
            .pages
            .iter_mut()
            .find(|page| page.id == allocation.page_id)
        else {
            return false;
        };
        debug_assert_eq!(page.format, allocation.format);
        if page.format != allocation.format {
            return false;
        }
        if page.shadow.is_empty() {
            return false;
        }
        let bytes_per_pixel = page.format.bytes_per_pixel() as usize;
        let copied_row_bytes = upload.width as usize * bytes_per_pixel;
        let source_stride = upload.bytes_per_row as usize;
        let page_stride = self.page_width as usize * bytes_per_pixel;
        let destination_x = allocation.x as usize * bytes_per_pixel;
        for row in 0..upload.height as usize {
            let source_start = row * source_stride;
            let destination_start = (allocation.y as usize + row) * page_stride + destination_x;
            page.shadow[destination_start..destination_start + copied_row_bytes]
                .copy_from_slice(&pixels[source_start..source_start + copied_row_bytes]);
            let dirty = &mut page.dirty_rows[allocation.y as usize + row];
            *dirty = Some(match *dirty {
                Some((left, right)) => (
                    left.min(allocation.x),
                    right.max(allocation.x + upload.width),
                ),
                None => (allocation.x, allocation.x + upload.width),
            });
        }
        true
    }

    pub(super) fn flush_pending_uploads(&mut self, queue: &wgpu::Queue) {
        let page_width = self.page_width;
        for page in &mut self.pages {
            let bytes_per_pixel = page.format.bytes_per_pixel();
            let dirty_rects = dirty_upload_rects(&page.dirty_rows, bytes_per_pixel);
            for rect in dirty_rects {
                let Some(bytes_per_row) = aligned_bytes_per_row(rect.width, bytes_per_pixel) else {
                    continue;
                };
                let copied_row_bytes = rect.width as usize * bytes_per_pixel as usize;
                let upload_stride = bytes_per_row as usize;
                let required_len = upload_stride * rect.height as usize;
                self.upload_scratch.resize(required_len, 0);
                let page_stride = page_width as usize * bytes_per_pixel as usize;
                let source_x = rect.x as usize * bytes_per_pixel as usize;
                for row in 0..rect.height as usize {
                    let source_start = (rect.y as usize + row) * page_stride + source_x;
                    let destination_start = row * upload_stride;
                    self.upload_scratch[destination_start..destination_start + copied_row_bytes]
                        .copy_from_slice(
                            &page.shadow[source_start..source_start + copied_row_bytes],
                        );
                }
                let data_len = (rect.height as usize - 1) * upload_stride + copied_row_bytes;
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &page._texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: rect.x,
                            y: rect.y,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &self.upload_scratch[..data_len],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(rect.height),
                    },
                    wgpu::Extent3d {
                        width: rect.width,
                        height: rect.height,
                        depth_or_array_layers: 1,
                    },
                );
                #[cfg(any(test, feature = "bench-support"))]
                {
                    self.upload_stats.write_calls = self.upload_stats.write_calls.saturating_add(1);
                    self.upload_stats.uploaded_bytes =
                        self.upload_stats.uploaded_bytes.saturating_add(data_len);
                    match page.format {
                        TextAtlasFormat::R8Coverage => {
                            self.upload_stats.r8_uploaded_bytes =
                                self.upload_stats.r8_uploaded_bytes.saturating_add(data_len);
                        }
                        TextAtlasFormat::Rgba => {
                            self.upload_stats.rgba_uploaded_bytes = self
                                .upload_stats
                                .rgba_uploaded_bytes
                                .saturating_add(data_len);
                        }
                    }
                }
            }
            page.dirty_rows.fill(None);
        }
        #[cfg(any(test, feature = "bench-support"))]
        {
            self.refresh_shadow_stats();
        }
    }

    #[cfg(any(test, feature = "bench-support"))]
    fn record_immediate_upload(&mut self, data_len: usize, format: TextAtlasFormat) {
        self.upload_stats.write_calls = self.upload_stats.write_calls.saturating_add(1);
        self.upload_stats.uploaded_bytes =
            self.upload_stats.uploaded_bytes.saturating_add(data_len);
        match format {
            TextAtlasFormat::R8Coverage => {
                self.upload_stats.r8_uploaded_bytes =
                    self.upload_stats.r8_uploaded_bytes.saturating_add(data_len);
            }
            TextAtlasFormat::Rgba => {
                self.upload_stats.rgba_uploaded_bytes = self
                    .upload_stats
                    .rgba_uploaded_bytes
                    .saturating_add(data_len);
            }
        }
        self.refresh_shadow_stats();
    }

    #[cfg(any(test, feature = "bench-support"))]
    #[allow(dead_code)]
    pub(super) fn upload_stats(&self) -> TextAtlasUploadStats {
        self.upload_stats
    }

    #[cfg(any(test, feature = "bench-support"))]
    #[allow(dead_code)]
    pub(super) fn reset_upload_stats(&mut self) {
        self.upload_stats = TextAtlasUploadStats {
            shadow_bytes: self.pages.iter().map(|page| page.shadow.len()).sum(),
            r8_shadow_bytes: self
                .pages
                .iter()
                .filter(|page| page.format == TextAtlasFormat::R8Coverage)
                .map(|page| page.shadow.len())
                .sum(),
            rgba_shadow_bytes: self
                .pages
                .iter()
                .filter(|page| page.format == TextAtlasFormat::Rgba)
                .map(|page| page.shadow.len())
                .sum(),
            shadow_budget_bytes: self.shadow_budget_bytes,
            ..TextAtlasUploadStats::default()
        };
    }

    pub(super) fn release(&mut self, allocation: TextAtlasAllocation) {
        let Some(index) = self
            .pages
            .iter()
            .position(|page| page.id == allocation.page_id)
        else {
            debug_assert!(
                false,
                "released text atlas allocation references a missing page"
            );
            return;
        };
        let page = &mut self.pages[index];
        debug_assert_eq!(page.format, allocation.format);
        page.allocator.release(AtlasRect {
            x: allocation.x,
            y: allocation.y,
            width: allocation.width,
            height: allocation.height,
        });
        if page.allocator.is_empty() {
            self.pages.swap_remove(index);
        }
        #[cfg(any(test, feature = "bench-support"))]
        {
            self.refresh_shadow_stats();
        }
    }

    #[cfg(feature = "bench-support")]
    pub(super) fn page_count(&self) -> usize {
        self.pages.len()
    }

    #[cfg(feature = "bench-support")]
    pub(super) fn page_counts_by_format(&self) -> (usize, usize) {
        let r8 = self
            .pages
            .iter()
            .filter(|page| page.format == TextAtlasFormat::R8Coverage)
            .count();
        (r8, self.pages.len() - r8)
    }

    #[cfg(any(test, feature = "bench-support"))]
    fn refresh_shadow_stats(&mut self) {
        self.upload_stats.r8_shadow_bytes = self
            .pages
            .iter()
            .filter(|page| page.format == TextAtlasFormat::R8Coverage)
            .map(|page| page.shadow.len())
            .sum();
        self.upload_stats.rgba_shadow_bytes = self
            .pages
            .iter()
            .filter(|page| page.format == TextAtlasFormat::Rgba)
            .map(|page| page.shadow.len())
            .sum();
        self.upload_stats.shadow_bytes = self
            .upload_stats
            .r8_shadow_bytes
            .saturating_add(self.upload_stats.rgba_shadow_bytes);
    }
}

const MAX_DIRTY_UPLOAD_RECTS_PER_PAGE: usize = 8;
const DIRTY_UPLOAD_CALL_COST_BYTES: u64 = 64 * 1024;

fn shadow_fits_budget(current_bytes: usize, page_bytes: usize, budget_bytes: usize) -> bool {
    current_bytes
        .checked_add(page_bytes)
        .is_some_and(|bytes| bytes <= budget_bytes)
}

fn dirty_upload_rects(rows: &[Option<(u32, u32)>], bytes_per_pixel: u32) -> Vec<AtlasRect> {
    let mut rects = Vec::new();
    let mut row = 0;
    while row < rows.len() {
        let Some((left, right)) = rows[row] else {
            row += 1;
            continue;
        };
        let start = row;
        row += 1;
        while row < rows.len() && rows[row] == Some((left, right)) {
            row += 1;
        }
        rects.push(AtlasRect {
            x: left,
            y: start as u32,
            width: right - left,
            height: (row - start) as u32,
        });
    }

    // Adjacent row bands are merged when one fewer Queue::write_texture call is
    // worth the extra copied pixels. A hard cap keeps pathological fragmentation
    // bounded while retaining small uploads for sparse cache misses.
    loop {
        let mut best = None;
        for index in 0..rects.len().saturating_sub(1) {
            let merged = bounding_rect(rects[index], rects[index + 1]);
            let extra = upload_rect_bytes(merged, bytes_per_pixel)
                .saturating_sub(upload_rect_bytes(rects[index], bytes_per_pixel))
                .saturating_sub(upload_rect_bytes(rects[index + 1], bytes_per_pixel));
            if best.map_or(true, |(_, best_extra)| extra < best_extra) {
                best = Some((index, extra));
            }
        }
        let Some((index, extra)) = best else {
            break;
        };
        if rects.len() <= MAX_DIRTY_UPLOAD_RECTS_PER_PAGE && extra > DIRTY_UPLOAD_CALL_COST_BYTES {
            break;
        }
        let merged = bounding_rect(rects[index], rects[index + 1]);
        rects[index] = merged;
        rects.remove(index + 1);
    }
    rects
}

fn bounding_rect(left: AtlasRect, right: AtlasRect) -> AtlasRect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let end_x = left.right().max(right.right());
    let end_y = left.bottom().max(right.bottom());
    AtlasRect {
        x,
        y,
        width: end_x - x,
        height: end_y - y,
    }
}

fn upload_rect_bytes(rect: AtlasRect, bytes_per_pixel: u32) -> u64 {
    u64::from(aligned_bytes_per_row(rect.width, bytes_per_pixel).unwrap_or(0))
        * u64::from(rect.height)
}

struct TextDrawAllocation {
    draw: TextDrawBinding,
    allocation: TextAtlasAllocation,
}

pub(super) fn padded_atlas_extent(width: u32, height: u32) -> Option<(u32, u32)> {
    Some((
        width.checked_add(TEXT_ATLAS_PADDING * 2)?,
        height.checked_add(TEXT_ATLAS_PADDING * 2)?,
    ))
}

pub(super) fn text_atlas_page_size(max_texture_dimension_2d: u32) -> Option<(u32, u32)> {
    let width = TEXT_ATLAS_PAGE_WIDTH.min(max_texture_dimension_2d);
    let height = TEXT_ATLAS_PAGE_HEIGHT.min(max_texture_dimension_2d);
    (width > TEXT_ATLAS_PADDING * 2 && height > TEXT_ATLAS_PADDING * 2).then_some((width, height))
}

fn atlas_content_uv(allocation: AtlasRect, page_width: u32, page_height: u32) -> Rect {
    Rect::new(
        (allocation.x + TEXT_ATLAS_PADDING) as f32 / page_width as f32,
        (allocation.y + TEXT_ATLAS_PADDING) as f32 / page_height as f32,
        (allocation.width - TEXT_ATLAS_PADDING * 2) as f32 / page_width as f32,
        (allocation.height - TEXT_ATLAS_PADDING * 2) as f32 / page_height as f32,
    )
}

fn rich_span_cache_keys(
    spans: &[crate::ui::widget::CanvasTextSpanPrimitive],
) -> Arc<[TextRichSpanCacheKey]> {
    spans
        .iter()
        .map(|span| TextRichSpanCacheKey {
            content: Arc::clone(&span.content),
            content_hash: text_content_hash(&span.content),
            font_family: span.font_family.clone(),
            color: span.color.to_rgba8(),
            font_size_bits: span.font_size.to_bits(),
            font_weight: span.font_weight.to_raw(),
            line_height_bits: span.line_height.map(f32::to_bits),
            letter_spacing_bits: span.letter_spacing.to_bits(),
        })
        .collect::<Vec<_>>()
        .into()
}

pub(super) fn text_content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn text_raster_cache_color(color: TguiColor) -> [u8; 4] {
    let mut rgba = color.to_rgba8();
    // The text texture is always rasterized at full top-level alpha. Runtime opacity is encoded
    // in the quad vertices, so alpha-only animation must reuse the same shaped/rasterized pixels.
    // Rich-span alpha remains part of `TextRichSpanCacheKey` because it is baked per span.
    rgba[3] = 255;
    rgba
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RgbaUploadBounds {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
    has_ink: bool,
}

impl RgbaUploadBounds {
    fn include_clipped_rect(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        texture_width: u32,
        texture_height: u32,
    ) {
        let left = i64::from(x).clamp(0, i64::from(texture_width)) as u32;
        let top = i64::from(y).clamp(0, i64::from(texture_height)) as u32;
        let right = (i64::from(x) + i64::from(width)).clamp(0, i64::from(texture_width)) as u32;
        let bottom = (i64::from(y) + i64::from(height)).clamp(0, i64::from(texture_height)) as u32;
        if left >= right || top >= bottom {
            return;
        }

        if self.has_ink {
            self.min_x = self.min_x.min(left);
            self.min_y = self.min_y.min(top);
            self.max_x = self.max_x.max(right);
            self.max_y = self.max_y.max(bottom);
        } else {
            self.min_x = left;
            self.min_y = top;
            self.max_x = right;
            self.max_y = bottom;
            self.has_ink = true;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RgbaUploadPlan {
    origin_x: u32,
    origin_y: u32,
    width: u32,
    height: u32,
    bytes_per_row: u32,
    data_len: usize,
}

fn aligned_rgba_bytes_per_row(width: u32) -> Option<u32> {
    aligned_bytes_per_row(width, RGBA_BYTES_PER_PIXEL)
}

fn aligned_bytes_per_row(width: u32, bytes_per_pixel: u32) -> Option<u32> {
    let row_bytes = width.checked_mul(bytes_per_pixel)?;
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    row_bytes
        .checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
}

fn prepare_text_upload_scratch(pixels: &mut Vec<u8>, required_len: usize) {
    pixels.resize(required_len, 0);
    pixels.fill(0);
}

/// Packs the clipped ink rectangle at the beginning of the reusable raster buffer.
///
/// Both source and destination strides are 256-byte aligned. That lets wgpu's
/// `Queue::write_texture` take its contiguous staging fast path instead of allocating
/// another padded image and copying every row internally. Rows are moved from top to
/// bottom; the packed destination never reaches a not-yet-read source row.
fn pack_rgba_upload_in_place(
    pixels: &mut [u8],
    raster_bytes_per_row: u32,
    texture_width: u32,
    texture_height: u32,
    bounds: RgbaUploadBounds,
) -> Option<RgbaUploadPlan> {
    if !bounds.has_ink {
        return None;
    }
    debug_assert!(bounds.max_x <= texture_width);
    debug_assert!(bounds.max_y <= texture_height);

    let width = bounds.max_x - bounds.min_x;
    let height = bounds.max_y - bounds.min_y;
    let bytes_per_row = aligned_rgba_bytes_per_row(width)?;
    let copied_row_bytes = width.checked_mul(RGBA_BYTES_PER_PIXEL)? as usize;
    let raster_stride = raster_bytes_per_row as usize;
    let upload_stride = bytes_per_row as usize;
    let source_x = bounds.min_x.checked_mul(RGBA_BYTES_PER_PIXEL)? as usize;

    for row in 0..height as usize {
        let source_row = bounds.min_y as usize + row;
        let source_start = source_row
            .checked_mul(raster_stride)?
            .checked_add(source_x)?;
        let source_end = source_start.checked_add(copied_row_bytes)?;
        let destination_start = row.checked_mul(upload_stride)?;
        pixels.copy_within(source_start..source_end, destination_start);
    }

    let data_len = (height as usize - 1)
        .checked_mul(upload_stride)?
        .checked_add(copied_row_bytes)?;
    Some(RgbaUploadPlan {
        origin_x: bounds.min_x,
        origin_y: bounds.min_y,
        width,
        height,
        bytes_per_row,
        data_len,
    })
}

/// Expands a full text raster into a one-pixel transparent atlas gutter.
///
/// Atlas slots are reusable, so uploading only the ink rectangle would leave pixels
/// from the previous occupant behind. Moving rows from bottom to top lets the existing
/// raster scratch become the complete padded upload without allocating a second image.
fn pad_rgba_for_atlas_in_place(
    pixels: &mut Vec<u8>,
    raster_bytes_per_row: u32,
    width: u32,
    height: u32,
) -> Option<RgbaUploadPlan> {
    let (padded_width, padded_height) = padded_atlas_extent(width, height)?;
    let padded_bytes_per_row = aligned_rgba_bytes_per_row(padded_width)?;
    let padded_len = usize::try_from(padded_bytes_per_row)
        .ok()?
        .checked_mul(padded_height as usize)?;
    pixels.resize(padded_len, 0);

    let source_stride = raster_bytes_per_row as usize;
    let destination_stride = padded_bytes_per_row as usize;
    let copied_row_bytes = width.checked_mul(RGBA_BYTES_PER_PIXEL)? as usize;
    let left_gutter_bytes = (TEXT_ATLAS_PADDING * RGBA_BYTES_PER_PIXEL) as usize;

    for row in (0..height as usize).rev() {
        let source_start = row.checked_mul(source_stride)?;
        let source_end = source_start.checked_add(copied_row_bytes)?;
        let destination_row = row + TEXT_ATLAS_PADDING as usize;
        let destination_start = destination_row
            .checked_mul(destination_stride)?
            .checked_add(left_gutter_bytes)?;
        pixels.copy_within(source_start..source_end, destination_start);

        let row_start = destination_row.checked_mul(destination_stride)?;
        pixels[row_start..destination_start].fill(0);
        let content_end = destination_start.checked_add(copied_row_bytes)?;
        pixels[content_end..row_start + destination_stride].fill(0);
    }

    let top_gutter_end = TEXT_ATLAS_PADDING as usize * destination_stride;
    pixels[..top_gutter_end].fill(0);
    let bottom_gutter_start =
        (TEXT_ATLAS_PADDING as usize + height as usize).checked_mul(destination_stride)?;
    pixels[bottom_gutter_start..].fill(0);

    let data_len = (padded_height as usize - 1)
        .checked_mul(destination_stride)?
        .checked_add(padded_width as usize * RGBA_BYTES_PER_PIXEL as usize)?;
    Some(RgbaUploadPlan {
        origin_x: 0,
        origin_y: 0,
        width: padded_width,
        height: padded_height,
        bytes_per_row: padded_bytes_per_row,
        data_len,
    })
}

/// Expands an aligned R8 paragraph raster with a transparent one-pixel gutter.
/// Rows move bottom-to-top, so the one-row downward shift remains overlap-safe
/// even when source and destination both use the minimum 256-byte stride.
fn pad_r8_for_atlas_in_place(
    pixels: &mut Vec<u8>,
    raster_bytes_per_row: u32,
    width: u32,
    height: u32,
) -> Option<RgbaUploadPlan> {
    let (padded_width, padded_height) = padded_atlas_extent(width, height)?;
    let padded_bytes_per_row = aligned_bytes_per_row(padded_width, 1)?;
    let source_stride = raster_bytes_per_row as usize;
    let destination_stride = padded_bytes_per_row as usize;
    let padded_len = destination_stride.checked_mul(padded_height as usize)?;
    pixels.resize(padded_len, 0);

    for row in (0..height as usize).rev() {
        let source_start = row.checked_mul(source_stride)?;
        let source_end = source_start.checked_add(width as usize)?;
        let destination_row = row + TEXT_ATLAS_PADDING as usize;
        let destination_start = destination_row
            .checked_mul(destination_stride)?
            .checked_add(TEXT_ATLAS_PADDING as usize)?;
        pixels.copy_within(source_start..source_end, destination_start);

        let row_start = destination_row.checked_mul(destination_stride)?;
        pixels[row_start..destination_start].fill(0);
        let content_end = destination_start.checked_add(width as usize)?;
        pixels[content_end..row_start + destination_stride].fill(0);
    }

    let top_gutter_end = TEXT_ATLAS_PADDING as usize * destination_stride;
    pixels[..top_gutter_end].fill(0);
    let bottom_gutter_start =
        (TEXT_ATLAS_PADDING as usize + height as usize).checked_mul(destination_stride)?;
    pixels[bottom_gutter_start..].fill(0);

    let data_len = (padded_height as usize - 1)
        .checked_mul(destination_stride)?
        .checked_add(padded_width as usize)?;
    Some(RgbaUploadPlan {
        origin_x: 0,
        origin_y: 0,
        width: padded_width,
        height: padded_height,
        bytes_per_row: padded_bytes_per_row,
        data_len,
    })
}

/// Restores a direct R8 paragraph raster to the legacy white RGBA mask layout.
/// This is a cold defensive fallback used only if an atlas-eligible allocation
/// unexpectedly cannot reserve a page.
fn expand_r8_mask_to_rgba_in_place(
    pixels: &mut Vec<u8>,
    source_bytes_per_row: u32,
    width: u32,
    height: u32,
) -> Option<u32> {
    let destination_bytes_per_row = aligned_rgba_bytes_per_row(width)?;
    let destination_len = usize::try_from(destination_bytes_per_row)
        .ok()?
        .checked_mul(height as usize)?;
    pixels.resize(destination_len, 0);
    let source_stride = source_bytes_per_row as usize;
    let destination_stride = destination_bytes_per_row as usize;
    for row in (0..height as usize).rev() {
        let source_row = row.checked_mul(source_stride)?;
        let destination_row = row.checked_mul(destination_stride)?;
        for x in (0..width as usize).rev() {
            let coverage = pixels[source_row + x];
            let destination = destination_row.checked_add(x.checked_mul(4)?)?;
            pixels[destination..destination + 4].copy_from_slice(&[255, 255, 255, coverage]);
        }
        pixels[destination_row + width as usize * 4..destination_row + destination_stride].fill(0);
    }
    Some(destination_bytes_per_row)
}

fn attrs_for_text(text: &TextPrimitive, font_size: f32, letter_spacing: f32) -> Attrs<'_> {
    let family = text
        .font_family
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(Family::Name)
        .unwrap_or(Family::SansSerif);

    Attrs::new()
        .family(family)
        .weight(text_weight(text.font_weight))
        .letter_spacing(letter_spacing / font_size.max(1.0))
}

fn attrs_for_span(
    span: &crate::ui::widget::CanvasTextSpanPrimitive,
    fallback_font_size: f32,
    fallback_line_height: f32,
    fallback_letter_spacing: f32,
) -> Attrs<'_> {
    let font_size = span.font_size.max(1.0);
    let family = span
        .font_family
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(Family::Name)
        .unwrap_or(Family::SansSerif);

    Attrs::new()
        .family(family)
        .color(color_to_text(span.color))
        .weight(text_weight(span.font_weight))
        .metrics(Metrics::new(
            if span.font_size > 0.0 {
                span.font_size
            } else {
                fallback_font_size
            },
            span.line_height.unwrap_or(fallback_line_height),
        ))
        .letter_spacing(
            span.letter_spacing
                / font_size
                    .max(fallback_font_size)
                    .max(fallback_letter_spacing.abs())
                    .max(1.0),
        )
}

fn text_wrap(text: &TextPrimitive) -> cosmic_text::Wrap {
    match text.wrap {
        crate::ui::widget::CanvasTextWrap::Word => cosmic_text::Wrap::WordOrGlyph,
        crate::ui::widget::CanvasTextWrap::Glyph => cosmic_text::Wrap::Glyph,
        crate::ui::widget::CanvasTextWrap::None => cosmic_text::Wrap::None,
    }
}

fn text_offsets(text: &TextPrimitive, buffer: &Buffer, width: f32, height: f32) -> (i32, i32) {
    let mut content_width = 0.0f32;
    let mut content_height = 0.0f32;
    for run in buffer.layout_runs() {
        content_width = content_width.max(run.line_w.max(0.0));
        content_height = content_height.max(run.line_top + run.line_height);
    }
    if content_height <= 0.0 {
        content_height = text.line_height;
    }

    let offset_x = match text.horizontal_align {
        crate::ui::widget::CanvasTextHorizontalAlign::Start => 0.0,
        crate::ui::widget::CanvasTextHorizontalAlign::Center => {
            ((width - content_width).max(0.0)) * 0.5
        }
        crate::ui::widget::CanvasTextHorizontalAlign::End => (width - content_width).max(0.0),
    };
    let offset_y = match text.vertical_align {
        crate::ui::widget::CanvasTextVerticalAlign::Start => 0.0,
        crate::ui::widget::CanvasTextVerticalAlign::Center => {
            ((height - content_height).max(0.0)) * 0.5
        }
        crate::ui::widget::CanvasTextVerticalAlign::End => (height - content_height).max(0.0),
    };

    (offset_x.round() as i32, offset_y.round() as i32)
}

fn wrap_mode_key(text: &TextPrimitive) -> u8 {
    match text.wrap {
        crate::ui::widget::CanvasTextWrap::Word => 0,
        crate::ui::widget::CanvasTextWrap::Glyph => 1,
        crate::ui::widget::CanvasTextWrap::None => 2,
    }
}

fn overflow_mode_key(text: &TextPrimitive) -> u8 {
    match text.overflow {
        crate::ui::widget::CanvasTextOverflow::Clip => 0,
        crate::ui::widget::CanvasTextOverflow::Ellipsis => 1,
    }
}

fn horizontal_align_key(text: &TextPrimitive) -> u8 {
    match text.horizontal_align {
        crate::ui::widget::CanvasTextHorizontalAlign::Start => 0,
        crate::ui::widget::CanvasTextHorizontalAlign::Center => 1,
        crate::ui::widget::CanvasTextHorizontalAlign::End => 2,
    }
}

fn vertical_align_key(text: &TextPrimitive) -> u8 {
    match text.vertical_align {
        crate::ui::widget::CanvasTextVerticalAlign::Start => 0,
        crate::ui::widget::CanvasTextVerticalAlign::Center => 1,
        crate::ui::widget::CanvasTextVerticalAlign::End => 2,
    }
}

fn overflow_content(
    text: &TextPrimitive,
    buffer: &mut Buffer,
    font_system: &mut cosmic_text::FontSystem,
    attrs: &Attrs<'_>,
) -> String {
    if !matches!(
        text.overflow,
        crate::ui::widget::CanvasTextOverflow::Ellipsis
    ) {
        return text.content.to_string();
    }

    buffer.set_text(&text.content, attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);

    if !text_requires_ellipsis(text, buffer, font_system) {
        return text.content.to_string();
    }

    ellipsize_at_grapheme_boundary(&text.content, |ellipsized| {
        buffer.set_text(ellipsized, attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(font_system, false);
        !text_requires_ellipsis(text, buffer, font_system)
    })
}

/// Finds the longest fitting prefix without splitting an extended grapheme cluster.
///
/// The full content is already known to overflow when this is called. Probe from the
/// end with exponential backoff so the common case (removing only a short suffix)
/// needs very few shaping passes, then binary-search the fitting boundary. Candidate
/// construction is linear in its prefix length, but shaping is reduced from O(n)
/// passes to O(log n) passes in the worst case.
fn ellipsize_at_grapheme_boundary(content: &str, mut fits: impl FnMut(&str) -> bool) -> String {
    let boundaries = content
        .grapheme_indices(true)
        .map(|(byte_index, _)| byte_index)
        .collect::<Vec<_>>();
    let Some(mut known_overflowing) = boundaries.len().checked_sub(1) else {
        return "…".to_string();
    };

    let mut candidate = String::with_capacity(content.len() + '…'.len_utf8());
    let mut probe = |boundary_index: usize| {
        write_ellipsis_candidate(&mut candidate, content, boundaries[boundary_index]);
        fits(&candidate)
    };

    // Ellipsis overflow always removes at least one grapheme, matching the previous
    // path's behavior of popping before its first fit check.
    if probe(known_overflowing) {
        return candidate;
    }
    if known_overflowing == 0 {
        return "…".to_string();
    }

    let furthest_boundary = known_overflowing;
    let mut distance = 1usize;
    loop {
        let fitting_candidate = furthest_boundary.saturating_sub(distance);
        if probe(fitting_candidate) {
            let mut low = fitting_candidate;
            let mut high = known_overflowing;
            while high - low > 1 {
                let middle = low + (high - low) / 2;
                if probe(middle) {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            write_ellipsis_candidate(&mut candidate, content, boundaries[low]);
            return candidate;
        }

        if fitting_candidate == 0 {
            // Preserve the established behavior for a box narrower than the
            // ellipsis itself: still render the ellipsis and let clipping apply.
            return "…".to_string();
        }

        known_overflowing = fitting_candidate;
        distance = distance.saturating_mul(2);
    }
}

fn write_ellipsis_candidate(candidate: &mut String, content: &str, byte_index: usize) {
    candidate.clear();
    candidate.push_str(content[..byte_index].trim_end_matches(char::is_whitespace));
    candidate.push('…');
}

fn text_requires_ellipsis(
    text: &TextPrimitive,
    buffer: &mut Buffer,
    font_system: &mut cosmic_text::FontSystem,
) -> bool {
    let width = buffer.size().0.unwrap_or_default();
    let height = buffer.size().1.unwrap_or_default();
    let max_lines = ((height / text.line_height.max(1.0)).floor() as usize).max(1);
    let mut line_count = 0usize;

    // `layout_runs()` only exposes visible runs, so it cannot tell whether more
    // wrapped or explicit lines exist below the buffer. Inspect line layouts until
    // the height budget is exceeded to keep max-lines overflow detection correct.
    for line_index in 0..buffer.lines.len() {
        let Some(layout_lines) = buffer.line_layout(font_system, line_index) else {
            continue;
        };
        for line in layout_lines {
            line_count += 1;
            if line_count > max_lines || line.w > width + 0.5 {
                return true;
            }
        }
    }
    false
}

fn text_weight(weight: FontWeight) -> Weight {
    Weight(weight.to_raw())
}

fn color_to_text(color: TguiColor) -> Color {
    Color::rgba(color.r, color.g, color.b, color.a)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg(any(test, feature = "bench-support"))]
pub(crate) struct TextBlendStats {
    pub(crate) transparent_source_pixels: usize,
    pub(crate) direct_copy_pixels: usize,
    pub(crate) general_blend_pixels: usize,
}

#[inline(always)]
#[cfg_attr(any(test, feature = "bench-support"), allow(dead_code))]
fn blend_pixel(pixels: &mut [u8], bytes_per_row: u32, x: u32, y: u32, src: [u8; 4]) {
    let index = (y * bytes_per_row + x * RGBA_BYTES_PER_PIXEL) as usize;
    let dst = &mut pixels[index..index + 4];

    if src[3] == 0 && (dst[3] != 0 || dst[..3] == [0, 0, 0]) {
        return;
    }
    if src[3] == 255 || (dst[3] == 0 && src[3] != 0) {
        dst.copy_from_slice(&src);
        return;
    }

    blend_pixel_general(dst, src);
}

#[inline(always)]
fn blend_pixel_general(dst: &mut [u8], src: [u8; 4]) {
    let src_alpha = src[3] as f32 / 255.0;
    let dst_alpha = dst[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

    if out_alpha <= 0.0 {
        dst.copy_from_slice(&[0, 0, 0, 0]);
        return;
    }

    for channel in 0..3 {
        let src_value = src[channel] as f32 / 255.0;
        let dst_value = dst[channel] as f32 / 255.0;
        let out = (src_value * src_alpha + dst_value * dst_alpha * (1.0 - src_alpha)) / out_alpha;
        dst[channel] = (out * 255.0).round() as u8;
    }
    dst[3] = (out_alpha * 255.0).round() as u8;
}

#[inline(always)]
#[cfg_attr(any(test, feature = "bench-support"), allow(dead_code))]
fn blend_coverage(pixels: &mut [u8], bytes_per_row: u32, x: u32, y: u32, src: u8) {
    let dst = &mut pixels[(y * bytes_per_row + x) as usize];
    if src == 0 {
        return;
    }
    if src == 255 || *dst == 0 {
        *dst = src;
        return;
    }
    *dst = blend_coverage_general(*dst, src);
}

#[inline(always)]
fn blend_coverage_general(dst: u8, src: u8) -> u8 {
    let src_alpha = src as f32 / 255.0;
    let dst_alpha = dst as f32 / 255.0;
    ((src_alpha + dst_alpha * (1.0 - src_alpha)) * 255.0).round() as u8
}

#[cfg(any(test, feature = "bench-support"))]
#[inline(always)]
fn blend_pixel_controlled(
    pixels: &mut [u8],
    bytes_per_row: u32,
    x: u32,
    y: u32,
    src: [u8; 4],
    fast_path_enabled: bool,
    stats: &mut TextBlendStats,
) {
    let index = (y * bytes_per_row + x * RGBA_BYTES_PER_PIXEL) as usize;
    let dst = &mut pixels[index..index + 4];
    if fast_path_enabled {
        if src[3] == 0 && (dst[3] != 0 || dst[..3] == [0, 0, 0]) {
            stats.transparent_source_pixels += 1;
            return;
        }
        if src[3] == 255 || (dst[3] == 0 && src[3] != 0) {
            stats.direct_copy_pixels += 1;
            dst.copy_from_slice(&src);
            return;
        }
    }
    stats.general_blend_pixels += 1;
    blend_pixel_general(dst, src);
}

#[cfg(any(test, feature = "bench-support"))]
#[inline(always)]
fn blend_coverage_controlled(
    pixels: &mut [u8],
    bytes_per_row: u32,
    x: u32,
    y: u32,
    src: u8,
    fast_path_enabled: bool,
    stats: &mut TextBlendStats,
) {
    let dst = &mut pixels[(y * bytes_per_row + x) as usize];
    if fast_path_enabled {
        if src == 0 {
            stats.transparent_source_pixels += 1;
            return;
        }
        if src == 255 || *dst == 0 {
            stats.direct_copy_pixels += 1;
            *dst = src;
            return;
        }
    }
    stats.general_blend_pixels += 1;
    *dst = blend_coverage_general(*dst, src);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::color::Color as TguiColor;
    use crate::ui::widget::{
        CanvasTextHorizontalAlign, CanvasTextOverflow, CanvasTextSpanPrimitive,
        CanvasTextVerticalAlign, CanvasTextWrap,
    };

    fn blend_reference(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
        let mut pixel = dst;
        blend_pixel_general(&mut pixel, src);
        pixel
    }

    #[test]
    fn text_blend_fast_paths_are_byte_exact_for_random_rgba_pixels() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut stats = TextBlendStats::default();
        for _ in 0..1_000_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let dst = (state as u32).to_le_bytes();
            state = state.wrapping_mul(0xd134_2543_de82_ef95);
            let src = (state as u32).to_le_bytes();
            let expected = blend_reference(dst, src);
            let mut actual = dst.to_vec();
            blend_pixel_controlled(&mut actual, 4, 0, 0, src, true, &mut stats);
            assert_eq!(actual.as_slice(), expected, "dst={dst:?} src={src:?}");
        }
        assert!(stats.transparent_source_pixels > 0);
        assert!(stats.direct_copy_pixels > 0);
        assert!(stats.general_blend_pixels > 0);
    }

    #[test]
    fn text_blend_fast_paths_cover_masks_color_glyphs_and_rich_span_overlap() {
        let cases = [
            ([0, 0, 0, 0], [12, 34, 56, 0]),
            ([0, 0, 0, 0], [240, 240, 240, 91]),
            ([32, 64, 96, 128], [255, 64, 16, 255]),
            ([20, 80, 160, 210], [220, 40, 120, 117]),
            ([255, 200, 30, 180], [24, 180, 240, 96]),
        ];
        let mut stats = TextBlendStats::default();
        for (dst, src) in cases {
            let expected = blend_reference(dst, src);
            let mut actual = dst.to_vec();
            blend_pixel_controlled(&mut actual, 4, 0, 0, src, true, &mut stats);
            assert_eq!(actual.as_slice(), expected);
        }
        assert_eq!(stats.transparent_source_pixels, 1);
        assert_eq!(stats.direct_copy_pixels, 2);
        assert_eq!(stats.general_blend_pixels, 2);
    }

    #[test]
    fn direct_r8_coverage_blend_matches_rgba_mask_alpha_byte_for_byte() {
        for dst in 0u8..=255 {
            for src in 0u8..=255 {
                let expected = blend_reference([255, 255, 255, dst], [255, 255, 255, src])[3];
                let mut actual = vec![dst];
                let mut stats = TextBlendStats::default();
                blend_coverage_controlled(&mut actual, 1, 0, 0, src, true, &mut stats);
                assert_eq!(actual[0], expected, "dst={dst} src={src}");
            }
        }
    }

    #[test]
    fn text_upload_rows_are_copy_aligned() {
        assert_eq!(aligned_rgba_bytes_per_row(1), Some(256));
        assert_eq!(aligned_rgba_bytes_per_row(63), Some(256));
        assert_eq!(aligned_rgba_bytes_per_row(64), Some(256));
        assert_eq!(aligned_rgba_bytes_per_row(65), Some(512));
        for width in 1..=1024 {
            assert_eq!(
                aligned_rgba_bytes_per_row(width).unwrap() % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
                0
            );
        }
    }

    #[test]
    fn text_upload_scratch_reuses_capacity_and_clears_old_pixels() {
        let mut scratch = Vec::new();
        prepare_text_upload_scratch(&mut scratch, 64 * 1024);
        let pointer = scratch.as_ptr();
        let capacity = scratch.capacity();
        scratch[17] = 255;

        for _ in 0..1_000 {
            prepare_text_upload_scratch(&mut scratch, 32 * 1024);
            assert_eq!(scratch.as_ptr(), pointer);
            assert_eq!(scratch.capacity(), capacity);
            assert!(scratch.iter().all(|byte| *byte == 0));
            scratch[17] = 255;
        }
    }

    #[test]
    fn text_atlas_allocator_never_overlaps_live_regions() {
        let mut allocator = TextAtlasAllocator::new(128, 96);
        let mut allocations: Vec<AtlasRect> = Vec::new();
        for &(width, height) in &[(31, 19), (47, 17), (20, 33), (52, 21), (17, 41)] {
            if let Some(allocation) = allocator.allocate(width, height) {
                for existing in &allocations {
                    assert!(
                        allocation.right() <= existing.x
                            || existing.right() <= allocation.x
                            || allocation.bottom() <= existing.y
                            || existing.bottom() <= allocation.y,
                        "atlas allocations overlap: {allocation:?} and {existing:?}"
                    );
                }
                allocations.push(allocation);
            }
        }
        assert!(allocations.len() >= 3);
    }

    fn atlas_rects_overlap(left: AtlasRect, right: AtlasRect) -> bool {
        left.x < right.right()
            && right.x < left.right()
            && left.y < right.bottom()
            && right.y < left.bottom()
    }

    fn assert_atlas_partition(allocator: &TextAtlasAllocator, live: &[AtlasRect]) {
        assert_eq!(allocator.live_allocations, live.len());
        for (index, rect) in live.iter().copied().enumerate() {
            assert!(rect.width > 0 && rect.height > 0);
            assert!(rect.right() <= allocator.width && rect.bottom() <= allocator.height);
            for other in live.iter().copied().skip(index + 1) {
                assert!(
                    !atlas_rects_overlap(rect, other),
                    "live atlas allocations overlap: {rect:?} and {other:?}"
                );
            }
            for free in allocator.free.iter().copied() {
                assert!(
                    !atlas_rects_overlap(rect, free),
                    "live atlas allocation overlaps free space: {rect:?} and {free:?}"
                );
            }
        }
        for (index, rect) in allocator.free.iter().copied().enumerate() {
            assert!(rect.width > 0 && rect.height > 0);
            assert!(rect.right() <= allocator.width && rect.bottom() <= allocator.height);
            for other in allocator.free.iter().copied().skip(index + 1) {
                assert!(
                    !atlas_rects_overlap(rect, other),
                    "atlas free rectangles overlap: {rect:?} and {other:?}"
                );
            }
        }

        let live_area: u64 = live.iter().copied().map(AtlasRect::area).sum();
        let free_area: u64 = allocator.free.iter().copied().map(AtlasRect::area).sum();
        assert_eq!(
            live_area + free_area,
            u64::from(allocator.width) * u64::from(allocator.height),
            "atlas live/free rectangles must remain an exact page partition"
        );
    }

    #[test]
    fn text_atlas_dense_dashboard_slots_remain_disjoint() {
        let mut allocator = TextAtlasAllocator::new(2048, 256);
        let mut live = Vec::new();
        while let Some(allocation) = allocator.allocate(22, 24) {
            assert!(
                live.len() < 2_000,
                "allocator failed to exhaust a finite page"
            );
            live.push(allocation);
        }

        // This fixed-size workload reaches the tall-remainder split that previously emitted two
        // overlapping free rectangles late in a dense page.
        assert!(live.len() > 800);
        assert_atlas_partition(&allocator, &live);

        while let Some(allocation) = live.pop() {
            allocator.release(allocation);
        }
        assert_atlas_partition(&allocator, &live);
        assert_eq!(
            allocator.free,
            vec![AtlasRect {
                x: 0,
                y: 0,
                width: 2048,
                height: 256,
            }]
        );
    }

    #[test]
    fn text_atlas_randomized_allocate_release_preserves_partition() {
        let mut allocator = TextAtlasAllocator::new(257, 193);
        let mut live = Vec::new();
        let mut state = 0x5eed_cafe_u64;

        for step in 0..2_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let release = !live.is_empty() && (state >> 61) == 0;
            if release {
                let index = (state as usize) % live.len();
                let allocation = live.swap_remove(index);
                allocator.release(allocation);
            } else {
                let width = 1 + ((state >> 8) % 37) as u32;
                let height = 1 + ((state >> 24) % 31) as u32;
                if let Some(allocation) = allocator.allocate(width, height) {
                    live.push(allocation);
                }
            }

            if step % 32 == 0 {
                assert_atlas_partition(&allocator, &live);
            }
        }
        assert_atlas_partition(&allocator, &live);

        while !live.is_empty() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let index = (state as usize) % live.len();
            allocator.release(live.swap_remove(index));
        }
        assert_atlas_partition(&allocator, &live);
        assert!(allocator.is_empty());
        assert_eq!(allocator.free.len(), 1);
        assert_eq!(allocator.free[0].area(), 257 * 193);
    }

    #[cfg(feature = "bench-support")]
    fn read_texture_bytes(
        renderer: &Renderer,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        bytes_per_pixel: u32,
    ) -> Vec<u8> {
        let bytes_per_row = aligned_bytes_per_row(width, bytes_per_pixel).unwrap();
        let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tgui-text-atlas-isolation-readback"),
            size: u64::from(bytes_per_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = renderer
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tgui-text-atlas-isolation-copy"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        renderer.queue.submit(Some(encoder.finish()));

        let slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        renderer
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("atlas readback device poll");
        receiver
            .recv()
            .expect("atlas readback callback")
            .expect("atlas readback map");
        let mapped = slice.get_mapped_range();
        let tight_row_bytes = width as usize * bytes_per_pixel as usize;
        let mut pixels = vec![0; tight_row_bytes * height as usize];
        for row in 0..height as usize {
            let source = row * bytes_per_row as usize;
            let destination = row * tight_row_bytes;
            pixels[destination..destination + tight_row_bytes]
                .copy_from_slice(&mapped[source..source + tight_row_bytes]);
        }
        drop(mapped);
        buffer.unmap();
        pixels
    }

    #[cfg(feature = "bench-support")]
    fn read_rgba_texture(
        renderer: &Renderer,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
    ) -> Vec<u8> {
        read_texture_bytes(renderer, texture, width, height, RGBA_BYTES_PER_PIXEL)
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn text_atlas_dense_gpu_uploads_keep_live_slots_isolated() {
        let Ok((mut renderer, _)) = pollster::block_on(Renderer::new_headless_for_bench(
            crate::platform::dpi::PhysicalSize::new(1, 1),
            TguiColor::BLACK,
        )) else {
            eprintln!("skipping text atlas GPU isolation test: no headless adapter");
            return;
        };
        let width = 2048;
        let height = 256;
        let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-text-atlas-isolation"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let legacy_texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-text-atlas-isolation-legacy"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let binding = renderer.create_text_sprite_binding(&texture, "tgui-text-atlas-isolation");
        let page_shadow_bytes = width as usize * height as usize * 4;
        let mut atlas = TextAtlas::with_shadow_budget(8192, page_shadow_bytes);
        atlas.add_page(texture, binding, TextAtlasFormat::Rgba);
        let mut allocations = Vec::new();
        while let Some(reservation) = atlas.allocate_existing(20, 22, TextAtlasFormat::Rgba) {
            let allocation = reservation.allocation;
            let index = allocations.len() as u32 + 1;
            let color = [
                (index & 0xff) as u8,
                ((index >> 8) & 0xff) as u8,
                ((index >> 16) & 0xff) as u8,
                255,
            ];
            let bytes_per_row = aligned_rgba_bytes_per_row(allocation.width).unwrap();
            let mut upload = vec![0; bytes_per_row as usize * allocation.height as usize];
            for row in TEXT_ATLAS_PADDING as usize
                ..allocation.height as usize - TEXT_ATLAS_PADDING as usize
            {
                let content_start = row * bytes_per_row as usize
                    + TEXT_ATLAS_PADDING as usize * RGBA_BYTES_PER_PIXEL as usize;
                for pixel in upload[content_start
                    ..content_start + (allocation.width - TEXT_ATLAS_PADDING * 2) as usize * 4]
                    .chunks_exact_mut(4)
                {
                    pixel.copy_from_slice(&color);
                }
            }
            assert!(atlas.stage_upload(
                allocation,
                &upload,
                RgbaUploadPlan {
                    origin_x: 0,
                    origin_y: 0,
                    width: allocation.width,
                    height: allocation.height,
                    bytes_per_row,
                    data_len: upload.len(),
                },
            ));
            renderer.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &legacy_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: allocation.x,
                        y: allocation.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &upload,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(allocation.height),
                },
                wgpu::Extent3d {
                    width: allocation.width,
                    height: allocation.height,
                    depth_or_array_layers: 1,
                },
            );
            allocations.push((allocation, color));
        }
        assert!(allocations.len() > 800);
        atlas.flush_pending_uploads(&renderer.queue);
        atlas.reset_upload_stats();

        // Reuse a released live-page slot and prove the shadow overwrites both
        // content and transparent gutters before the next aggregated flush.
        let released = allocations[0].0;
        atlas.release(released);
        let replacement = atlas
            .allocate_existing(20, 22, TextAtlasFormat::Rgba)
            .expect("released atlas slot must be reusable")
            .allocation;
        assert_eq!((replacement.x, replacement.y), (released.x, released.y));
        let replacement_color = [7, 19, 251, 255];
        let replacement_stride = aligned_rgba_bytes_per_row(replacement.width).unwrap();
        let mut replacement_upload =
            vec![0; replacement_stride as usize * replacement.height as usize];
        for row in 1..replacement.height as usize - 1 {
            let start = row * replacement_stride as usize + 4;
            for pixel in replacement_upload[start..start + (replacement.width as usize - 2) * 4]
                .chunks_exact_mut(4)
            {
                pixel.copy_from_slice(&replacement_color);
            }
        }
        assert!(atlas.stage_upload(
            replacement,
            &replacement_upload,
            RgbaUploadPlan {
                origin_x: 0,
                origin_y: 0,
                width: replacement.width,
                height: replacement.height,
                bytes_per_row: replacement_stride,
                data_len: replacement_upload.len(),
            },
        ));
        renderer.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &legacy_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: replacement.x,
                    y: replacement.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &replacement_upload,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(replacement_stride),
                rows_per_image: Some(replacement.height),
            },
            wgpu::Extent3d {
                width: replacement.width,
                height: replacement.height,
                depth_or_array_layers: 1,
            },
        );
        allocations[0] = (replacement, replacement_color);
        atlas.flush_pending_uploads(&renderer.queue);
        let stats = atlas.upload_stats();
        assert!(stats.write_calls <= MAX_DIRTY_UPLOAD_RECTS_PER_PAGE);
        assert_eq!(stats.shadow_bytes, page_shadow_bytes);
        assert_eq!(stats.shadow_budget_bytes, page_shadow_bytes);

        let pixels = read_rgba_texture(
            &renderer,
            &atlas.page(1).expect("isolation atlas page")._texture,
            width,
            height,
        );
        let legacy_pixels = read_rgba_texture(&renderer, &legacy_texture, width, height);
        assert_eq!(pixels, legacy_pixels);
        for (allocation, expected) in allocations.iter().copied() {
            let content_index =
                ((allocation.y as usize + 1) * width as usize + allocation.x as usize + 1) * 4;
            let sample = content_index..content_index + 4;
            assert_eq!(&pixels[sample], &expected);
            let gutter_index = (allocation.y as usize * width as usize + allocation.x as usize) * 4;
            assert_eq!(&pixels[gutter_index..gutter_index + 4], &[0, 0, 0, 0]);
        }

        // The next page exceeds the explicit shadow budget. It must remain a valid
        // atlas page and use the immediate upload fallback with identical content.
        let fallback_texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-text-atlas-shadow-budget-fallback"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let fallback_binding =
            renderer.create_text_sprite_binding(&fallback_texture, "tgui-text-atlas-fallback");
        atlas.add_page(fallback_texture, fallback_binding, TextAtlasFormat::Rgba);
        let fallback = atlas
            .allocate_existing(20, 22, TextAtlasFormat::Rgba)
            .expect("shadowless fallback page allocation")
            .allocation;
        assert_eq!(fallback.page_id, 2);
        let fallback_stride = aligned_rgba_bytes_per_row(fallback.width).unwrap();
        let fallback_color = [199, 31, 73, 255];
        let mut fallback_upload = vec![0; fallback_stride as usize * fallback.height as usize];
        for row in 1..fallback.height as usize - 1 {
            let start = row * fallback_stride as usize + 4;
            for pixel in fallback_upload[start..start + (fallback.width as usize - 2) * 4]
                .chunks_exact_mut(4)
            {
                pixel.copy_from_slice(&fallback_color);
            }
        }
        let fallback_plan = RgbaUploadPlan {
            origin_x: 0,
            origin_y: 0,
            width: fallback.width,
            height: fallback.height,
            bytes_per_row: fallback_stride,
            data_len: fallback_upload.len(),
        };
        assert!(!atlas.stage_upload(fallback, &fallback_upload, fallback_plan));
        let fallback_page = atlas.page(fallback.page_id).unwrap();
        renderer.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &fallback_page._texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: fallback.x,
                    y: fallback.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &fallback_upload,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(fallback_stride),
                rows_per_image: Some(fallback.height),
            },
            wgpu::Extent3d {
                width: fallback.width,
                height: fallback.height,
                depth_or_array_layers: 1,
            },
        );
        let fallback_pixels = read_rgba_texture(
            &renderer,
            &atlas.page(fallback.page_id).unwrap()._texture,
            width,
            height,
        );
        let fallback_content =
            ((fallback.y as usize + 1) * width as usize + fallback.x as usize + 1) * 4;
        assert_eq!(
            &fallback_pixels[fallback_content..fallback_content + 4],
            &fallback_color
        );
        let fallback_gutter = (fallback.y as usize * width as usize + fallback.x as usize) * 4;
        assert_eq!(
            &fallback_pixels[fallback_gutter..fallback_gutter + 4],
            &[0, 0, 0, 0]
        );

        for (allocation, _) in allocations {
            atlas.release(allocation);
        }
        assert_eq!(atlas.upload_stats().shadow_bytes, 0);
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn r8_atlas_upload_reuse_budget_and_format_isolation_are_exact() {
        let Ok((mut renderer, _)) = pollster::block_on(Renderer::new_headless_for_bench(
            crate::platform::dpi::PhysicalSize::new(1, 1),
            TguiColor::BLACK,
        )) else {
            eprintln!("skipping R8 text atlas GPU test: no headless adapter");
            return;
        };
        let width = TEXT_ATLAS_PAGE_WIDTH;
        let height = TEXT_ATLAS_PAGE_HEIGHT;
        let r8_page_bytes = width as usize * height as usize;
        let mut atlas = TextAtlas::with_shadow_budget(8192, r8_page_bytes);

        let r8_texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-text-r8-atlas-exactness"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let r8_binding =
            renderer.create_text_sprite_binding(&r8_texture, "tgui-text-r8-atlas-exactness");
        atlas.add_page(r8_texture, r8_binding, TextAtlasFormat::R8Coverage);
        let first = atlas
            .allocate_existing(20, 22, TextAtlasFormat::R8Coverage)
            .expect("first R8 allocation")
            .allocation;
        let survivor = atlas
            .allocate_existing(20, 22, TextAtlasFormat::R8Coverage)
            .expect("surviving R8 allocation")
            .allocation;

        let stage_coverage =
            |atlas: &mut TextAtlas, allocation: TextAtlasAllocation, coverage: u8| {
                let stride = aligned_bytes_per_row(allocation.width, 1).unwrap();
                let mut upload = vec![0; stride as usize * allocation.height as usize];
                for row in 1..allocation.height as usize - 1 {
                    let start = row * stride as usize + 1;
                    upload[start..start + allocation.width as usize - 2].fill(coverage);
                }
                assert!(atlas.stage_upload(
                    allocation,
                    &upload,
                    RgbaUploadPlan {
                        origin_x: 0,
                        origin_y: 0,
                        width: allocation.width,
                        height: allocation.height,
                        bytes_per_row: stride,
                        data_len: upload.len(),
                    },
                ));
            };
        stage_coverage(&mut atlas, first, 73);
        stage_coverage(&mut atlas, survivor, 181);
        atlas.flush_pending_uploads(&renderer.queue);
        atlas.reset_upload_stats();

        atlas.release(first);
        let replacement = atlas
            .allocate_existing(20, 22, TextAtlasFormat::R8Coverage)
            .expect("released R8 slot must be reusable")
            .allocation;
        assert_eq!((replacement.x, replacement.y), (first.x, first.y));
        stage_coverage(&mut atlas, replacement, 247);
        atlas.flush_pending_uploads(&renderer.queue);

        let stats = atlas.upload_stats();
        assert!(stats.write_calls <= MAX_DIRTY_UPLOAD_RECTS_PER_PAGE);
        assert!(stats.uploaded_bytes > 0);
        assert_eq!(stats.uploaded_bytes, stats.r8_uploaded_bytes);
        assert_eq!(stats.rgba_uploaded_bytes, 0);
        assert_eq!(stats.shadow_bytes, r8_page_bytes);
        assert_eq!(stats.r8_shadow_bytes, r8_page_bytes);
        assert_eq!(stats.rgba_shadow_bytes, 0);

        let page = atlas.page(replacement.page_id).expect("live R8 page");
        assert_eq!(page.format, TextAtlasFormat::R8Coverage);
        let pixels = read_texture_bytes(&renderer, &page._texture, width, height, 1);
        let sample = |allocation: TextAtlasAllocation| {
            pixels[(allocation.y as usize + 1) * width as usize + allocation.x as usize + 1]
        };
        assert_eq!(sample(replacement), 247);
        assert_eq!(sample(survivor), 181);
        assert_eq!(
            pixels[replacement.y as usize * width as usize + replacement.x as usize],
            0
        );

        // The R8 page consumes the entire explicit shadow budget. A mixed RGBA
        // page remains valid, but cleanly uses immediate uploads instead of
        // exceeding the shared CPU residency cap.
        let rgba_texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-text-rgba-atlas-after-r8-budget"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let rgba_binding = renderer
            .create_text_sprite_binding(&rgba_texture, "tgui-text-rgba-atlas-after-r8-budget");
        atlas.add_page(rgba_texture, rgba_binding, TextAtlasFormat::Rgba);
        let rgba = atlas
            .allocate_existing(20, 22, TextAtlasFormat::Rgba)
            .expect("mixed RGBA allocation")
            .allocation;
        assert_ne!(rgba.page_id, replacement.page_id);
        assert_eq!(rgba.format, TextAtlasFormat::Rgba);
        let rgba_stride = aligned_rgba_bytes_per_row(rgba.width).unwrap();
        let rgba_upload = vec![0; rgba_stride as usize * rgba.height as usize];
        assert!(!atlas.stage_upload(
            rgba,
            &rgba_upload,
            RgbaUploadPlan {
                origin_x: 0,
                origin_y: 0,
                width: rgba.width,
                height: rgba.height,
                bytes_per_row: rgba_stride,
                data_len: rgba_upload.len(),
            },
        ));
        assert_eq!(atlas.upload_stats().shadow_bytes, r8_page_bytes);

        atlas.release(replacement);
        atlas.release(survivor);
        atlas.release(rgba);
        assert_eq!(atlas.page_count(), 0);
        assert_eq!(atlas.upload_stats().shadow_bytes, 0);
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn r8_dedicated_fallback_gpu_upload_matches_white_rgba_mask() {
        let Ok((renderer, _)) = pollster::block_on(Renderer::new_headless_for_bench(
            crate::platform::dpi::PhysicalSize::new(1, 1),
            TguiColor::BLACK,
        )) else {
            eprintln!("skipping R8 dedicated fallback GPU test: no headless adapter");
            return;
        };
        let width = 67u32;
        let height = 5u32;
        let r8_stride = aligned_bytes_per_row(width, 1).unwrap();
        let mut pixels = vec![0u8; r8_stride as usize * height as usize];
        let mut expected = vec![0u8; width as usize * height as usize * 4];
        for y in 0..height {
            for x in 0..width {
                let coverage = x.wrapping_mul(17).wrapping_add(y.wrapping_mul(43)) as u8;
                pixels[(y * r8_stride + x) as usize] = coverage;
                let expected_index = ((y * width + x) * 4) as usize;
                expected[expected_index..expected_index + 4]
                    .copy_from_slice(&[255, 255, 255, coverage]);
            }
        }
        let rgba_stride = expand_r8_mask_to_rgba_in_place(&mut pixels, r8_stride, width, height)
            .expect("R8 dedicated fallback expansion");
        let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-text-r8-dedicated-fallback-readback"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        renderer.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(rgba_stride),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        assert_eq!(
            read_rgba_texture(&renderer, &texture, width, height),
            expected
        );
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn plain_mask_text_reuses_one_raster_across_rgb_and_alpha_changes() {
        let Ok((mut renderer, _)) = pollster::block_on(Renderer::new_headless_for_bench(
            crate::platform::dpi::PhysicalSize::new(320, 80),
            TguiColor::BLACK,
        )) else {
            eprintln!("skipping paragraph mask cache test: no headless adapter");
            return;
        };
        let font_manager = FontManager::new(&crate::text::font::FontCatalog::default());
        assert!(!renderer
            .text_system
            .prepare_font_system(font_manager.cache_identity()));
        let mut text = text_primitive(
            "Theme-aware retained text",
            300.0,
            40.0,
            CanvasTextWrap::None,
        );
        text.color = TguiColor::rgba(25, 110, 230, 255);

        let blue = renderer
            .text_bind_group_for(&text, &font_manager)
            .expect("blue mask raster")
            .expect("blue mask draw");
        assert!(blue.tintable_mask);
        assert!(blue.r8_coverage);
        assert_eq!(renderer.text_cache_hits, 0);
        assert_eq!(renderer.text_cache_misses, 1);
        let cache_len = renderer.text_cache.len();

        text.color = TguiColor::rgba(238, 74, 96, 61);
        let pink = renderer
            .text_bind_group_for(&text, &font_manager)
            .expect("pink mask cache lookup")
            .expect("pink mask draw");
        assert!(pink.tintable_mask);
        assert!(pink.r8_coverage);
        assert_eq!(pink.binding.id, blue.binding.id);
        assert_eq!(pink.uv_rect, blue.uv_rect);
        assert_eq!(renderer.text_cache.len(), cache_len);
        assert_eq!(renderer.text_cache_hits, 1);
        assert_eq!(renderer.text_cache_misses, 1);
        assert_eq!(renderer.text_atlas_releases, 0);
        assert_eq!(renderer.text_prepare_cache_clears, 0);

        // The same resolver is used by scene liveness. It must recover the normalized mask key
        // from a differently colored primitive so the live atlas slot is never released early.
        let live_key = renderer
            .text_cache_key(&text)
            .expect("live mask cache key after RGB change");
        assert!(live_key.tintable_mask);
        assert!(renderer.text_cache.contains_key(&live_key));

        // Font IDs are database-local. The render loop performs this exact identity transition
        // before clearing whole-text textures, so no mask classification can survive a manager
        // switch and alias glyphs from the previous database.
        let other_font_manager = FontManager::new(&crate::text::font::FontCatalog::default());
        assert!(renderer
            .text_system
            .prepare_font_system(other_font_manager.cache_identity()));
        renderer.clear_text_cache();
        assert!(renderer.text_cache.is_empty());
        let rebuilt = renderer
            .text_bind_group_for(&text, &other_font_manager)
            .expect("mask rebuild after font identity change")
            .expect("mask draw after font identity change");
        assert!(rebuilt.tintable_mask);
        assert!(rebuilt.r8_coverage);
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn rich_text_and_color_glyphs_never_use_vertex_tint() {
        let Ok((mut renderer, _)) = pollster::block_on(Renderer::new_headless_for_bench(
            crate::platform::dpi::PhysicalSize::new(320, 80),
            TguiColor::BLACK,
        )) else {
            eprintln!("skipping paragraph RGBA fallback test: no headless adapter");
            return;
        };
        let font_manager = FontManager::new(&crate::text::font::FontCatalog::default());
        renderer
            .text_system
            .prepare_font_system(font_manager.cache_identity());

        let plain = text_primitive("plain mask", 100.0, 40.0, CanvasTextWrap::None);
        let plain_draw = renderer
            .text_bind_group_for(&plain, &font_manager)
            .expect("plain R8 raster")
            .expect("plain R8 draw");
        assert!(plain_draw.tintable_mask);
        assert!(plain_draw.r8_coverage);

        let mut rich = text_primitive("rich", 100.0, 40.0, CanvasTextWrap::None);
        rich.rich_spans = Some(
            vec![crate::ui::widget::CanvasTextSpanPrimitive {
                content: Arc::from("rich"),
                font_family: None,
                color: TguiColor::rgba(220, 40, 80, 255),
                font_size: 16.0,
                font_weight: FontWeight::NORMAL,
                line_height: Some(20.0),
                letter_spacing: 0.0,
            }]
            .into(),
        );
        let rich_draw = renderer
            .text_bind_group_for(&rich, &font_manager)
            .expect("rich RGBA raster")
            .expect("rich RGBA draw");
        assert!(!rich_draw.tintable_mask);
        assert!(!rich_draw.r8_coverage);
        assert_ne!(plain_draw.binding.id, rich_draw.binding.id);
        assert!(renderer
            .text_atlas
            .pages
            .iter()
            .any(|page| page.format == TextAtlasFormat::R8Coverage));
        assert!(renderer
            .text_atlas
            .pages
            .iter()
            .any(|page| page.format == TextAtlasFormat::Rgba));

        let emoji = text_primitive("😀", 64.0, 40.0, CanvasTextWrap::None);
        let emoji_draw = renderer
            .text_bind_group_for(&emoji, &font_manager)
            .expect("emoji raster");
        if let Some(emoji_draw) = emoji_draw {
            let has_color_glyph = font_manager.with_font_system(|font_system| {
                let mut buffer = Buffer::new(font_system, Metrics::new(16.0, 20.0));
                buffer.set_size(Some(64.0), Some(40.0));
                buffer.set_text("😀", &Attrs::new(), Shaping::Advanced, None);
                buffer.shape_until_scroll(font_system, false);
                buffer.layout_runs().any(|run| {
                    run.glyphs.iter().any(|glyph| {
                        let physical = glyph.physical((0.0, run.line_y), 1.0);
                        renderer
                            .text_system
                            .swash_cache
                            .get_image(font_system, physical.cache_key)
                            .as_ref()
                            .is_some_and(|image| image.content != SwashContent::Mask)
                    })
                })
            });
            if has_color_glyph {
                assert!(!emoji_draw.tintable_mask);
                assert!(!emoji_draw.r8_coverage);
            }
        }
    }

    #[test]
    fn dirty_atlas_rows_are_bounded_and_preserve_sparse_regions() {
        let mut rows = vec![None; 256];
        for index in 0..40_u32 {
            let y = index * 6;
            rows[y as usize] = Some((index * 11 % 1800, index * 11 % 1800 + 24));
        }
        let rects = dirty_upload_rects(&rows, RGBA_BYTES_PER_PIXEL);
        assert!(!rects.is_empty());
        assert!(rects.len() <= MAX_DIRTY_UPLOAD_RECTS_PER_PAGE);
        for (y, span) in rows.into_iter().enumerate() {
            let Some((left, right)) = span else {
                continue;
            };
            assert!(rects.iter().any(|rect| {
                rect.y <= y as u32
                    && rect.bottom() > y as u32
                    && rect.x <= left
                    && rect.right() >= right
            }));
        }
    }

    #[test]
    fn text_atlas_release_coalesces_back_to_full_page() {
        let mut allocator = TextAtlasAllocator::new(128, 64);
        let first = allocator.allocate(64, 32).unwrap();
        let second = allocator.allocate(64, 32).unwrap();
        let third = allocator.allocate(64, 32).unwrap();
        let fourth = allocator.allocate(64, 32).unwrap();
        assert!(allocator.allocate(1, 1).is_none());

        // Release in a deliberately unfriendly order to exercise repeated merging.
        allocator.release(second);
        allocator.release(fourth);
        allocator.release(first);
        allocator.release(third);
        assert!(allocator.is_empty());
        assert_eq!(
            allocator.free,
            vec![AtlasRect {
                x: 0,
                y: 0,
                width: 128,
                height: 64
            }]
        );
        assert_eq!(
            allocator.allocate(128, 64),
            Some(AtlasRect {
                x: 0,
                y: 0,
                width: 128,
                height: 64
            })
        );
    }

    #[test]
    fn text_atlas_uv_excludes_transparent_gutter() {
        let allocation = AtlasRect {
            x: 10,
            y: 20,
            width: 102,
            height: 22,
        };
        assert_eq!(
            atlas_content_uv(allocation, 512, 256),
            Rect::new(11.0 / 512.0, 21.0 / 256.0, 100.0 / 512.0, 20.0 / 256.0)
        );
    }

    #[test]
    fn oversized_text_cleanly_falls_back_from_atlas() {
        let atlas = TextAtlas::new(8192);
        assert_eq!(atlas.page_size_for(2046, 254), Some((2048, 256)));
        assert_eq!(atlas.page_size_for(2047, 20), None);
        assert_eq!(atlas.page_size_for(200, 255), None);

        // Extremely constrained adapters disable the atlas rather than creating
        // an invalid zero-sized page; dedicated textures remain available.
        let constrained = TextAtlas::new(2);
        assert_eq!(constrained.page_size_for(1, 1), None);
    }

    #[test]
    fn shared_32_mib_shadow_budget_accepts_exact_mixed_boundary_only() {
        let r8_page = TEXT_ATLAS_PAGE_WIDTH as usize * TEXT_ATLAS_PAGE_HEIGHT as usize;
        let rgba_page = r8_page * RGBA_BYTES_PER_PIXEL as usize;
        assert_eq!(r8_page, 512 * 1024);
        assert_eq!(rgba_page, 2 * 1024 * 1024);

        // Eight RGBA pages plus thirty-one R8 pages leave exactly one R8 page.
        let mixed_residency = 8 * rgba_page + 31 * r8_page;
        assert!(shadow_fits_budget(
            mixed_residency,
            r8_page,
            TEXT_ATLAS_SHADOW_BUDGET_BYTES
        ));
        assert!(!shadow_fits_budget(
            mixed_residency + 1,
            r8_page,
            TEXT_ATLAS_SHADOW_BUDGET_BYTES
        ));
        assert!(!shadow_fits_budget(
            usize::MAX,
            r8_page,
            TEXT_ATLAS_SHADOW_BUDGET_BYTES
        ));
    }

    #[test]
    fn padded_atlas_upload_preserves_rgba_and_clears_all_gutters() {
        let width = 67u32;
        let height = 3u32;
        let source_stride = aligned_rgba_bytes_per_row(width).unwrap();
        let mut pixels = vec![0u8; source_stride as usize * height as usize];
        for y in 0..height {
            for x in 0..width {
                let index = (y * source_stride + x * 4) as usize;
                pixels[index..index + 4].copy_from_slice(&[
                    x as u8,
                    y as u8,
                    x.wrapping_add(y) as u8,
                    255,
                ]);
            }
        }

        let plan = pad_rgba_for_atlas_in_place(&mut pixels, source_stride, width, height).unwrap();
        assert_eq!((plan.width, plan.height), (69, 5));
        let stride = plan.bytes_per_row as usize;
        for y in 0..plan.height as usize {
            for x in 0..plan.width as usize {
                let pixel = &pixels[y * stride + x * 4..y * stride + x * 4 + 4];
                if y == 0 || y + 1 == plan.height as usize || x == 0 || x + 1 == plan.width as usize
                {
                    assert_eq!(pixel, &[0, 0, 0, 0]);
                } else {
                    let source_x = x - 1;
                    let source_y = y - 1;
                    assert_eq!(
                        pixel,
                        &[
                            source_x as u8,
                            source_y as u8,
                            source_x.wrapping_add(source_y) as u8,
                            255,
                        ]
                    );
                }
            }
        }
    }

    #[test]
    fn padded_r8_mask_upload_preserves_coverage_alignment_and_gutters() {
        for (width, height) in [(1u32, 7u32), (67, 3)] {
            let source_stride = aligned_bytes_per_row(width, 1).unwrap();
            let mut pixels = vec![0u8; source_stride as usize * height as usize];
            for y in 0..height {
                for x in 0..width {
                    let index = (y * source_stride + x) as usize;
                    pixels[index] = x.wrapping_mul(31).wrapping_add(y.wrapping_mul(47)) as u8;
                }
            }

            let plan = pad_r8_for_atlas_in_place(&mut pixels, source_stride, width, height)
                .expect("valid R8 mask upload");
            assert_eq!((plan.width, plan.height), (width + 2, height + 2));
            assert_eq!(plan.bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
            let stride = plan.bytes_per_row as usize;
            for y in 0..plan.height as usize {
                for x in 0..plan.width as usize {
                    let actual = pixels[y * stride + x];
                    if y == 0
                        || y + 1 == plan.height as usize
                        || x == 0
                        || x + 1 == plan.width as usize
                    {
                        assert_eq!(actual, 0, "non-zero R8 gutter at ({x}, {y})");
                    } else {
                        let source_x = (x - 1) as u32;
                        let source_y = (y - 1) as u32;
                        let expected = source_x
                            .wrapping_mul(31)
                            .wrapping_add(source_y.wrapping_mul(47))
                            as u8;
                        assert_eq!(actual, expected, "coverage mismatch at ({x}, {y})");
                    }
                }
            }
            assert_eq!(
                plan.data_len,
                (plan.height as usize - 1) * stride + plan.width as usize
            );
        }
    }

    #[test]
    fn r8_mask_defensive_fallback_restores_white_rgba_rows_exactly() {
        for (width, height) in [(1u32, 7u32), (67, 3)] {
            let source_stride = aligned_bytes_per_row(width, 1).unwrap();
            let mut pixels = vec![0u8; source_stride as usize * height as usize];
            for y in 0..height {
                for x in 0..width {
                    pixels[(y * source_stride + x) as usize] =
                        x.wrapping_mul(31).wrapping_add(y.wrapping_mul(47)) as u8;
                }
            }
            let rgba_stride =
                expand_r8_mask_to_rgba_in_place(&mut pixels, source_stride, width, height)
                    .expect("valid R8 to RGBA fallback");
            assert_eq!(rgba_stride % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
            for y in 0..height {
                for x in 0..width {
                    let index = (y * rgba_stride + x * 4) as usize;
                    let coverage = x.wrapping_mul(31).wrapping_add(y.wrapping_mul(47)) as u8;
                    assert_eq!(&pixels[index..index + 4], &[255, 255, 255, coverage]);
                }
                assert!(pixels
                    [(y * rgba_stride + width * 4) as usize..((y + 1) * rgba_stride) as usize]
                    .iter()
                    .all(|byte| *byte == 0));
            }
        }
    }

    #[test]
    fn blank_text_produces_no_texture_upload_plan() {
        let mut scratch = vec![0; 256 * 8];
        assert_eq!(
            pack_rgba_upload_in_place(&mut scratch, 256, 32, 8, RgbaUploadBounds::default(),),
            None
        );
    }

    #[test]
    fn clipped_ink_bounds_include_antialias_and_shadow_edges() {
        let mut bounds = RgbaUploadBounds::default();
        // A left/top antialias fringe clipped by the texture edge.
        bounds.include_clipped_rect(-2, -1, 5, 4, 16, 10);
        // A detached lower/right shadow fringe.
        bounds.include_clipped_rect(12, 7, 6, 5, 16, 10);

        assert_eq!(
            bounds,
            RgbaUploadBounds {
                min_x: 0,
                min_y: 0,
                max_x: 16,
                max_y: 10,
                has_ink: true,
            }
        );
    }

    fn raster_test_text_bounds(content: &str) -> (RgbaUploadBounds, RgbaUploadBounds) {
        let width = 96u32;
        let height = 32u32;
        let stride = aligned_rgba_bytes_per_row(width).unwrap();
        let mut pixels = vec![0u8; stride as usize * height as usize];
        let mut tracked = RgbaUploadBounds::default();
        let mut font_system = cosmic_text::FontSystem::new();
        let mut swash_cache = cosmic_text::SwashCache::new();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(18.0, 24.0));
        buffer.set_size(Some(width as f32), Some(height as f32));
        buffer.set_text(content, &Attrs::new(), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut font_system, false);
        buffer.draw(
            &mut font_system,
            &mut swash_cache,
            cosmic_text::Color::rgb(255, 255, 255),
            |x, y, w, h, color| {
                let rgba = color.as_rgba();
                if rgba[3] != 0 {
                    tracked.include_clipped_rect(x, y, w, h, width, height);
                }
                for dy in 0..h {
                    for dx in 0..w {
                        let px = x + dx as i32;
                        let py = y + dy as i32;
                        if px >= 0 && py >= 0 && px < width as i32 && py < height as i32 {
                            blend_pixel(&mut pixels, stride, px as u32, py as u32, rgba);
                        }
                    }
                }
            },
        );

        let mut scanned = RgbaUploadBounds::default();
        for y in 0..height {
            for x in 0..width {
                let alpha = pixels[(y * stride + x * RGBA_BYTES_PER_PIXEL + 3) as usize];
                if alpha != 0 {
                    scanned.include_clipped_rect(x as i32, y as i32, 1, 1, width, height);
                }
            }
        }
        (tracked, scanned)
    }

    #[test]
    fn tracked_cosmic_text_bounds_match_all_antialiased_pixels() {
        let (tracked, scanned) = raster_test_text_bounds("Ågj");
        assert!(tracked.has_ink);
        assert_eq!(tracked, scanned);
    }

    #[test]
    fn whitespace_only_raster_has_zero_uploads() {
        let (tracked, scanned) = raster_test_text_bounds("   \t");
        assert_eq!(tracked, RgbaUploadBounds::default());
        assert_eq!(scanned, RgbaUploadBounds::default());
    }

    #[test]
    fn packed_ink_upload_reconstructs_full_rgba_texture_byte_for_byte() {
        let width = 67u32;
        let height = 9u32;
        let raster_stride = aligned_rgba_bytes_per_row(width).unwrap();
        let mut raster = vec![0u8; raster_stride as usize * height as usize];
        let mut bounds = RgbaUploadBounds::default();

        // Two overlapping translucent glyph/shadow rectangles exercise blending,
        // antialias alpha, non-zero origins, and a non-64px-multiple texture width.
        for &(x, y, w, h, rgba) in &[
            (7, 2, 13, 3, [30, 90, 220, 96]),
            (10, 3, 21, 4, [240, 250, 255, 173]),
        ] {
            bounds.include_clipped_rect(x, y, w, h, width, height);
            for py in y as u32..y as u32 + h {
                for px in x as u32..x as u32 + w {
                    blend_pixel(&mut raster, raster_stride, px, py, rgba);
                }
            }
        }

        let mut expected = vec![0u8; width as usize * height as usize * 4];
        for row in 0..height as usize {
            let source = row * raster_stride as usize;
            let destination = row * width as usize * 4;
            expected[destination..destination + width as usize * 4]
                .copy_from_slice(&raster[source..source + width as usize * 4]);
        }

        let plan = pack_rgba_upload_in_place(&mut raster, raster_stride, width, height, bounds)
            .expect("non-transparent text should produce an upload");
        assert_eq!(plan.origin_x, 7);
        assert_eq!(plan.origin_y, 2);
        assert_eq!(plan.width, 24);
        assert_eq!(plan.height, 5);
        assert_eq!(plan.bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);

        // WebGPU initializes the untouched part of a newly created texture to zero.
        // Replaying the exact copy layout over zeroes must therefore reproduce the
        // previous full-frame upload byte-for-byte.
        let mut reconstructed = vec![0u8; expected.len()];
        let copied_row_bytes = plan.width as usize * 4;
        for row in 0..plan.height as usize {
            let source = row * plan.bytes_per_row as usize;
            let destination =
                ((plan.origin_y as usize + row) * width as usize + plan.origin_x as usize) * 4;
            reconstructed[destination..destination + copied_row_bytes]
                .copy_from_slice(&raster[source..source + copied_row_bytes]);
        }
        assert_eq!(reconstructed, expected);

        let full_staging_bytes = raster_stride as usize * height as usize;
        let packed_staging_bytes = plan.data_len;
        assert!(packed_staging_bytes < full_staging_bytes / 2);
    }

    #[test]
    fn rich_span_style_participates_in_text_cache_identity() {
        let base = CanvasTextSpanPrimitive {
            content: Arc::from("status"),
            font_family: Some(Arc::from("Inter")),
            color: TguiColor::WHITE,
            font_size: 14.0,
            font_weight: FontWeight::NORMAL,
            line_height: Some(18.0),
            letter_spacing: 0.0,
        };
        let mut changed = base.clone();
        changed.color = TguiColor::rgba(64, 128, 255, 255);

        let base_key = rich_span_cache_keys(&[base]);
        let changed_key = rich_span_cache_keys(&[changed]);
        assert!(base_key != changed_key);

        let plain = TextCacheKey {
            content: Arc::from("status"),
            content_hash: text_content_hash("status"),
            rich_spans: None,
            font_family: None,
            width: 100,
            height: 20,
            color: [255; 4],
            tintable_mask: false,
            force_color: false,
            font_size_bits: 14.0f32.to_bits(),
            line_height_bits: 18.0f32.to_bits(),
            letter_spacing_bits: 0.0f32.to_bits(),
            font_weight: FontWeight::NORMAL.to_raw(),
            wrap_mode: 0,
            overflow_mode: 0,
            horizontal_align: 0,
            vertical_align: 0,
        };
        assert!(
            TextCacheKey {
                rich_spans: Some(base_key),
                ..plain.clone()
            } != TextCacheKey {
                rich_spans: Some(changed_key),
                ..plain
            }
        );
    }

    #[test]
    fn top_level_alpha_reuses_raster_but_rgb_and_rich_alpha_do_not() {
        assert_eq!(
            text_raster_cache_color(TguiColor::rgba(32, 96, 160, 24)),
            text_raster_cache_color(TguiColor::rgba(32, 96, 160, 240)),
        );
        assert_ne!(
            text_raster_cache_color(TguiColor::rgba(32, 96, 160, 240)),
            text_raster_cache_color(TguiColor::rgba(33, 96, 160, 240)),
        );

        let span = CanvasTextSpanPrimitive {
            content: Arc::from("status"),
            font_family: None,
            color: TguiColor::rgba(32, 96, 160, 24),
            font_size: 14.0,
            font_weight: FontWeight::NORMAL,
            line_height: Some(18.0),
            letter_spacing: 0.0,
        };
        let mut changed_alpha = span.clone();
        changed_alpha.color = TguiColor::rgba(32, 96, 160, 240);
        assert!(
            rich_span_cache_keys(&[span]) != rich_span_cache_keys(&[changed_alpha]),
            "rich-span alpha is baked into the shared RGBA text texture"
        );
    }

    fn text_primitive(
        content: &str,
        width: f32,
        height: f32,
        wrap: CanvasTextWrap,
    ) -> TextPrimitive {
        TextPrimitive {
            content: Arc::from(content),
            rich_spans: None,
            frame: Rect::new(0.0, 0.0, width, height),
            quad: None,
            color: TguiColor::WHITE,
            force_color: false,
            font_family: None,
            font_size: 16.0,
            font_weight: FontWeight::NORMAL,
            line_height: 20.0,
            letter_spacing: 0.0,
            wrap,
            overflow: CanvasTextOverflow::Ellipsis,
            horizontal_align: CanvasTextHorizontalAlign::Start,
            vertical_align: CanvasTextVerticalAlign::Start,
            clip_rect: None,
            clip_mask: None,
        }
    }

    fn shape_ellipsis(
        content: &str,
        width: f32,
        height: f32,
        wrap: CanvasTextWrap,
    ) -> (String, bool, bool) {
        let text = text_primitive(content, width, height, wrap);
        let mut font_system = cosmic_text::FontSystem::new();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(16.0, 20.0));
        buffer.set_size(Some(width), Some(height));
        buffer.set_wrap(text_wrap(&text));
        let attrs = attrs_for_text(&text, 16.0, 0.0);

        buffer.set_text(content, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut font_system, false);
        let original_overflows = text_requires_ellipsis(&text, &mut buffer, &mut font_system);

        let result = overflow_content(&text, &mut buffer, &mut font_system, &attrs);
        buffer.set_text(&result, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut font_system, false);
        let result_fits = !text_requires_ellipsis(&text, &mut buffer, &mut font_system);
        (result, original_overflows, result_fits)
    }

    fn assert_grapheme_prefix(content: &str, ellipsized: &str) {
        let prefix = ellipsized
            .strip_suffix('…')
            .expect("ellipsized content should end in an ellipsis");
        assert!(content.starts_with(prefix));
        assert!(content
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .any(|index| index == prefix.len()));
    }

    #[test]
    fn ellipsis_search_finds_longest_ascii_prefix() {
        let content = "The quick brown fox jumps over the lazy dog";
        let (result, original_overflows, result_fits) =
            shape_ellipsis(content, 96.0, 20.0, CanvasTextWrap::None);

        assert!(original_overflows);
        assert!(result_fits);
        assert_grapheme_prefix(content, &result);
    }

    #[test]
    fn ellipsis_search_handles_cjk_graphemes() {
        let content = "现代化界面需要清晰简洁并保持高性能";
        let (result, original_overflows, result_fits) =
            shape_ellipsis(content, 96.0, 20.0, CanvasTextWrap::None);

        assert!(original_overflows);
        assert!(result_fits);
        assert_grapheme_prefix(content, &result);
    }

    #[test]
    fn ellipsis_search_does_not_split_emoji_sequence() {
        let content = "Status 👨‍👩‍👧‍👦 ready for a long retained-mode frame";
        let (result, original_overflows, result_fits) =
            shape_ellipsis(content, 108.0, 20.0, CanvasTextWrap::None);

        assert!(original_overflows);
        assert!(result_fits);
        assert_grapheme_prefix(content, &result);
    }

    #[test]
    fn ellipsis_search_honors_multiline_height_budget() {
        let content = "first line fits\nsecond line must be hidden\nthird line";
        let (result, original_overflows, result_fits) =
            shape_ellipsis(content, 220.0, 20.0, CanvasTextWrap::Word);

        assert!(original_overflows);
        assert!(result_fits);
        assert!(!result.contains('\n'));
        assert_grapheme_prefix(content, &result);
    }

    #[test]
    fn ellipsis_search_honors_wrapped_max_lines() {
        let content = "retained text wrapping should stop after exactly two visible lines even when the paragraph is much longer";
        let (result, original_overflows, result_fits) =
            shape_ellipsis(content, 112.0, 40.0, CanvasTextWrap::Word);

        assert!(original_overflows);
        assert!(result_fits);
        assert_grapheme_prefix(content, &result);
    }

    #[test]
    fn ellipsis_search_keeps_ellipsis_for_extremely_narrow_width() {
        let (result, original_overflows, result_fits) =
            shape_ellipsis("narrow", 1.0, 20.0, CanvasTextWrap::None);

        assert!(original_overflows);
        assert_eq!(result, "…");
        assert!(
            !result_fits,
            "the renderer should clip an ellipsis wider than its box"
        );
    }

    #[test]
    fn ellipsis_search_trims_whitespace_before_marker() {
        let result = ellipsize_at_grapheme_boundary("hello   world", |candidate| {
            candidate.chars().count() <= 6
        });

        assert_eq!(result, "hello…");
    }

    #[test]
    fn ellipsis_search_uses_logarithmic_fit_probes() {
        let content = "a".repeat(16_384);
        let mut probes = 0usize;
        let result = ellipsize_at_grapheme_boundary(&content, |candidate| {
            probes += 1;
            candidate.chars().count() <= 64
        });

        assert_eq!(result.graphemes(true).count(), 64);
        assert!(result.ends_with('…'));
        assert!(
            probes <= 32,
            "expected logarithmic shaping probes for 16K graphemes, got {probes}"
        );
    }
}
