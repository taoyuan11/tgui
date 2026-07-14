use crate::foundation::error::TguiError;
use crate::media::TextureFrame;
#[cfg(feature = "video")]
use crate::video::backend::{
    VideoYuvColorMatrix, VideoYuvColorRange, VideoYuvColorSpace, VideoYuvFormat, VideoYuvFrame,
    VideoYuvPlane, VideoYuvPlaneFormat,
};
#[cfg(feature = "bench-support")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "video")]
use wgpu::util::DeviceExt;

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
#[cfg(feature = "video")]
use super::{
    VideoYuvPlaneCacheSignature, VideoYuvTextureCacheEntry, VideoYuvTextureCacheSignature,
};

#[cfg(feature = "video")]
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct VideoYuvUniform {
    format_matrix_range: [f32; 4],
}

#[cfg(feature = "bench-support")]
static RGBA_TEXTURE_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "bench-support")]
static RGBA_TEXTURE_CREATES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "bench-support")]
static RGBA_TEXTURE_FULL_UPLOADS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "bench-support")]
static RGBA_TEXTURE_DIRTY_UPLOADS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "bench-support")]
static RGBA_TEXTURE_UPLOADED_BYTES: AtomicU64 = AtomicU64::new(0);

#[cfg(all(feature = "bench-support", feature = "video"))]
static YUV_TEXTURE_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
#[cfg(all(feature = "bench-support", feature = "video"))]
static YUV_TEXTURE_CREATES: AtomicU64 = AtomicU64::new(0);
#[cfg(all(feature = "bench-support", feature = "video"))]
static YUV_TEXTURE_FULL_UPLOADS: AtomicU64 = AtomicU64::new(0);
#[cfg(all(feature = "bench-support", feature = "video"))]
static YUV_TEXTURE_DIRTY_UPLOADS: AtomicU64 = AtomicU64::new(0);
#[cfg(all(feature = "bench-support", feature = "video"))]
static YUV_TEXTURE_UPLOADED_BYTES: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RendererTextureDiagnostics {
    pub rgba_cache_hits: u64,
    pub rgba_creates: u64,
    pub rgba_full_uploads: u64,
    pub rgba_dirty_uploads: u64,
    pub rgba_uploaded_bytes: u64,
    pub yuv_cache_hits: u64,
    pub yuv_creates: u64,
    pub yuv_full_uploads: u64,
    pub yuv_dirty_uploads: u64,
    pub yuv_uploaded_bytes: u64,
}

#[cfg(feature = "bench-support")]
pub fn reset_renderer_texture_diagnostics() {
    RGBA_TEXTURE_CACHE_HITS.store(0, Ordering::Relaxed);
    RGBA_TEXTURE_CREATES.store(0, Ordering::Relaxed);
    RGBA_TEXTURE_FULL_UPLOADS.store(0, Ordering::Relaxed);
    RGBA_TEXTURE_DIRTY_UPLOADS.store(0, Ordering::Relaxed);
    RGBA_TEXTURE_UPLOADED_BYTES.store(0, Ordering::Relaxed);
    reset_yuv_texture_diagnostics();
}

#[cfg(feature = "bench-support")]
pub fn renderer_texture_diagnostics() -> RendererTextureDiagnostics {
    let yuv = yuv_texture_diagnostics();
    RendererTextureDiagnostics {
        rgba_cache_hits: RGBA_TEXTURE_CACHE_HITS.load(Ordering::Relaxed),
        rgba_creates: RGBA_TEXTURE_CREATES.load(Ordering::Relaxed),
        rgba_full_uploads: RGBA_TEXTURE_FULL_UPLOADS.load(Ordering::Relaxed),
        rgba_dirty_uploads: RGBA_TEXTURE_DIRTY_UPLOADS.load(Ordering::Relaxed),
        rgba_uploaded_bytes: RGBA_TEXTURE_UPLOADED_BYTES.load(Ordering::Relaxed),
        yuv_cache_hits: yuv.cache_hits,
        yuv_creates: yuv.creates,
        yuv_full_uploads: yuv.full_uploads,
        yuv_dirty_uploads: yuv.dirty_uploads,
        yuv_uploaded_bytes: yuv.uploaded_bytes,
    }
}

