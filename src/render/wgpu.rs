//! Optional wgpu executor for the compiled scene boundary.
//!
//! The adapter owns GPU objects and never appears in Element, Layout, or
//! RenderTree code. It intentionally starts with a small WGSL quad pipeline;
//! paths and resource commands remain represented in CompiledScene and can be
//! added without changing the CPU contracts.

use super::{BatchKind, CompiledScene, QuadInstance, RendererCapabilities};
use crate::core::{DpiScale, Error, ImageHandle, Result, Size};
use crate::media::{DecodedImage, ImageTextureUploader};
use std::collections::VecDeque;
use std::sync::Arc;

const QUAD_SHADER: &str = r#"
struct Viewport { size: vec2<f32> }
@group(0) @binding(0) var<uniform> viewport: Viewport;
struct Instance { rect: vec4<f32>, radii: vec4<f32>, color: vec4<f32>, opacity: f32 }
struct VertexOutput { @builtin(position) position: vec4<f32>, @location(2) color: vec4<f32>, @location(3) opacity: f32 }
@vertex fn vs(@builtin(vertex_index) vertex: u32, @location(0) rect: vec4<f32>, @location(1) radii: vec4<f32>, @location(2) color: vec4<f32>, @location(3) opacity: f32) -> VertexOutput {
    let corners = array<vec2<f32>, 6>(vec2(0.0,0.0), vec2(1.0,0.0), vec2(1.0,1.0), vec2(0.0,0.0), vec2(1.0,1.0), vec2(0.0,1.0));
    let p = rect.xy + corners[vertex] * rect.zw;
    let ndc = vec2(p.x / viewport.size.x * 2.0 - 1.0, 1.0 - p.y / viewport.size.y * 2.0);
    return VertexOutput(vec4(ndc, 0.0, 1.0), color, opacity);
}
@fragment fn fs(@location(2) color: vec4<f32>, @location(3) opacity: f32) -> @location(0) vec4<f32> { return vec4(color.rgb, color.a * opacity); }
@vertex fn path_vs(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>, @location(2) opacity: f32) -> VertexOutput {
    let ndc = vec2(position.x / viewport.size.x * 2.0 - 1.0, 1.0 - position.y / viewport.size.y * 2.0);
    return VertexOutput(vec4(ndc, 0.0, 1.0), color, opacity);
}
@fragment fn path_fs(@location(2) color: vec4<f32>, @location(3) opacity: f32) -> @location(0) vec4<f32> { return vec4(color.rgb, color.a * opacity); }
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureUpload {
    pub handle: ImageHandle,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    /// The uploaded object remains alive until this receipt is dropped.
    pub texture: Arc<wgpu::Texture>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeferredResource {
    pub submission: u64,
    pub bytes: u64,
    pub label: Arc<str>,
}

#[derive(Clone, Debug, Default)]
pub struct DeferredResourceQueue {
    pending: VecDeque<DeferredResource>,
    released_bytes: u64,
}

impl DeferredResourceQueue {
    pub fn defer(&mut self, submission: u64, bytes: u64, label: impl Into<Arc<str>>) {
        self.pending.push_back(DeferredResource {
            submission,
            bytes,
            label: label.into(),
        });
    }
    pub fn collect_completed(&mut self, completed_submission: u64) -> u64 {
        let mut released: u64 = 0;
        while self
            .pending
            .front()
            .is_some_and(|resource| resource.submission <= completed_submission)
        {
            if let Some(resource) = self.pending.pop_front() {
                released = released.saturating_add(resource.bytes);
            }
        }
        self.released_bytes = self.released_bytes.saturating_add(released);
        released
    }
    pub fn pending(&self) -> impl ExactSizeIterator<Item = &DeferredResource> {
        self.pending.iter()
    }
    pub fn released_bytes(&self) -> u64 {
        self.released_bytes
    }
}

pub struct WgpuRenderer<'window> {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'window>>,
    config: Option<wgpu::SurfaceConfiguration>,
    pipeline: wgpu::RenderPipeline,
    path_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    viewport_buffer: wgpu::Buffer,
    physical_size: (u32, u32),
    dpi_scale: DpiScale,
    submission: u64,
    deferred: DeferredResourceQueue,
}

