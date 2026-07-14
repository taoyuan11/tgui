use crate::foundation::error::TguiError;
use crate::media::TextureFrame;

use super::{Renderer, SpriteBindGroup, TextureCacheEntry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextureCacheAction {
    Reuse,
    UploadInPlace,
    Recreate,
}

fn texture_cache_action(
    cached_revision: u64,
    cached_size: (u32, u32),
    frame_revision: u64,
    frame_size: (u32, u32),
) -> TextureCacheAction {
    if cached_size != frame_size {
        TextureCacheAction::Recreate
    } else if cached_revision == frame_revision {
        TextureCacheAction::Reuse
    } else {
        TextureCacheAction::UploadInPlace
    }
}

impl Renderer {
    pub(super) fn texture_bind_group_for(
        &mut self,
        texture_frame: &TextureFrame,
    ) -> Result<Option<SpriteBindGroup>, TguiError> {
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

        let revision = texture_frame.revision();
        if let Some(entry) = self.texture_cache.get_mut(&key) {
            match texture_cache_action(
                entry.revision,
                (entry.width, entry.height),
                revision,
                (width, height),
            ) {
                TextureCacheAction::Reuse => return Ok(Some(entry.binding.clone())),
                TextureCacheAction::UploadInPlace => {
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
                    entry.revision = revision;
                    return Ok(Some(entry.binding.clone()));
                }
                TextureCacheAction::Recreate => {}
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

        let binding = SpriteBindGroup {
            id: self.allocate_sprite_bind_group_id(),
            bind_group,
        };
        self.texture_cache.insert(
            key,
            TextureCacheEntry {
                revision,
                width,
                height,
                binding: binding.clone(),
                texture,
            },
        );

        Ok(Some(binding))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_texture_revision_skips_gpu_upload() {
        assert_eq!(
            texture_cache_action(7, (320, 180), 7, (320, 180)),
            TextureCacheAction::Reuse
        );
    }

    #[test]
    fn changed_revision_with_stable_size_uploads_in_place() {
        assert_eq!(
            texture_cache_action(7, (320, 180), 8, (320, 180)),
            TextureCacheAction::UploadInPlace
        );
    }

    #[test]
    fn changed_size_recreates_even_if_revision_was_reused() {
        assert_eq!(
            texture_cache_action(7, (320, 180), 7, (640, 360)),
            TextureCacheAction::Recreate
        );
    }
}