#[cfg(feature = "bench-support")]
#[derive(Clone, Copy, Debug, Default)]
struct YuvTextureDiagnostics {
    cache_hits: u64,
    creates: u64,
    full_uploads: u64,
    dirty_uploads: u64,
    uploaded_bytes: u64,
}

#[cfg(all(feature = "bench-support", feature = "video"))]
fn reset_yuv_texture_diagnostics() {
    YUV_TEXTURE_CACHE_HITS.store(0, Ordering::Relaxed);
    YUV_TEXTURE_CREATES.store(0, Ordering::Relaxed);
    YUV_TEXTURE_FULL_UPLOADS.store(0, Ordering::Relaxed);
    YUV_TEXTURE_DIRTY_UPLOADS.store(0, Ordering::Relaxed);
    YUV_TEXTURE_UPLOADED_BYTES.store(0, Ordering::Relaxed);
}

#[cfg(all(feature = "bench-support", not(feature = "video")))]
fn reset_yuv_texture_diagnostics() {}

#[cfg(all(feature = "bench-support", feature = "video"))]
fn yuv_texture_diagnostics() -> YuvTextureDiagnostics {
    YuvTextureDiagnostics {
        cache_hits: YUV_TEXTURE_CACHE_HITS.load(Ordering::Relaxed),
        creates: YUV_TEXTURE_CREATES.load(Ordering::Relaxed),
        full_uploads: YUV_TEXTURE_FULL_UPLOADS.load(Ordering::Relaxed),
        dirty_uploads: YUV_TEXTURE_DIRTY_UPLOADS.load(Ordering::Relaxed),
        uploaded_bytes: YUV_TEXTURE_UPLOADED_BYTES.load(Ordering::Relaxed),
    }
}

#[cfg(all(feature = "bench-support", not(feature = "video")))]
fn yuv_texture_diagnostics() -> YuvTextureDiagnostics {
    YuvTextureDiagnostics::default()
}

#[cfg(feature = "bench-support")]
fn record_rgba_texture_cache_hit() {
    RGBA_TEXTURE_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(feature = "bench-support"))]
fn record_rgba_texture_cache_hit() {}

#[cfg(feature = "bench-support")]
fn record_rgba_texture_create() {
    RGBA_TEXTURE_CREATES.fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(feature = "bench-support"))]
fn record_rgba_texture_create() {}

#[cfg(feature = "bench-support")]
fn record_rgba_texture_full_upload(bytes: usize) {
    RGBA_TEXTURE_FULL_UPLOADS.fetch_add(1, Ordering::Relaxed);
    RGBA_TEXTURE_UPLOADED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
}

#[cfg(not(feature = "bench-support"))]
fn record_rgba_texture_full_upload(_bytes: usize) {}

#[cfg(feature = "bench-support")]
fn record_rgba_texture_dirty_upload(bytes: usize) {
    RGBA_TEXTURE_DIRTY_UPLOADS.fetch_add(1, Ordering::Relaxed);
    RGBA_TEXTURE_UPLOADED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
}

#[cfg(not(feature = "bench-support"))]
fn record_rgba_texture_dirty_upload(_bytes: usize) {}

#[cfg(all(feature = "bench-support", feature = "video"))]
fn record_yuv_texture_cache_hit() {
    YUV_TEXTURE_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
}

#[cfg(any(
    all(not(feature = "bench-support"), feature = "video"),
    all(feature = "bench-support", not(feature = "video"), test)
))]
fn record_yuv_texture_cache_hit() {}

#[cfg(all(feature = "bench-support", feature = "video"))]
fn record_yuv_texture_create() {
    YUV_TEXTURE_CREATES.fetch_add(1, Ordering::Relaxed);
}

#[cfg(any(
    all(not(feature = "bench-support"), feature = "video"),
    all(feature = "bench-support", not(feature = "video"), test)
))]
fn record_yuv_texture_create() {}

#[cfg(all(feature = "bench-support", feature = "video"))]
fn record_yuv_texture_full_upload(bytes: usize) {
    YUV_TEXTURE_FULL_UPLOADS.fetch_add(1, Ordering::Relaxed);
    YUV_TEXTURE_UPLOADED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
}

#[cfg(any(
    all(not(feature = "bench-support"), feature = "video"),
    all(feature = "bench-support", not(feature = "video"), test)
))]
fn record_yuv_texture_full_upload(_bytes: usize) {}