impl<'window> WgpuRenderer<'window> {
    pub async fn new_headless() -> Result<WgpuRenderer<'static>> {
        WgpuRenderer::<'static>::initialize_with_instance(
            None,
            make_instance(),
            Size::new(1.0, 1.0),
            DpiScale::ONE,
        )
        .await
    }

    pub async fn new_for_window(
        target: impl Into<wgpu::SurfaceTarget<'window>>,
        logical_size: Size,
        dpi_scale: DpiScale,
    ) -> Result<Self> {
        let instance = make_instance();
        let surface = instance
            .create_surface(target)
            .map_err(|error| Error::platform("create_surface", error.to_string(), false))?;
        Self::initialize_with_instance(Some(surface), instance, logical_size, dpi_scale).await
    }

    async fn initialize_with_instance(
        surface: Option<wgpu::Surface<'window>>,
        instance: wgpu::Instance,
        logical_size: Size,
        dpi_scale: DpiScale,
    ) -> Result<Self> {
        logical_size.validate().map_err(Error::from)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: surface.as_ref(),
                force_fallback_adapter: true,
            })
            .await
            .map_err(|error| Error::platform("request_adapter", error.to_string(), true))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tgui device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|error| Error::platform("request_device", error.to_string(), true))?;
        let format = surface
            .as_ref()
            .and_then(|surface| surface.get_capabilities(&adapter).formats.first().copied())
            .unwrap_or(wgpu::TextureFormat::Rgba8UnormSrgb);
        let (pipeline, path_pipeline, bind_group, viewport_buffer) =
            create_pipeline(&device, format);
        let mut renderer = Self {
            instance,
            adapter,
            device,
            queue,
            surface,
            config: None,
            pipeline,
            path_pipeline,
            bind_group,
            viewport_buffer,
            physical_size: (1, 1),
            dpi_scale,
            submission: 0,
            deferred: DeferredResourceQueue::default(),
        };
        renderer.resize(logical_size, dpi_scale)?;
        Ok(renderer)
    }

    pub fn capabilities(&self) -> RendererCapabilities {
        RendererCapabilities {
            generation: self.adapter.get_info().vendor as u64
                ^ u64::from(self.adapter.get_info().device),
            supports_paths: true,
            supports_native_surface: false,
            supports_backdrop: false,
            max_texture_dimension_2d: self.device.limits().max_texture_dimension_2d,
        }
    }

    pub fn dpi_scale(&self) -> DpiScale {
        self.dpi_scale
    }
    pub fn physical_size(&self) -> (u32, u32) {
        self.physical_size
    }
    pub fn deferred_resources(&self) -> &DeferredResourceQueue {
        &self.deferred
    }

    pub fn resize(&mut self, logical_size: Size, dpi_scale: DpiScale) -> Result<()> {
        logical_size.validate().map_err(Error::from)?;
        let width = dpi_scale
            .logical_to_physical(logical_size.width)
            .map_err(Error::from)?
            .max(1);
        let height = dpi_scale
            .logical_to_physical(logical_size.height)
            .map_err(Error::from)?
            .max(1);
        self.physical_size = (width, height);
        self.dpi_scale = dpi_scale;
        self.queue.write_buffer(
            &self.viewport_buffer,
            0,
            bytemuck::cast_slice(&[logical_size.width.max(1.0), logical_size.height.max(1.0)]),
        );
        if let (Some(surface), Some(config)) = (&self.surface, &mut self.config) {
            config.width = width;
            config.height = height;
            surface.configure(&self.device, config);
        }
        Ok(())
    }

    pub fn configure_surface(&mut self) -> Result<()> {
        let Some(surface) = &self.surface else {
            return Err(Error::platform(
                "configure_surface",
                "renderer has no surface",
                false,
            ));
        };
        let capabilities = surface.get_capabilities(&self.adapter);
        let format = capabilities.formats.first().copied().ok_or_else(|| {
            Error::platform("surface_format", "surface exposes no formats", false)
        })?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: self.physical_size.0,
            height: self.physical_size.1,
            present_mode: capabilities
                .present_modes
                .first()
                .copied()
                .unwrap_or(wgpu::PresentMode::Fifo),
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        let (pipeline, path_pipeline, bind_group, viewport_buffer) =
            create_pipeline(&self.device, format);
        self.pipeline = pipeline;
        self.path_pipeline = path_pipeline;
        self.bind_group = bind_group;
        self.viewport_buffer = viewport_buffer;
        surface.configure(&self.device, &config);
        self.config = Some(config);
        Ok(())
    }

    pub fn render_to_view(
        &mut self,
        scene: &CompiledScene,
        view: &wgpu::TextureView,
    ) -> Result<wgpu::SubmissionIndex> {
        self.queue.write_buffer(
            &self.viewport_buffer,
            0,
            bytemuck::cast_slice(&[
                self.dpi_scale.physical_to_logical(self.physical_size.0),
                self.dpi_scale.physical_to_logical(self.physical_size.1),
            ]),
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tgui frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            for render_pass in &scene.passes {
                for batch in &render_pass.batches {
                    if let Some(clip) = batch.clip {
                        let x = self
                            .dpi_scale
                            .logical_to_physical(clip.origin.x.max(0.0))
                            .map_err(Error::from)?;
                        let y = self
                            .dpi_scale
                            .logical_to_physical(clip.origin.y.max(0.0))
                            .map_err(Error::from)?;
                        let width = self
                            .dpi_scale
                            .logical_to_physical(clip.size.width)
                            .map_err(Error::from)?
                            .min(self.physical_size.0.saturating_sub(x));
                        let height = self
                            .dpi_scale
                            .logical_to_physical(clip.size.height)
                            .map_err(Error::from)?
                            .min(self.physical_size.1.saturating_sub(y));
                        if width == 0 || height == 0 {
                            continue;
                        }
                        pass.set_scissor_rect(x, y, width, height);
                    } else {
                        pass.set_scissor_rect(0, 0, self.physical_size.0, self.physical_size.1);
                    }
                    match batch.kind {
                        BatchKind::Quad { instances } => {
                            pass.set_pipeline(&self.pipeline);
                            let bytes = scene.quad_instances[instances.start..instances.end()]
                                .iter()
                                .flat_map(gpu_quad_bytes)
                                .collect::<Vec<_>>();
                            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some("tgui quad instances"),
                                size: bytes.len().max(1) as u64,
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                                mapped_at_creation: false,
                            });
                            self.queue.write_buffer(&buffer, 0, &bytes);
                            pass.set_vertex_buffer(0, buffer.slice(..));
                            pass.draw(0..6, 0..u32::try_from(instances.count).unwrap_or(u32::MAX));
                        }
                        BatchKind::Path { vertices, indices } => {
                            pass.set_pipeline(&self.path_pipeline);
                            let vertex_bytes = scene.path_vertices[vertices.start..vertices.end()]
                                .iter()
                                .flat_map(gpu_path_vertex_bytes)
                                .collect::<Vec<_>>();
                            let index_bytes = scene.path_indices[indices.start..indices.end()]
                                .iter()
                                .flat_map(|index| index.to_le_bytes())
                                .collect::<Vec<_>>();
                            let vertex_buffer =
                                self.device.create_buffer(&wgpu::BufferDescriptor {
                                    label: Some("tgui path vertices"),
                                    size: vertex_bytes.len().max(1) as u64,
                                    usage: wgpu::BufferUsages::VERTEX
                                        | wgpu::BufferUsages::COPY_DST,
                                    mapped_at_creation: false,
                                });
                            let index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some("tgui path indices"),
                                size: index_bytes.len().max(1) as u64,
                                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                                mapped_at_creation: false,
                            });
                            self.queue.write_buffer(&vertex_buffer, 0, &vertex_bytes);
                            self.queue.write_buffer(&index_buffer, 0, &index_bytes);
                            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                            pass.set_index_buffer(
                                index_buffer.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            pass.draw_indexed(
                                0..u32::try_from(indices.count).unwrap_or(u32::MAX),
                                0,
                                0..1,
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        let submission = self.queue.submit(Some(encoder.finish()));
        self.submission = self.submission.saturating_add(1);
        Ok(submission)
    }

    pub fn render_surface(&mut self, scene: &CompiledScene) -> Result<()> {
        let surface = self
            .surface
            .as_ref()
            .ok_or_else(|| Error::platform("render_surface", "renderer has no surface", false))?;
        let frame = surface
            .get_current_texture()
            .map_err(|error| Error::platform("acquire_surface", format!("{error:?}"), true))?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.render_to_view(scene, &view)?;
        frame.present();
        Ok(())
    }

    pub fn upload_rgba8(
        &self,
        handle: ImageHandle,
        width: u32,
        height: u32,
        bytes: &[u8],
    ) -> Result<TextureUpload> {
        let texture = Arc::new(self.create_rgba8_texture(width, height, bytes)?);
        Ok(TextureUpload {
            handle,
            width,
            height,
            bytes: u64::from(width)
                .saturating_mul(u64::from(height))
                .saturating_mul(4),
            texture,
        })
    }

    /// Creates an uploaded texture for ownership by a `GpuTextureCache`.
    pub fn create_rgba8_texture(
        &self,
        width: u32,
        height: u32,
        bytes: &[u8],
    ) -> Result<wgpu::Texture> {
        let expected = u64::from(width)
            .saturating_mul(u64::from(height))
            .saturating_mul(4);
        if width == 0 || height == 0 {
            return Err(Error::resource(
                None,
                "RGBA upload dimensions must be non-zero",
                true,
            ));
        }
        let max_dimension = self.device.limits().max_texture_dimension_2d;
        if width > max_dimension || height > max_dimension {
            return Err(Error::resource(
                None,
                "RGBA upload exceeds the renderer's maximum texture dimension",
                true,
            ));
        }
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != expected {
            return Err(Error::resource(
                None,
                "RGBA upload byte count does not match dimensions",
                true,
            ));
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui image"),
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
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width.saturating_mul(4)),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        Ok(texture)
    }

    pub fn defer_resource(&mut self, bytes: u64, label: impl Into<Arc<str>>) {
        self.deferred.defer(self.submission, bytes, label);
    }
    pub fn collect_completed(&mut self, completed_submission: u64) -> u64 {
        self.deferred.collect_completed(completed_submission)
    }

    pub async fn recover_device(&mut self) -> Result<()> {
        let adapter = self
            .instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: self.surface.as_ref(),
                force_fallback_adapter: true,
            })
            .await
            .map_err(|error| Error::platform("recover_adapter", error.to_string(), true))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tgui recovered device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|error| Error::platform("recover_device", error.to_string(), true))?;
        let format = self
            .config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8UnormSrgb, |config| config.format);
        let (pipeline, path_pipeline, bind_group, viewport_buffer) =
            create_pipeline(&device, format);
        self.adapter = adapter;
        self.device = device;
        self.queue = queue;
        self.pipeline = pipeline;
        self.path_pipeline = path_pipeline;
        self.bind_group = bind_group;
        self.viewport_buffer = viewport_buffer;
        if self.surface.is_some() {
            self.config = None;
            self.configure_surface()?;
        }
        Ok(())
    }
}

