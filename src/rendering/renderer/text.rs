use cosmic_text::{Attrs, Buffer, Color, Family, Metrics, Shaping, Weight};

use crate::foundation::color::Color as TguiColor;
use crate::foundation::error::TguiError;
use crate::text::font::FontWeight;
use crate::ui::widget::{Rect, TextPrimitive};

use super::{Renderer, TextCacheEntry, TextCacheKey};

impl Renderer {
    pub(super) fn text_cache_key(&self, text: &TextPrimitive) -> Option<TextCacheKey> {
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
            content: text.content.clone(),
            font_family: text.font_family.clone(),
            width,
            height,
            color: text.color.to_rgba8(),
            force_color: text.force_color,
            font_size_bits: font_size.to_bits(),
            line_height_bits: line_height.to_bits(),
            letter_spacing_bits: letter_spacing.to_bits(),
            font_weight: text.font_weight.to_raw(),
        })
    }

    pub(super) fn text_bind_group_for(
        &mut self,
        text: &TextPrimitive,
    ) -> Result<Option<wgpu::BindGroup>, TguiError> {
        let Some(key) = self.text_cache_key(text) else {
            return Ok(None);
        };

        if let Some(entry) = self.text_cache.iter().find(|entry| entry.key == key) {
            return Ok(Some(entry.bind_group.clone()));
        }

        let bind_group = match self.rasterize_text(text)? {
            Some((texture, bind_group)) => {
                self.text_cache.push(TextCacheEntry {
                    key,
                    bind_group: bind_group.clone(),
                    _texture: texture,
                });
                bind_group
            }
            None => return Ok(None),
        };

        Ok(Some(bind_group))
    }

    pub(super) fn rasterize_text(
        &mut self,
        text: &TextPrimitive,
    ) -> Result<Option<(wgpu::Texture, wgpu::BindGroup)>, TguiError> {
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

        let mut buffer = Buffer::new(
            &mut self.text_system.font_system,
            Metrics::new(font_size, line_height),
        );
        buffer.set_size(Some(width as f32), Some(height as f32));
        buffer.set_wrap(cosmic_text::Wrap::WordOrGlyph);
        let attrs = attrs_for_text(text, font_size, letter_spacing);
        buffer.set_text(&text.content, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.text_system.font_system, false);

        let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
        let requested_rgba = text.color.to_rgba8();
        buffer.draw(
            &mut self.text_system.font_system,
            &mut self.text_system.swash_cache,
            color_to_text(text.color),
            |x, y, w, h, color| {
                let mut rgba = color.as_rgba();
                if text.force_color {
                    rgba[0] = requested_rgba[0];
                    rgba[1] = requested_rgba[1];
                    rgba[2] = requested_rgba[2];
                    rgba[3] = ((rgba[3] as u16 * requested_rgba[3] as u16) / 255) as u8;
                }
                for dy in 0..h {
                    for dx in 0..w {
                        let px = x + dx as i32;
                        let py = y + dy as i32;
                        if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                            continue;
                        }
                        blend_pixel(
                            &mut pixels,
                            width,
                            px as u32,
                            py as u32,
                            [rgba[0], rgba[1], rgba[2], rgba[3]],
                        );
                    }
                }
            },
        );

        if pixels.chunks_exact(4).all(|pixel| pixel[3] == 0) {
            return Ok(None);
        }

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
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-text-bind-group"),
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

        Ok(Some((texture, bind_group)))
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

fn text_weight(weight: FontWeight) -> Weight {
    Weight(weight.to_raw())
}

fn color_to_text(color: TguiColor) -> Color {
    Color::rgba(color.r, color.g, color.b, color.a)
}

fn blend_pixel(pixels: &mut [u8], width: u32, x: u32, y: u32, src: [u8; 4]) {
    let index = ((y * width + x) * 4) as usize;
    let dst = &mut pixels[index..index + 4];

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