#[cfg(all(feature = "bench-support", feature = "video"))]
fn record_yuv_texture_dirty_upload(bytes: usize) {
    YUV_TEXTURE_DIRTY_UPLOADS.fetch_add(1, Ordering::Relaxed);
    YUV_TEXTURE_UPLOADED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
}

#[cfg(any(
    all(not(feature = "bench-support"), feature = "video"),
    all(feature = "bench-support", not(feature = "video"), test)
))]
fn record_yuv_texture_dirty_upload(_bytes: usize) {}

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
                TextureCacheAction::Reuse => {
                    record_rgba_texture_cache_hit();
                    return Ok(Some(entry.binding.clone()));
                }
                TextureCacheAction::UploadInPlace => {
                    write_texture_frame_update(&self.queue, entry, texture_frame);
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
        record_rgba_texture_create();
        record_rgba_texture_full_upload(texture_frame.pixels().len());

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
                last_uploaded_pixels: initial_texture_snapshot(texture_frame),
            },
        );

        Ok(Some(binding))
    }

    #[cfg(feature = "video")]
    pub(super) fn video_yuv_bind_group_for(
        &mut self,
        frame: &VideoYuvFrame,
    ) -> Result<Option<SpriteBindGroup>, TguiError> {
        let key = frame.id();
        let (width, height) = frame.size();
        if width == 0 || height == 0 {
            return Ok(None);
        }
        let signature = video_yuv_cache_signature(frame);

        if let Some(entry) = self.video_yuv_texture_cache.get_mut(&key) {
            if entry.signature == signature {
                record_yuv_texture_cache_hit();
                if entry.revision != frame.revision() {
                    write_yuv_texture_frame_update(&self.queue, entry, frame);
                    entry.revision = frame.revision();
                }
                return Ok(Some(entry.binding.clone()));
            }
        }

        let planes = frame.planes();
        record_yuv_texture_create();
        let y_texture =
            create_yuv_plane_texture(&self.device, &self.queue, "tgui-video-y-plane", &planes[0]);
        let u_texture = create_yuv_plane_texture(
            &self.device,
            &self.queue,
            "tgui-video-u-or-uv-plane",
            &planes[1],
        );
        let v_texture = match frame.format() {
            VideoYuvFormat::Nv12 => create_dummy_v_plane_texture(&self.device, &self.queue),
            VideoYuvFormat::Yuv420p => create_yuv_plane_texture(
                &self.device,
                &self.queue,
                "tgui-video-v-plane",
                &planes[2],
            ),
        };

        let y_view = y_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let u_view = u_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let v_view = v_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let uniform = video_yuv_uniform(frame.format(), frame.color_space());
        let uniform_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tgui-video-yuv-uniform"),
                contents: bytemuck::bytes_of(&uniform),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-video-yuv-bind-group"),
            layout: &self.video_yuv_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&y_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&u_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&v_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });
        let binding = SpriteBindGroup {
            id: self.allocate_sprite_bind_group_id(),
            bind_group,
        };

        self.video_yuv_texture_cache.insert(
            key,
            VideoYuvTextureCacheEntry {
                revision: frame.revision(),
                signature,
                binding: binding.clone(),
                y_texture,
                u_texture,
                v_texture,
                _uniform_buffer: uniform_buffer,
                last_uploaded_planes: initial_yuv_plane_snapshots(frame),
            },
        );

        Ok(Some(binding))
    }
}

#[cfg(test)]
mod cache_action_tests {
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

fn initial_texture_snapshot(texture_frame: &TextureFrame) -> Option<Vec<u8>> {
    texture_frame
        .retain_upload_snapshot()
        .then(|| texture_frame.pixels().to_vec())
}

#[cfg(feature = "video")]
fn create_yuv_plane_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &'static str,
    plane: &VideoYuvPlane,
) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: plane.width,
            height: plane.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: yuv_plane_texture_format(plane.format),
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_yuv_plane_texture(queue, &texture, plane);
    texture
}