impl ImageTextureUploader for WgpuRenderer<'_> {
    type Texture = wgpu::Texture;

    fn upload_image(
        &mut self,
        _handle: ImageHandle,
        image: &DecodedImage,
    ) -> Result<Self::Texture> {
        let size = image.size();
        self.create_rgba8_texture(size.width, size.height, image.rgba8())
    }
}

fn make_instance() -> wgpu::Instance {
    wgpu::Instance::new(&wgpu::InstanceDescriptor::default())
}

fn create_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> (
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::BindGroup,
    wgpu::Buffer,
) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tgui quad shader"),
        source: wgpu::ShaderSource::Wgsl(QUAD_SHADER.into()),
    });
    let viewport_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("tgui viewport"),
        size: 8,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("tgui uniforms"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tgui uniform bind group"),
        layout: &bind_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: viewport_buffer.as_entire_binding(),
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tgui pipeline layout"),
        bind_group_layouts: &[&bind_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor { label: Some("tgui quad pipeline"), layout: Some(&pipeline_layout), vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: wgpu::PipelineCompilationOptions::default(), buffers: &[wgpu::VertexBufferLayout { array_stride: 52, step_mode: wgpu::VertexStepMode::Instance, attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32] }] }, fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), compilation_options: wgpu::PipelineCompilationOptions::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], }), primitive: wgpu::PrimitiveState::default(), depth_stencil: None, multisample: wgpu::MultisampleState::default(), multiview: None, cache: None });
    let path_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tgui path pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("path_vs"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 28,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4, 2 => Float32],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("path_fs"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    (pipeline, path_pipeline, bind_group, viewport_buffer)
}

