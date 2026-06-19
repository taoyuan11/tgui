use crate::foundation::error::TguiError;
use crate::media::TextureFrame;

use super::{Renderer, TextureCacheEntry};

impl Renderer {
    pub(super) fn texture_bind_group_for(
        &mut self,
        texture_frame: &TextureFrame,
    ) -> Result<Option<wgpu::BindGroup>, TguiError> {
        let key = texture_frame.id();
        let (width, height) = texture_frame.size();
        if width == 0 || height == 0 {
            return Ok(None);
        }
        if !texture_frame.has_valid_rgba_len() {
            return Err(TguiError::Media(format!(
                "invalid texture frame: {}x{} requires {} RGBA bytes but has {}",
                width,
                height,
                texture_frame
                    .expected_rgba_len()
                    .map(|len| len.to_string())
                    .unwrap_or_else(|| "an overflowing number of".to_string()),
                texture_frame.pixels().len()
            )));
        }

        if let Some(entry) = self.texture_cache.get_mut(&key) {
            if entry.revision == texture_frame.revision() {
                return Ok(Some(entry.bind_group.clone()));
            }

            if entry.width == width && entry.height == height {
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &entry.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    texture_frame.pixels(),
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
                entry.revision = texture_frame.revision();
                return Ok(Some(entry.bind_group.clone()));
            }
        }

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-media-texture"),
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
            texture_frame.pixels(),
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
            label: Some("tgui-media-bind-group"),
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

        self.texture_cache.insert(
            key,
            TextureCacheEntry {
                revision: texture_frame.revision(),
                width,
                height,
                bind_group: bind_group.clone(),
                texture,
            },
        );

        Ok(Some(bind_group))
    }
}