#[cfg(feature = "video")]
fn write_yuv_texture_frame_update(
    queue: &wgpu::Queue,
    entry: &mut VideoYuvTextureCacheEntry,
    frame: &VideoYuvFrame,
) {
    let planes = frame.planes();
    if entry.last_uploaded_planes.len() != planes.len() {
        write_yuv_plane_texture(queue, &entry.y_texture, &planes[0]);
        write_yuv_plane_texture(queue, &entry.u_texture, &planes[1]);
        if frame.format() == VideoYuvFormat::Yuv420p {
            write_yuv_plane_texture(queue, &entry.v_texture, &planes[2]);
        }
        entry.last_uploaded_planes = initial_yuv_plane_snapshots(frame);
        return;
    }

    write_yuv_plane_texture_update(
        queue,
        &entry.y_texture,
        &planes[0],
        &mut entry.last_uploaded_planes[0],
    );
    write_yuv_plane_texture_update(
        queue,
        &entry.u_texture,
        &planes[1],
        &mut entry.last_uploaded_planes[1],
    );
    if frame.format() == VideoYuvFormat::Yuv420p {
        write_yuv_plane_texture_update(
            queue,
            &entry.v_texture,
            &planes[2],
            &mut entry.last_uploaded_planes[2],
        );
    }
}

#[cfg(feature = "video")]
fn write_yuv_plane_texture(queue: &wgpu::Queue, texture: &wgpu::Texture, plane: &VideoYuvPlane) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        plane.bytes.as_ref(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(plane.bytes_per_row),
            rows_per_image: Some(plane.height),
        },
        wgpu::Extent3d {
            width: plane.width,
            height: plane.height,
            depth_or_array_layers: 1,
        },
    );
    record_yuv_texture_full_upload(plane.bytes.len());
}

#[cfg(feature = "video")]
fn write_yuv_plane_texture_update(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    plane: &VideoYuvPlane,
    previous: &mut Vec<u8>,
) {
    let Some(rows) = dirty_yuv_plane_rows(previous, plane) else {
        return;
    };

    let row_bytes = plane.bytes_per_row as usize;
    let start_byte = rows.start as usize * row_bytes;
    let end_byte = rows.end as usize * row_bytes;
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: 0,
                y: rows.start,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        },
        &plane.bytes.as_ref()[start_byte..end_byte],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(plane.bytes_per_row),
            rows_per_image: Some(rows.end - rows.start),
        },
        wgpu::Extent3d {
            width: plane.width,
            height: rows.end - rows.start,
            depth_or_array_layers: 1,
        },
    );
    record_yuv_texture_dirty_upload(end_byte.saturating_sub(start_byte));
    copy_dirty_texture_pixels_snapshot(previous, plane.bytes.as_ref(), start_byte..end_byte);
}

#[cfg(feature = "video")]
fn dirty_yuv_plane_rows(previous: &[u8], plane: &VideoYuvPlane) -> Option<TextureDirtyRows> {
    dirty_texture_rows(previous, plane.bytes.as_ref(), plane.bytes_per_row as usize)
}

#[cfg(feature = "video")]
fn initial_yuv_plane_snapshots(frame: &VideoYuvFrame) -> Vec<Vec<u8>> {
    frame
        .planes()
        .iter()
        .map(|plane| plane.bytes.as_ref().to_vec())
        .collect()
}

#[cfg(feature = "video")]
fn create_dummy_v_plane_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
    let plane = VideoYuvPlane::new(
        VideoYuvPlaneFormat::R8,
        1,
        1,
        1,
        std::sync::Arc::from([128_u8]),
    )
    .expect("neutral dummy YUV plane should be valid");
    create_yuv_plane_texture(device, queue, "tgui-video-dummy-v-plane", &plane)
}

#[cfg(feature = "video")]
fn yuv_plane_texture_format(format: VideoYuvPlaneFormat) -> wgpu::TextureFormat {
    match format {
        VideoYuvPlaneFormat::R8 => wgpu::TextureFormat::R8Unorm,
        VideoYuvPlaneFormat::Rg8 => wgpu::TextureFormat::Rg8Unorm,
    }
}

#[cfg(feature = "video")]
fn video_yuv_uniform(format: VideoYuvFormat, color_space: VideoYuvColorSpace) -> VideoYuvUniform {
    VideoYuvUniform {
        format_matrix_range: [
            match format {
                VideoYuvFormat::Nv12 => 0.0,
                VideoYuvFormat::Yuv420p => 1.0,
            },
            match color_space.matrix {
                VideoYuvColorMatrix::Bt601 => 0.0,
                VideoYuvColorMatrix::Bt709 => 1.0,
                VideoYuvColorMatrix::Bt2020 => 2.0,
            },
            match color_space.range {
                VideoYuvColorRange::Limited => 0.0,
                VideoYuvColorRange::Full => 1.0,
            },
            0.0,
        ],
    }
}