fn gpu_quad_bytes(instance: &QuadInstance) -> impl IntoIterator<Item = u8> {
    let mut bytes = Vec::with_capacity(52);
    for value in instance
        .rect
        .iter()
        .chain(instance.radii.iter())
        .chain(instance.color.iter())
        .copied()
        .chain([instance.opacity])
    {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn gpu_path_vertex_bytes(vertex: &super::compiler::PathVertex) -> impl IntoIterator<Item = u8> {
    let mut bytes = Vec::with_capacity(28);
    for value in vertex
        .position
        .iter()
        .chain(vertex.color.iter())
        .copied()
        .chain([vertex.opacity])
    {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Clip, Color, Point, Rect, SceneRevision};
    use crate::render::{
        CompileContext, FillRule, Paint, PaintCommand, Path, PathSegment, RenderCompiler,
    };

    #[test]
    fn deferred_resources_wait_for_their_submission() {
        let mut queue = DeferredResourceQueue::default();
        queue.defer(2, 16, "instance buffer");
        assert_eq!(queue.collect_completed(1), 0);
        assert_eq!(queue.pending().len(), 1);
        assert_eq!(queue.collect_completed(2), 16);
        assert_eq!(queue.pending().len(), 0);
    }

    #[test]
    fn headless_device_covers_resize_upload_submit_and_recovery_when_available() {
        let Ok(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
            return;
        };
        renderer
            .resize(Size::new(20.0, 10.0), DpiScale::new(1.5).unwrap())
            .unwrap();
        assert_eq!(renderer.physical_size(), (30, 15));
        let upload = renderer
            .upload_rgba8(ImageHandle::from_parts(1, 1), 1, 1, &[255, 0, 0, 128])
            .unwrap();
        assert_eq!(upload.bytes, 4);

        let path = Path::new(
            [
                PathSegment::MoveTo(Point::new(1.0, 1.0)),
                PathSegment::LineTo(Point::new(8.0, 1.0)),
                PathSegment::LineTo(Point::new(4.0, 4.0)),
                PathSegment::Close,
            ],
            FillRule::NonZero,
        )
        .unwrap();
        let commands = [
            PaintCommand::PushClip(Clip::Rect(Rect::from_xywh(0.0, 0.0, 10.0, 5.0))),
            PaintCommand::FillRect {
                rect: Rect::from_xywh(0.0, 0.0, 10.0, 5.0),
                color: Color::rgba8(255, 0, 0, 128),
            },
            PaintCommand::FillPath {
                path,
                paint: Paint::solid(Color::BLACK).with_opacity(0.5),
            },
            PaintCommand::PopClip,
        ];
        let context = CompileContext::new(renderer.capabilities(), renderer.dpi_scale())
            .with_scene_revision(SceneRevision::new(1));
        let scene = RenderCompiler::default()
            .compile(&commands, &context)
            .unwrap();
        let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui test target"),
            size: wgpu::Extent3d {
                width: 30,
                height: 15,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        renderer.render_to_view(&scene, &view).unwrap();
        pollster::block_on(renderer.recover_device()).unwrap();
    }
}