#[cfg(feature = "video")]
fn video_yuv_cache_signature(frame: &VideoYuvFrame) -> VideoYuvTextureCacheSignature {
    let (width, height) = frame.size();
    VideoYuvTextureCacheSignature {
        width,
        height,
        format: frame.format(),
        color_space: frame.color_space(),
        planes: frame
            .planes()
            .iter()
            .map(|plane| VideoYuvPlaneCacheSignature {
                format: plane.format,
                width: plane.width,
                height: plane.height,
            })
            .collect(),
    }
}

fn write_texture_frame_update(
    queue: &wgpu::Queue,
    entry: &mut TextureCacheEntry,
    texture_frame: &TextureFrame,
) {
    let (width, height) = texture_frame.size();
    let row_bytes = (width * 4) as usize;
    let pixels = texture_frame.pixels();

    let dirty_rows = entry
        .last_uploaded_pixels
        .as_deref()
        .and_then(|previous| dirty_texture_rows(previous, pixels, row_bytes));

    match dirty_rows {
        Some(rows) => {
            let start = rows.start as usize;
            let end = rows.end as usize;
            let start_byte = start * row_bytes;
            let end_byte = end * row_bytes;
            let dirty_byte_range = start_byte..end_byte;
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &entry.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: rows.start,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels[start_byte..end_byte],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(rows.end - rows.start),
                },
                wgpu::Extent3d {
                    width,
                    height: rows.end - rows.start,
                    depth_or_array_layers: 1,
                },
            );
            record_rgba_texture_dirty_upload(end_byte.saturating_sub(start_byte));
            if let Some(previous) = entry.last_uploaded_pixels.as_mut() {
                copy_dirty_texture_pixels_snapshot(previous, pixels, dirty_byte_range);
            }
        }
        None if entry.last_uploaded_pixels.is_some() => {}
        None => {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &entry.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
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
            record_rgba_texture_full_upload(pixels.len());
            entry.last_uploaded_pixels = Some(pixels.to_vec());
        }
    }
}

fn copy_dirty_texture_pixels_snapshot(
    previous: &mut Vec<u8>,
    current: &[u8],
    dirty_byte_range: std::ops::Range<usize>,
) {
    if previous.len() != current.len() {
        previous.clear();
        previous.extend_from_slice(current);
        return;
    }

    previous[dirty_byte_range.clone()].copy_from_slice(&current[dirty_byte_range]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TextureDirtyRows {
    start: u32,
    end: u32,
}

fn dirty_texture_rows(
    previous: &[u8],
    current: &[u8],
    row_bytes: usize,
) -> Option<TextureDirtyRows> {
    if row_bytes == 0 || current.len() % row_bytes != 0 {
        return None;
    }

    let rows = current.len() / row_bytes;
    if previous.len() != current.len() {
        return Some(TextureDirtyRows {
            start: 0,
            end: rows as u32,
        });
    }

    let start = (0..rows).find(|row| {
        let range = row * row_bytes..(row + 1) * row_bytes;
        previous[range.clone()] != current[range]
    })?;
    let end = (start + 1..=rows)
        .rev()
        .find(|row| {
            let row = row - 1;
            let range = row * row_bytes..(row + 1) * row_bytes;
            previous[range.clone()] != current[range]
        })
        .unwrap_or(start + 1);

    Some(TextureDirtyRows {
        start: start as u32,
        end: end as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        copy_dirty_texture_pixels_snapshot, dirty_texture_rows, initial_texture_snapshot,
        TextureDirtyRows,
    };
    use crate::media::TextureFrame;
    #[cfg(feature = "video")]
    use crate::video::backend::{
        VideoYuvColorMatrix, VideoYuvColorRange, VideoYuvColorSpace, VideoYuvFormat, VideoYuvFrame,
        VideoYuvPlane, VideoYuvPlaneFormat,
    };
    use std::sync::Arc;

    #[test]
    fn dirty_texture_rows_returns_none_for_identical_pixels() {
        let previous = vec![1_u8; 4 * 3];
        let current = previous.clone();

        assert_eq!(dirty_texture_rows(&previous, &current, 4), None);
    }

    #[test]
    fn initial_texture_snapshot_skips_static_texture_frames() {
        let frame = TextureFrame::new(1, 1, vec![1, 2, 3, 4]);

        assert_eq!(initial_texture_snapshot(&frame), None);
    }

    #[test]
    fn initial_texture_snapshot_retains_revised_texture_frames() {
        let pixels: Arc<[u8]> = Arc::from(vec![1, 2, 3, 4]);
        let frame = TextureFrame::with_id_revision_and_pixels(7, 1, 1, 1, pixels);

        assert_eq!(initial_texture_snapshot(&frame), Some(vec![1, 2, 3, 4]));
    }

    #[test]
    fn dirty_texture_rows_bounds_contiguous_changed_rows() {
        let previous = vec![0_u8; 4 * 5];
        let mut current = previous.clone();
        current[4..8].fill(1);
        current[8..12].fill(2);

        assert_eq!(
            dirty_texture_rows(&previous, &current, 4),
            Some(TextureDirtyRows { start: 1, end: 3 })
        );
    }

    #[test]
    fn dirty_texture_rows_bounds_sparse_changes() {
        let previous = vec![0_u8; 4 * 5];
        let mut current = previous.clone();
        current[0] = 1;
        current[4 * 4] = 2;

        assert_eq!(
            dirty_texture_rows(&previous, &current, 4),
            Some(TextureDirtyRows { start: 0, end: 5 })
        );
    }

    #[test]
    fn dirty_texture_rows_falls_back_to_full_upload_for_mismatched_lengths() {
        let previous = vec![0_u8; 4 * 4];
        let current = vec![0_u8; 4 * 5];

        assert_eq!(
            dirty_texture_rows(&previous, &current, 4),
            Some(TextureDirtyRows { start: 0, end: 5 })
        );
    }

    #[test]
    fn copy_dirty_texture_pixels_snapshot_updates_only_dirty_bytes() {
        let mut previous = vec![0_u8; 16];
        let mut current = previous.clone();
        current[4..8].fill(7);

        copy_dirty_texture_pixels_snapshot(&mut previous, &current, 4..8);

        assert_eq!(&previous[..4], &[0, 0, 0, 0]);
        assert_eq!(&previous[4..8], &[7, 7, 7, 7]);
        assert_eq!(&previous[8..], &[0; 8]);
    }

    #[test]
    fn copy_dirty_texture_pixels_snapshot_replaces_mismatched_snapshot() {
        let mut previous = vec![0_u8; 8];
        let current = vec![3_u8; 16];

        copy_dirty_texture_pixels_snapshot(&mut previous, &current, 0..16);

        assert_eq!(previous, current);
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn renderer_texture_diagnostics_reset_and_snapshot_counts_uploads() {
        super::reset_renderer_texture_diagnostics();

        super::record_rgba_texture_cache_hit();
        super::record_rgba_texture_create();
        super::record_rgba_texture_full_upload(16);
        super::record_rgba_texture_dirty_upload(8);
        super::record_yuv_texture_cache_hit();
        super::record_yuv_texture_create();
        super::record_yuv_texture_full_upload(12);
        super::record_yuv_texture_dirty_upload(4);

        let expected_yuv = if cfg!(feature = "video") { 1 } else { 0 };
        let expected_yuv_bytes = if cfg!(feature = "video") { 16 } else { 0 };
        assert_eq!(
            super::renderer_texture_diagnostics(),
            super::RendererTextureDiagnostics {
                rgba_cache_hits: 1,
                rgba_creates: 1,
                rgba_full_uploads: 1,
                rgba_dirty_uploads: 1,
                rgba_uploaded_bytes: 24,
                yuv_cache_hits: expected_yuv,
                yuv_creates: expected_yuv,
                yuv_full_uploads: expected_yuv,
                yuv_dirty_uploads: expected_yuv,
                yuv_uploaded_bytes: expected_yuv_bytes,
            }
        );

        super::reset_renderer_texture_diagnostics();
        assert_eq!(
            super::renderer_texture_diagnostics(),
            super::RendererTextureDiagnostics::default()
        );
    }

    #[cfg(feature = "video")]
    #[test]
    fn video_yuv_uniform_encodes_format_matrix_and_range() {
        let uniform = super::video_yuv_uniform(
            VideoYuvFormat::Yuv420p,
            VideoYuvColorSpace {
                matrix: VideoYuvColorMatrix::Bt2020,
                range: VideoYuvColorRange::Full,
            },
        );

        assert_eq!(uniform.format_matrix_range, [1.0, 2.0, 1.0, 0.0]);
    }

    #[cfg(feature = "video")]
    fn yuv_plane(
        format: VideoYuvPlaneFormat,
        width: u32,
        height: u32,
        bytes_per_row: u32,
        fill: u8,
    ) -> VideoYuvPlane {
        let len = bytes_per_row as usize * height as usize;
        VideoYuvPlane::new(
            format,
            width,
            height,
            bytes_per_row,
            Arc::from(vec![fill; len]),
        )
        .expect("test YUV plane should be valid")
    }

    #[cfg(feature = "video")]
    fn nv12_frame(
        revision: u64,
        color_space: VideoYuvColorSpace,
        y_stride: u32,
        uv_stride: u32,
    ) -> VideoYuvFrame {
        VideoYuvFrame::with_id_revision_and_planes(
            42,
            revision,
            4,
            2,
            VideoYuvFormat::Nv12,
            color_space,
            Arc::from(vec![
                yuv_plane(VideoYuvPlaneFormat::R8, 4, 2, y_stride, revision as u8),
                yuv_plane(
                    VideoYuvPlaneFormat::Rg8,
                    2,
                    1,
                    uv_stride,
                    revision as u8 + 1,
                ),
            ]),
        )
        .expect("test NV12 frame should be valid")
    }

    #[cfg(feature = "video")]
    #[test]
    fn video_yuv_cache_signature_reuses_layout_across_revision_stride_and_bytes() {
        let compact = nv12_frame(1, VideoYuvColorSpace::default(), 4, 4);
        let padded = nv12_frame(2, VideoYuvColorSpace::default(), 8, 8);

        assert_eq!(
            super::video_yuv_cache_signature(&compact),
            super::video_yuv_cache_signature(&padded)
        );
    }

    #[cfg(feature = "video")]
    #[test]
    fn video_yuv_cache_signature_changes_for_color_space_and_plane_layout() {
        let base = nv12_frame(1, VideoYuvColorSpace::default(), 4, 4);
        let full_range = nv12_frame(
            2,
            VideoYuvColorSpace {
                matrix: VideoYuvColorMatrix::Bt709,
                range: VideoYuvColorRange::Full,
            },
            4,
            4,
        );
        let yuv420p = VideoYuvFrame::with_id_revision_and_planes(
            42,
            3,
            4,
            2,
            VideoYuvFormat::Yuv420p,
            VideoYuvColorSpace::default(),
            Arc::from(vec![
                yuv_plane(VideoYuvPlaneFormat::R8, 4, 2, 4, 0),
                yuv_plane(VideoYuvPlaneFormat::R8, 2, 1, 2, 1),
                yuv_plane(VideoYuvPlaneFormat::R8, 2, 1, 2, 2),
            ]),
        )
        .expect("test YUV420P frame should be valid");

        let base_signature = super::video_yuv_cache_signature(&base);

        assert_ne!(
            base_signature,
            super::video_yuv_cache_signature(&full_range)
        );
        assert_ne!(base_signature, super::video_yuv_cache_signature(&yuv420p));
    }

    #[cfg(feature = "video")]
    #[test]
    fn initial_yuv_plane_snapshots_retain_actual_plane_bytes() {
        let frame = nv12_frame(3, VideoYuvColorSpace::default(), 8, 8);

        let snapshots = super::initial_yuv_plane_snapshots(&frame);

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0], vec![3_u8; 16]);
        assert_eq!(snapshots[1], vec![4_u8; 8]);
    }

    #[cfg(feature = "video")]
    #[test]
    fn dirty_yuv_plane_rows_uses_plane_stride() {
        let previous = vec![0_u8; 12];
        let mut current = previous.clone();
        current[5] = 1;
        current[10] = 2;
        let plane = VideoYuvPlane::new(VideoYuvPlaneFormat::R8, 2, 3, 4, Arc::from(current))
            .expect("padded test plane should be valid");

        assert_eq!(
            super::dirty_yuv_plane_rows(&previous, &plane),
            Some(TextureDirtyRows { start: 1, end: 3 })
        );
    }
}
