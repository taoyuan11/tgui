use std::collections::HashSet;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
#[cfg(all(target_env = "ohos", feature = "ohos"))]
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use wgpu::util::DeviceExt;

use crate::foundation::color::Color as TguiColor;
use crate::foundation::error::TguiError;
use crate::media::TextureFrame;
use crate::platform::backend::window::Window;
use crate::platform::dpi::PhysicalSize;
use crate::text::font::{FontCatalog, FontWeight};
use crate::ui::unit::Dp;
use crate::ui::widget::{
    BackdropBlurPrimitive, BrushPrimitiveData, MeshVertex as SceneMeshVertex, Rect, RenderCommand,
    RenderPrimitive, ScenePrimitives, TextPrimitive,
};

pub enum RenderStatus {
    Rendered,
    ReconfigureSurface,
    SkipFrame,
}

pub struct Renderer {
    window: Arc<dyn Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    rect_pipeline: wgpu::RenderPipeline,
    brush_pipeline: wgpu::RenderPipeline,
    mesh_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    backdrop_blur_pipeline: wgpu::RenderPipeline,
    backdrop_composite_pipeline: wgpu::RenderPipeline,
    text_bind_group_layout: wgpu::BindGroupLayout,
    backdrop_blur_bind_group_layout: wgpu::BindGroupLayout,
    backdrop_composite_bind_group_layout: wgpu::BindGroupLayout,
    text_sampler: wgpu::Sampler,
    size: PhysicalSize<u32>,
    scale_factor: f32,
    msaa_sample_count: u32,
    msaa_target: Option<MultisampleTarget>,
    scene_target: Option<OffscreenTarget>,
    blur_target: Option<OffscreenTarget>,
    blur_scratch_target: Option<OffscreenTarget>,
    clear_color: TguiColor,
    text_system: TextSystem,
    text_cache: Vec<TextCacheEntry>,
    texture_cache: Vec<TextureCacheEntry>,
}

struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

struct MultisampleTarget {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
}

struct OffscreenTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct PreparedRect {
    clip_rect: Option<Rect>,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
}

struct PreparedBrush {
    clip_rect: Option<Rect>,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
}

struct PreparedMesh {
    clip_rect: Option<Rect>,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
}

struct PreparedSprite {
    bind_group: wgpu::BindGroup,
    clip_rect: Option<Rect>,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
}

struct PreparedBackdropBlur {
    primitive: BackdropBlurPrimitive,
    composite_buffer: wgpu::Buffer,
    composite_vertex_count: u32,
}

enum PreparedCommand {
    BackdropBlur(PreparedBackdropBlur),
    Rect(PreparedRect),
    Brush(PreparedBrush),
    Mesh(PreparedMesh),
    Sprite(PreparedSprite),
}

struct PreparedCommands(Vec<PreparedCommand>);

struct TextCacheEntry {
    key: TextCacheKey,
    bind_group: wgpu::BindGroup,
    _texture: wgpu::Texture,
}

struct TextureCacheEntry {
    id: u64,
    revision: u64,
    bind_group: wgpu::BindGroup,
    _texture: wgpu::Texture,
}

#[derive(Clone, PartialEq, Eq)]
struct TextCacheKey {
    content: String,
    font_family: Option<String>,
    width: u32,
    height: u32,
    color: [u8; 4],
    font_size_bits: u32,
    line_height_bits: u32,
    letter_spacing_bits: u32,
    font_weight: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurUniform {
    direction: [f32; 2],
    texel_size: [f32; 2],
    radius: f32,
    _pad: f32,
}

impl Renderer {
    pub fn new(
        window: Arc<dyn Window>,
        clear_color: TguiColor,
        fonts: &FontCatalog,
    ) -> Result<Self, TguiError> {
        pollster::block_on(Self::new_async(window, clear_color, fonts))
    }

    async fn new_async(
        window: Arc<dyn Window>,
        clear_color: TguiColor,
        fonts: &FontCatalog,
    ) -> Result<Self, TguiError> {
        let size = window.surface_size();
        let instance = create_instance(clear_color);
        let surface = create_surface(&instance, window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: adapter_power_preference(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;
        let required_limits = required_device_limits(&adapter);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tgui-device"),
                required_features: wgpu::Features::empty(),
                required_limits,
                memory_hints: wgpu::MemoryHints::Performance,
                experimental_features: Default::default(),
                trace: Default::default(),
            })
            .await?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| caps.formats.first().copied())
            .ok_or(TguiError::NoSurfaceFormat)?;

        let alpha_mode = surface_alpha_mode(&caps.alpha_modes);
        let msaa_sample_count = surface_msaa_sample_count(&adapter, format);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_present_mode(&caps.present_modes),
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-rect-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader/rect.wgsl").into()),
        });
        let mesh_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-mesh-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader/mesh.wgsl").into()),
        });
        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-text-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader/text.wgsl").into()),
        });
        let brush_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-brush-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader/brush.wgsl").into()),
        });
        let backdrop_blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-backdrop-blur-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader/backdrop_blur.wgsl").into()),
        });
        let backdrop_composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-backdrop-composite-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader/backdrop_composite.wgsl").into()),
        });

        let rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tgui-rect-pipeline-layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tgui-rect-pipeline"),
            layout: Some(&rect_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &rect_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[RectVertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &rect_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let brush_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("tgui-brush-pipeline-layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });

        let brush_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tgui-brush-pipeline"),
            layout: Some(&brush_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &brush_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[BrushVertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &brush_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let mesh_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tgui-mesh-pipeline-layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tgui-mesh-pipeline"),
            layout: Some(&mesh_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &mesh_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[MeshVertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &mesh_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let text_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tgui-text-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let backdrop_blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tgui-backdrop-blur-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let backdrop_composite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tgui-backdrop-composite-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tgui-text-pipeline-layout"),
            bind_group_layouts: &[Some(&text_bind_group_layout)],
            immediate_size: 0,
        });

        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tgui-text-pipeline"),
            layout: Some(&text_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[TextVertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let backdrop_blur_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("tgui-backdrop-blur-pipeline-layout"),
                bind_group_layouts: &[Some(&backdrop_blur_bind_group_layout)],
                immediate_size: 0,
            });
        let backdrop_blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tgui-backdrop-blur-pipeline"),
            layout: Some(&backdrop_blur_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &backdrop_blur_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[TextVertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &backdrop_blur_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let backdrop_composite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("tgui-backdrop-composite-pipeline-layout"),
                bind_group_layouts: &[Some(&backdrop_composite_bind_group_layout)],
                immediate_size: 0,
            });
        let backdrop_composite_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("tgui-backdrop-composite-pipeline"),
                layout: Some(&backdrop_composite_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &backdrop_composite_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[CompositeVertex::layout()],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &backdrop_composite_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });

        let text_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tgui-text-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let mut font_system = FontSystem::new();
        let _ = fonts.configure_font_system(&mut font_system);
        let scale_factor = 1.0_f32.max(window.scale_factor() as f32);

        surface.configure(&device, &config);
        let msaa_target = create_multisample_target(&device, &config, msaa_sample_count);
        let scene_target = create_offscreen_target(&device, &config, "tgui-scene-target");
        let blur_target = create_offscreen_target(&device, &config, "tgui-blur-target");
        let blur_scratch_target =
            create_offscreen_target(&device, &config, "tgui-blur-scratch-target");

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            rect_pipeline,
            brush_pipeline,
            mesh_pipeline,
            text_pipeline,
            backdrop_blur_pipeline,
            backdrop_composite_pipeline,
            text_bind_group_layout,
            backdrop_blur_bind_group_layout,
            backdrop_composite_bind_group_layout,
            text_sampler,
            size,
            scale_factor,
            msaa_sample_count,
            msaa_target,
            scene_target,
            blur_target,
            blur_scratch_target,
            clear_color,
            text_system: TextSystem {
                font_system,
                swash_cache: SwashCache::new(),
            },
            text_cache: Vec::new(),
            texture_cache: Vec::new(),
        })
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>, scale_factor: f32) {
        if new_size.width == 0 || new_size.height == 0 {
            self.size = new_size;
            self.scale_factor = scale_factor.max(1.0 / 64.0);
            return;
        }

        self.size = new_size;
        self.scale_factor = scale_factor.max(1.0 / 64.0);
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        self.recreate_multisample_target();
        self.recreate_offscreen_targets();
    }

    pub fn render(&mut self, scene: &ScenePrimitives) -> Result<RenderStatus, TguiError> {
        if self.config.width == 0 || self.config.height == 0 {
            return Ok(RenderStatus::SkipFrame);
        }
        let (logical_width, logical_height) = self.logical_viewport_size();

        let active_texture_keys: HashSet<_> = scene
            .textures
            .iter()
            .map(|texture| (texture.texture.id(), texture.texture.revision()))
            .collect();
        self.texture_cache
            .retain(|entry| active_texture_keys.contains(&(entry.id, entry.revision)));

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                return Ok(RenderStatus::ReconfigureSurface);
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(RenderStatus::SkipFrame),
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let command_buffers = self.prepare_commands(
            &scene.commands,
            logical_width,
            logical_height,
            self.config.width as f32,
            self.config.height as f32,
            self.scale_factor,
        )?;
        let overlay_buffers = self.prepare_commands(
            &scene.overlay_commands,
            logical_width,
            logical_height,
            self.config.width as f32,
            self.config.height as f32,
            self.scale_factor,
        )?;
        let color_attachment_view = view.clone();
        let scene_view = self.scene_target_view()?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tgui-render-encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-scene-clear-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(surface_clear_color(self.clear_color)),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let _ = &mut pass;
        }

        self.execute_prepared_commands(&mut encoder, &command_buffers.0)?;
        self.execute_prepared_commands(&mut encoder, &overlay_buffers.0)?;
        let scene_view = self.scene_target_view()?;
        self.blit_scene_to_surface(
            &mut encoder,
            scene_view,
            &color_attachment_view,
            None,
        );

        self.queue.submit(Some(encoder.finish()));
        self.window.pre_present_notify();
        frame.present();

        Ok(RenderStatus::Rendered)
    }

    fn prepare_commands(
        &mut self,
        commands: &[RenderCommand],
        logical_width: f32,
        logical_height: f32,
        physical_width: f32,
        physical_height: f32,
        scale_factor: f32,
    ) -> Result<PreparedCommands, TguiError> {
        let mut prepared = Vec::new();

        for command in commands {
            match command {
                RenderCommand::BackdropBlur(primitive) => {
                    if primitive.rect.width <= Dp::ZERO || primitive.rect.height <= Dp::ZERO {
                        continue;
                    }
                    let vertices = CompositeVertex::quad(
                        primitive.rect,
                        logical_width,
                        logical_height,
                        primitive.corner_radius,
                    );
                    let composite_buffer =
                        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("tgui-backdrop-composite-vertices"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                    prepared.push(PreparedCommand::BackdropBlur(PreparedBackdropBlur {
                        primitive: *primitive,
                        composite_buffer,
                        composite_vertex_count: vertices.len() as u32,
                    }));
                }
                RenderCommand::Brush(primitive) => {
                    if primitive.rect.width <= Dp::ZERO || primitive.rect.height <= Dp::ZERO {
                        continue;
                    }
                    let Some(brush_data) =
                        BrushPrimitiveData::from_background_brush(&primitive.brush, 1.0)
                    else {
                        continue;
                    };
                    let vertices = BrushVertex::from_primitive(
                        primitive.rect,
                        primitive.corner_radius,
                        brush_data,
                        physical_width,
                        physical_height,
                        scale_factor,
                    );
                    let vertex_buffer =
                        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("tgui-brush-vertices"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                    prepared.push(PreparedCommand::Brush(PreparedBrush {
                        clip_rect: primitive.clip_rect,
                        vertex_buffer,
                        vertex_count: vertices.len() as u32,
                    }));
                }
                RenderCommand::Shape(primitive) => {
                    if primitive.rect.width <= Dp::ZERO || primitive.rect.height <= Dp::ZERO {
                        continue;
                    }
                    let vertices = RectVertex::from_primitive(
                        *primitive,
                        physical_width,
                        physical_height,
                        scale_factor,
                    );
                    let vertex_buffer =
                        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("tgui-rect-vertices"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                    prepared.push(PreparedCommand::Rect(PreparedRect {
                        clip_rect: primitive.clip_rect,
                        vertex_buffer,
                        vertex_count: vertices.len() as u32,
                    }));
                }
                RenderCommand::Mesh(primitive) => {
                    if primitive.vertices.is_empty() {
                        continue;
                    }
                    let vertices: Vec<_> = primitive
                        .vertices
                        .iter()
                        .copied()
                        .map(|vertex| MeshVertex::from_scene_vertex(vertex, logical_width, logical_height))
                        .collect();
                    let vertex_buffer =
                        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("tgui-mesh-vertices"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                    prepared.push(PreparedCommand::Mesh(PreparedMesh {
                        clip_rect: primitive.clip_rect,
                        vertex_buffer,
                        vertex_count: vertices.len() as u32,
                    }));
                }
                RenderCommand::Texture(texture) => {
                    if let Some(bind_group) = self.texture_bind_group_for(&texture.texture)? {
                        let vertices = TextVertex::quad(
                            texture.frame,
                            logical_width,
                            logical_height,
                            texture.corner_radius,
                            physical_width,
                            physical_height,
                            scale_factor,
                        );
                        let vertex_buffer =
                            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("tgui-sprite-vertices"),
                                contents: bytemuck::cast_slice(&vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            });
                        prepared.push(PreparedCommand::Sprite(PreparedSprite {
                            bind_group,
                            clip_rect: texture.clip_rect,
                            vertex_buffer,
                            vertex_count: vertices.len() as u32,
                        }));
                    }
                }
                RenderCommand::Text(text) => {
                    if let Some(bind_group) = self.text_bind_group_for(text)? {
                        let vertices = TextVertex::quad(
                            text.frame,
                            logical_width,
                            logical_height,
                            0.0,
                            physical_width,
                            physical_height,
                            scale_factor,
                        );
                        let vertex_buffer =
                            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("tgui-text-vertices"),
                                contents: bytemuck::cast_slice(&vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            });
                        prepared.push(PreparedCommand::Sprite(PreparedSprite {
                            bind_group,
                            clip_rect: text.clip_rect,
                            vertex_buffer,
                            vertex_count: vertices.len() as u32,
                        }));
                    }
                }
            }
        }

        Ok(PreparedCommands(prepared))
    }

    fn apply_scissor<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        clip_rect: Option<Rect>,
    ) -> bool {
        let Some((x, y, width, height)) = self.scissor_rect(clip_rect) else {
            return false;
        };
        pass.set_scissor_rect(x, y, width, height);
        true
    }

    fn scissor_rect(&self, clip_rect: Option<Rect>) -> Option<(u32, u32, u32, u32)> {
        let (logical_width, logical_height) = self.logical_viewport_size();
        let clip_rect = clip_rect.unwrap_or(Rect::new(0.0, 0.0, logical_width, logical_height));
        let x = self.logical_to_physical(clip_rect.x.max(0.0).get()).floor() as u32;
        let y = self.logical_to_physical(clip_rect.y.max(0.0).get()).floor() as u32;
        let right = clip_rect.right().min(logical_width);
        let bottom = clip_rect.bottom().min(logical_height);
        let right = self.logical_to_physical(right.get()).ceil().max(x as f32) as u32;
        let bottom = self.logical_to_physical(bottom.get()).ceil().max(y as f32) as u32;
        let width = right.saturating_sub(x);
        let height = bottom.saturating_sub(y);
        (width > 0 && height > 0).then_some((x, y, width, height))
    }

    fn logical_viewport_size(&self) -> (f32, f32) {
        (
            self.config.width as f32 / self.scale_factor,
            self.config.height as f32 / self.scale_factor,
        )
    }

    fn logical_to_physical(&self, value: f32) -> f32 {
        value * self.scale_factor
    }

    pub fn set_clear_color(&mut self, clear_color: TguiColor) {
        self.clear_color = clear_color;
    }

    pub fn reconfigure(&mut self) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }

        self.surface.configure(&self.device, &self.config);
        self.recreate_multisample_target();
        self.recreate_offscreen_targets();
    }

    fn recreate_multisample_target(&mut self) {
        self.msaa_target =
            create_multisample_target(&self.device, &self.config, self.msaa_sample_count);
    }

    fn recreate_offscreen_targets(&mut self) {
        self.scene_target = create_offscreen_target(&self.device, &self.config, "tgui-scene-target");
        self.blur_target = create_offscreen_target(&self.device, &self.config, "tgui-blur-target");
        self.blur_scratch_target =
            create_offscreen_target(&self.device, &self.config, "tgui-blur-scratch-target");
    }

    fn execute_prepared_commands(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        commands: &[PreparedCommand],
    ) -> Result<(), TguiError> {
        let mut index = 0;
        while index < commands.len() {
            if let PreparedCommand::BackdropBlur(blur) = &commands[index] {
                self.apply_backdrop_blur(encoder, blur)?;
                index += 1;
                continue;
            }

            let scene_view = self.scene_target_view()?;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-scene-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            while index < commands.len() {
                match &commands[index] {
                    PreparedCommand::BackdropBlur(_) => break,
                    PreparedCommand::Rect(batch) => {
                        if self.apply_scissor(&mut pass, batch.clip_rect) {
                            pass.set_pipeline(&self.rect_pipeline);
                            pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                            pass.draw(0..batch.vertex_count, 0..1);
                        }
                    }
                    PreparedCommand::Brush(batch) => {
                        if self.apply_scissor(&mut pass, batch.clip_rect) {
                            pass.set_pipeline(&self.brush_pipeline);
                            pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                            pass.draw(0..batch.vertex_count, 0..1);
                        }
                    }
                    PreparedCommand::Mesh(batch) => {
                        if self.apply_scissor(&mut pass, batch.clip_rect) {
                            pass.set_pipeline(&self.mesh_pipeline);
                            pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                            pass.draw(0..batch.vertex_count, 0..1);
                        }
                    }
                    PreparedCommand::Sprite(batch) => {
                        if self.apply_scissor(&mut pass, batch.clip_rect) {
                            pass.set_pipeline(&self.text_pipeline);
                            pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                            pass.set_bind_group(0, &batch.bind_group, &[]);
                            pass.draw(0..batch.vertex_count, 0..1);
                        }
                    }
                }
                index += 1;
            }
        }

        Ok(())
    }

    fn apply_backdrop_blur(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        blur: &PreparedBackdropBlur,
    ) -> Result<(), TguiError> {
        let scene_target = self
            .scene_target
            .as_ref()
            .ok_or_else(|| TguiError::TextRender("scene target unavailable".into()))?;
        let blur_target = self
            .blur_target
            .as_ref()
            .ok_or_else(|| TguiError::TextRender("blur target unavailable".into()))?;
        let blur_scratch_target = self
            .blur_scratch_target
            .as_ref()
            .ok_or_else(|| TguiError::TextRender("blur scratch target unavailable".into()))?;

        let scene_snapshot = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-scene-snapshot"),
            size: wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &scene_target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &scene_snapshot,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );
        let scene_snapshot_view =
            scene_snapshot.create_view(&wgpu::TextureViewDescriptor::default());

        let full_screen = TextVertex::quad(
            Rect::new(
                0.0,
                0.0,
                self.config.width as f32 / self.scale_factor,
                self.config.height as f32 / self.scale_factor,
            ),
            self.config.width as f32 / self.scale_factor,
            self.config.height as f32 / self.scale_factor,
            0.0,
            self.config.width as f32,
            self.config.height as f32,
            1.0,
        );
        let full_screen_buffer =
            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tgui-backdrop-fullscreen-vertices"),
                contents: bytemuck::cast_slice(&full_screen),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let horizontal_uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tgui-backdrop-horizontal-uniform"),
            contents: bytemuck::bytes_of(&BlurUniform {
                direction: [1.0, 0.0],
                texel_size: [
                    1.0 / self.config.width.max(1) as f32,
                    1.0 / self.config.height.max(1) as f32,
                ],
                radius: blur.primitive.blur_radius.max(0.0),
                _pad: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let vertical_uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tgui-backdrop-vertical-uniform"),
            contents: bytemuck::bytes_of(&BlurUniform {
                direction: [0.0, 1.0],
                texel_size: [
                    1.0 / self.config.width.max(1) as f32,
                    1.0 / self.config.height.max(1) as f32,
                ],
                radius: blur.primitive.blur_radius.max(0.0),
                _pad: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let horizontal_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-backdrop-horizontal-bind-group"),
            layout: &self.backdrop_blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene_snapshot_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: horizontal_uniform.as_entire_binding(),
                },
            ],
        });
        let vertical_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-backdrop-vertical-bind-group"),
            layout: &self.backdrop_blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&blur_scratch_target.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: vertical_uniform.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-backdrop-horizontal-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &blur_scratch_target.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.backdrop_blur_pipeline);
            pass.set_vertex_buffer(0, full_screen_buffer.slice(..));
            pass.set_bind_group(0, &horizontal_bind_group, &[]);
            pass.draw(0..full_screen.len() as u32, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-backdrop-vertical-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &blur_target.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.backdrop_blur_pipeline);
            pass.set_vertex_buffer(0, full_screen_buffer.slice(..));
            pass.set_bind_group(0, &vertical_bind_group, &[]);
            pass.draw(0..full_screen.len() as u32, 0..1);
        }

        let composite_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-backdrop-composite-bind-group"),
            layout: &self.backdrop_composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&blur_target.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&scene_snapshot_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-backdrop-composite-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &scene_target.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if self.apply_scissor(&mut pass, blur.primitive.clip_rect) {
                pass.set_pipeline(&self.backdrop_composite_pipeline);
                pass.set_vertex_buffer(0, blur.composite_buffer.slice(..));
                pass.set_bind_group(0, &composite_bind_group, &[]);
                pass.draw(0..blur.composite_vertex_count, 0..1);
            }
        }

        Ok(())
    }

    fn blit_scene_to_surface(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scene_view: &wgpu::TextureView,
        color_attachment_view: &wgpu::TextureView,
        resolve_target: Option<&wgpu::TextureView>,
    ) {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-scene-present-bind-group"),
            layout: &self.text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
            ],
        });
        let quad = TextVertex::quad(
            Rect::new(
                0.0,
                0.0,
                self.config.width as f32 / self.scale_factor,
                self.config.height as f32 / self.scale_factor,
            ),
            self.config.width as f32 / self.scale_factor,
            self.config.height as f32 / self.scale_factor,
            0.0,
            self.config.width as f32,
            self.config.height as f32,
            1.0,
        );
        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tgui-scene-present-vertices"),
            contents: bytemuck::cast_slice(&quad),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tgui-present-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_attachment_view,
                resolve_target,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(surface_clear_color(self.clear_color)),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.text_pipeline);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..quad.len() as u32, 0..1);
    }

    fn scene_target_view(&self) -> Result<&wgpu::TextureView, TguiError> {
        self.scene_target
            .as_ref()
            .map(|target| &target.view)
            .ok_or_else(|| TguiError::TextRender("scene target unavailable".into()))
    }

    fn text_cache_key(&self, text: &TextPrimitive) -> Option<TextCacheKey> {
        let width = self
            .logical_to_physical(text.frame.width.get())
            .ceil()
            .max(1.0) as u32;
        let height = self
            .logical_to_physical(text.frame.height.get())
            .ceil()
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
            font_size_bits: font_size.to_bits(),
            line_height_bits: line_height.to_bits(),
            letter_spacing_bits: letter_spacing.to_bits(),
            font_weight: text.font_weight.to_raw(),
        })
    }

    fn text_bind_group_for(
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

    fn rasterize_text(
        &mut self,
        text: &TextPrimitive,
    ) -> Result<Option<(wgpu::Texture, wgpu::BindGroup)>, TguiError> {
        let width = self
            .logical_to_physical(text.frame.width.get())
            .ceil()
            .max(1.0) as u32;
        let height = self
            .logical_to_physical(text.frame.height.get())
            .ceil()
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
        buffer.draw(
            &mut self.text_system.font_system,
            &mut self.text_system.swash_cache,
            color_to_text(text.color),
            |x, y, w, h, color| {
                let rgba = color.as_rgba();
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

    fn texture_bind_group_for(
        &mut self,
        texture_frame: &TextureFrame,
    ) -> Result<Option<wgpu::BindGroup>, TguiError> {
        if let Some(entry) = self.texture_cache.iter().find(|entry| {
            entry.id == texture_frame.id() && entry.revision == texture_frame.revision()
        }) {
            return Ok(Some(entry.bind_group.clone()));
        }

        let (width, height) = texture_frame.size();
        if width == 0 || height == 0 {
            return Ok(None);
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

        self.texture_cache.push(TextureCacheEntry {
            id: texture_frame.id(),
            revision: texture_frame.revision(),
            bind_group: bind_group.clone(),
            _texture: texture,
        });

        Ok(Some(bind_group))
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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RectVertex {
    position: [f32; 2],
    color: [f32; 4],
    local_position: [f32; 2],
    rect_size: [f32; 2],
    corner_radius: f32,
    stroke_width: f32,
}

impl RectVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: std::mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: (std::mem::size_of::<[f32; 2]>() + std::mem::size_of::<[f32; 4]>())
                        as u64,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: (std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 4]>()
                        + std::mem::size_of::<[f32; 2]>()) as u64,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: (std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 4]>()
                        + std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 2]>()) as u64,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: (std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 4]>()
                        + std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<f32>()) as u64,
                    shader_location: 5,
                },
            ],
        }
    }

    fn from_primitive(
        primitive: RenderPrimitive,
        physical_width: f32,
        physical_height: f32,
        scale_factor: f32,
    ) -> [Self; 6] {
        let rect_x = primitive.rect.x.get() * scale_factor;
        let rect_y = primitive.rect.y.get() * scale_factor;
        let rect_width = primitive.rect.width.max(0.0).get() * scale_factor;
        let rect_height = primitive.rect.height.max(0.0).get() * scale_factor;
        let x0 = rect_x / physical_width * 2.0 - 1.0;
        let x1 = (rect_x + rect_width) / physical_width * 2.0 - 1.0;
        let y0 = 1.0 - rect_y / physical_height * 2.0;
        let y1 = 1.0 - (rect_y + rect_height) / physical_height * 2.0;
        let color = primitive.color.to_linear_rgba_f32();
        let rect_size = [rect_width, rect_height];
        let radius = (primitive.corner_radius.max(0.0) * scale_factor)
            .min(rect_size[0] * 0.5)
            .min(rect_size[1] * 0.5);
        let stroke_width = (primitive.stroke_width.max(0.0) * scale_factor)
            .min(rect_size[0] * 0.5)
            .min(rect_size[1] * 0.5);

        [
            Self {
                position: [x0, y0],
                color,
                local_position: [0.0, 0.0],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
            Self {
                position: [x1, y0],
                color,
                local_position: [rect_size[0], 0.0],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
            Self {
                position: [x1, y1],
                color,
                local_position: [rect_size[0], rect_size[1]],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
            Self {
                position: [x0, y0],
                color,
                local_position: [0.0, 0.0],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
            Self {
                position: [x1, y1],
                color,
                local_position: [rect_size[0], rect_size[1]],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
            Self {
                position: [x0, y1],
                color,
                local_position: [0.0, rect_size[1]],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
        ]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MeshVertex {
    position: [f32; 2],
    local_position: [f32; 2],
    brush_meta: [f32; 4],
    gradient_data0: [f32; 4],
    gradient_data1: [f32; 4],
    stop_offsets0: [f32; 4],
    stop_offsets1: [f32; 4],
    stop_color0: [f32; 4],
    stop_color1: [f32; 4],
    stop_color2: [f32; 4],
    stop_color3: [f32; 4],
    stop_color4: [f32; 4],
    stop_color5: [f32; 4],
    stop_color6: [f32; 4],
    stop_color7: [f32; 4],
}

impl MeshVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: std::mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2) as u64,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2 + std::mem::size_of::<[f32; 4]>())
                        as u64,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2
                        + std::mem::size_of::<[f32; 4]>() * 2) as u64,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2
                        + std::mem::size_of::<[f32; 4]>() * 3) as u64,
                    shader_location: 5,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2
                        + std::mem::size_of::<[f32; 4]>() * 4) as u64,
                    shader_location: 6,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2
                        + std::mem::size_of::<[f32; 4]>() * 5) as u64,
                    shader_location: 7,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2
                        + std::mem::size_of::<[f32; 4]>() * 6) as u64,
                    shader_location: 8,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2
                        + std::mem::size_of::<[f32; 4]>() * 7) as u64,
                    shader_location: 9,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2
                        + std::mem::size_of::<[f32; 4]>() * 8) as u64,
                    shader_location: 10,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2
                        + std::mem::size_of::<[f32; 4]>() * 9) as u64,
                    shader_location: 11,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2
                        + std::mem::size_of::<[f32; 4]>() * 10) as u64,
                    shader_location: 12,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2
                        + std::mem::size_of::<[f32; 4]>() * 11) as u64,
                    shader_location: 13,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2
                        + std::mem::size_of::<[f32; 4]>() * 12) as u64,
                    shader_location: 14,
                },
            ],
        }
    }

    fn from_scene_vertex(vertex: SceneMeshVertex, width: f32, height: f32) -> Self {
        let x = vertex.position[0] / width * 2.0 - 1.0;
        let y = 1.0 - vertex.position[1] / height * 2.0;
        Self {
            position: [x, y],
            local_position: vertex.local_position,
            brush_meta: vertex.brush_meta,
            gradient_data0: vertex.gradient_data0,
            gradient_data1: vertex.gradient_data1,
            stop_offsets0: vertex.stop_offsets0,
            stop_offsets1: vertex.stop_offsets1,
            stop_color0: vertex.stop_colors[0],
            stop_color1: vertex.stop_colors[1],
            stop_color2: vertex.stop_colors[2],
            stop_color3: vertex.stop_colors[3],
            stop_color4: vertex.stop_colors[4],
            stop_color5: vertex.stop_colors[5],
            stop_color6: vertex.stop_colors[6],
            stop_color7: vertex.stop_colors[7],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BrushVertex {
    position: [f32; 2],
    local_position: [f32; 2],
    rect_size: [f32; 2],
    corner_radius: f32,
    brush_meta: [f32; 4],
    gradient_data0: [f32; 4],
    gradient_data1: [f32; 4],
    stop_offsets0: [f32; 4],
    stop_offsets1: [f32; 4],
    stop_color0: [f32; 4],
    stop_color1: [f32; 4],
    stop_color2: [f32; 4],
    stop_color3: [f32; 4],
    stop_color4: [f32; 4],
    stop_color5: [f32; 4],
    stop_color6: [f32; 4],
}

impl BrushVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 16] = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x2,
            3 => Float32,
            4 => Float32x4,
            5 => Float32x4,
            6 => Float32x4,
            7 => Float32x4,
            8 => Float32x4,
            9 => Float32x4,
            10 => Float32x4,
            11 => Float32x4,
            12 => Float32x4,
            13 => Float32x4,
            14 => Float32x4,
            15 => Float32x4
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }

    fn from_primitive(
        rect: Rect,
        corner_radius: f32,
        brush_data: BrushPrimitiveData,
        physical_width: f32,
        physical_height: f32,
        scale_factor: f32,
    ) -> [Self; 6] {
        let rect_x = rect.x.get() * scale_factor;
        let rect_y = rect.y.get() * scale_factor;
        let rect_width = rect.width.max(0.0).get() * scale_factor;
        let rect_height = rect.height.max(0.0).get() * scale_factor;
        let x0 = rect_x / physical_width * 2.0 - 1.0;
        let x1 = (rect_x + rect_width) / physical_width * 2.0 - 1.0;
        let y0 = 1.0 - rect_y / physical_height * 2.0;
        let y1 = 1.0 - (rect_y + rect_height) / physical_height * 2.0;
        let rect_size = [rect_width, rect_height];
        let radius = (corner_radius.max(0.0) * scale_factor)
            .min(rect_size[0] * 0.5)
            .min(rect_size[1] * 0.5);

        let build = |position: [f32; 2], local_position: [f32; 2]| Self {
            position,
            local_position,
            rect_size,
            corner_radius: radius,
            brush_meta: brush_data.brush_meta,
            gradient_data0: scale_gradient_pair(brush_data.gradient_data0, scale_factor),
            gradient_data1: scale_gradient_pair(brush_data.gradient_data1, scale_factor),
            stop_offsets0: brush_data.stop_offsets0,
            stop_offsets1: brush_data.stop_offsets1,
            stop_color0: brush_data.stop_colors[0],
            stop_color1: brush_data.stop_colors[1],
            stop_color2: brush_data.stop_colors[2],
            stop_color3: brush_data.stop_colors[3],
            stop_color4: brush_data.stop_colors[4],
            stop_color5: brush_data.stop_colors[5],
            stop_color6: brush_data.stop_colors[6],
        };

        [
            build([x0, y0], [0.0, 0.0]),
            build([x1, y0], [rect_size[0], 0.0]),
            build([x1, y1], [rect_size[0], rect_size[1]]),
            build([x0, y0], [0.0, 0.0]),
            build([x1, y1], [rect_size[0], rect_size[1]]),
            build([x0, y1], [0.0, rect_size[1]]),
        ]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CompositeVertex {
    position: [f32; 2],
    uv: [f32; 2],
    local_position: [f32; 2],
    rect_size: [f32; 2],
    corner_radius: f32,
}

impl CompositeVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x2,
            3 => Float32x2,
            4 => Float32
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }

    fn quad(rect: Rect, width: f32, height: f32, corner_radius: f32) -> [Self; 6] {
        let rect_x = rect.x.get();
        let rect_y = rect.y.get();
        let rect_width = rect.width.get();
        let rect_height = rect.height.get();
        let x0 = rect_x / width * 2.0 - 1.0;
        let x1 = (rect_x + rect_width) / width * 2.0 - 1.0;
        let y0 = 1.0 - rect_y / height * 2.0;
        let y1 = 1.0 - (rect_y + rect_height) / height * 2.0;
        let uv_x0 = rect_x / width;
        let uv_x1 = (rect_x + rect_width) / width;
        let uv_y0 = rect_y / height;
        let uv_y1 = (rect_y + rect_height) / height;
        let rect_size = [rect_width, rect_height];
        let radius = corner_radius.min(rect_width * 0.5).min(rect_height * 0.5);

        let build = |position: [f32; 2], uv: [f32; 2], local_position: [f32; 2]| Self {
            position,
            uv,
            local_position,
            rect_size,
            corner_radius: radius,
        };

        [
            build([x0, y0], [uv_x0, uv_y0], [0.0, 0.0]),
            build([x1, y0], [uv_x1, uv_y0], [rect_size[0], 0.0]),
            build(
                [x1, y1],
                [uv_x1, uv_y1],
                [rect_size[0], rect_size[1]],
            ),
            build([x0, y0], [uv_x0, uv_y0], [0.0, 0.0]),
            build(
                [x1, y1],
                [uv_x1, uv_y1],
                [rect_size[0], rect_size[1]],
            ),
            build([x0, y1], [uv_x0, uv_y1], [0.0, rect_size[1]]),
        ]
    }
}

fn scale_gradient_pair(mut pair: [f32; 4], scale_factor: f32) -> [f32; 4] {
    pair[0] *= scale_factor;
    pair[1] *= scale_factor;
    pair[2] *= scale_factor;
    pair[3] *= scale_factor;
    pair
}

fn instance_descriptor(clear_color: TguiColor) -> wgpu::InstanceDescriptor {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = instance_backends(clear_color);
    #[cfg(all(target_os = "android", feature = "android"))]
    {
        descriptor.flags = wgpu::InstanceFlags::empty();
        descriptor.backend_options.gl.debug_fns = wgpu::GlDebugFns::Disabled;
    }
    descriptor
}

fn create_instance(clear_color: TguiColor) -> wgpu::Instance {
    let descriptor = instance_descriptor(clear_color);
    wgpu::Instance::new(descriptor)
}

fn create_surface(
    instance: &wgpu::Instance,
    window: Arc<dyn Window>,
) -> Result<wgpu::Surface<'static>, TguiError> {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        let raw_display_handle = window.display_handle().map_err(|error| {
            TguiError::TextRender(format!("display handle unavailable: {error}"))
        })?;
        let raw_window_handle = window.window_handle().map_err(|error| {
            TguiError::TextRender(format!("window handle unavailable: {error}"))
        })?;

        return Ok(unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display_handle.as_raw()),
                raw_window_handle: raw_window_handle.as_raw(),
            })?
        });
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        instance.create_surface(window).map_err(Into::into)
    }
}

fn adapter_power_preference() -> wgpu::PowerPreference {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return wgpu::PowerPreference::HighPerformance;
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        wgpu::PowerPreference::default()
    }
}

fn required_device_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return adapter.limits();
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        let _ = adapter;
        wgpu::Limits::default()
    }
}

fn surface_msaa_sample_count(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> u32 {
    let features = adapter.get_texture_format_features(format);
    supported_msaa_sample_count(features.flags)
}

fn supported_msaa_sample_count(flags: wgpu::TextureFormatFeatureFlags) -> u32 {
    [4, 2]
        .into_iter()
        .find(|count| {
            flags.sample_count_supported(*count)
                && flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE)
        })
        .unwrap_or(1)
}

fn create_multisample_target(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    sample_count: u32,
) -> Option<MultisampleTarget> {
    if sample_count <= 1 || config.width == 0 || config.height == 0 {
        return None;
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tgui-msaa-color-target"),
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    Some(MultisampleTarget {
        _texture: texture,
        _view: view,
    })
}

fn create_offscreen_target(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    label: &str,
) -> Option<OffscreenTarget> {
    if config.width == 0 || config.height == 0 {
        return None;
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    Some(OffscreenTarget {
        view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
        texture,
    })
}

#[cfg(test)]
mod tests {
    use super::supported_msaa_sample_count;

    #[test]
    fn prefers_x4_msaa_when_resolve_is_supported() {
        let flags = wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4
            | wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE;
        assert_eq!(supported_msaa_sample_count(flags), 4);
    }

    #[test]
    fn falls_back_to_x2_when_x4_is_unavailable() {
        let flags = wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X2
            | wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE;
        assert_eq!(supported_msaa_sample_count(flags), 2);
    }

    #[test]
    fn disables_msaa_without_resolve_support() {
        assert_eq!(
            supported_msaa_sample_count(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4),
            1
        );
    }
}

fn instance_backends(_clear_color: TguiColor) -> wgpu::Backends {
    #[cfg(target_os = "windows")]
    {
        if _clear_color.a < 255 {
            return wgpu::Backends::PRIMARY;
        }
    }

    default_backends()
}

fn default_backends() -> wgpu::Backends {
    #[cfg(target_arch = "wasm32")]
    {
        return wgpu::Backends::BROWSER_WEBGPU;
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        return wgpu::Backends::METAL;
    }

    #[cfg(all(
        target_os = "android",
        feature = "android",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    {
        return wgpu::Backends::GL;
    }

    #[cfg(any(
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos")),
        all(
            target_os = "android",
            feature = "android",
            not(any(target_arch = "x86", target_arch = "x86_64"))
        )
    ))]
    {
        return wgpu::Backends::VULKAN;
    }

    #[allow(unreachable_code)]
    wgpu::Backends::all()
}

fn surface_present_mode(modes: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Fifo)
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            })
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::AutoNoVsync)
            })
            .unwrap_or(wgpu::PresentMode::Fifo);
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::AutoNoVsync)
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            })
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::Fifo)
            })
            .unwrap_or(wgpu::PresentMode::Fifo)
    }
}

fn surface_alpha_mode(modes: &[wgpu::CompositeAlphaMode]) -> wgpu::CompositeAlphaMode {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        modes
            .iter()
            .copied()
            .find(|mode| *mode != wgpu::CompositeAlphaMode::Opaque)
            .unwrap_or(wgpu::CompositeAlphaMode::Auto)
    }
}

fn surface_clear_color(color: TguiColor) -> wgpu::Color {
    let alpha = color.a as f64 / 255.0;
    wgpu::Color {
        r: (color.r as f64 / 255.0) * alpha,
        g: (color.g as f64 / 255.0) * alpha,
        b: (color.b as f64 / 255.0) * alpha,
        a: alpha,
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TextVertex {
    position: [f32; 2],
    uv: [f32; 2],
    local_position: [f32; 2],
    rect_size: [f32; 2],
    corner_radius: f32,
    _padding: f32,
}

impl TextVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: std::mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: (std::mem::size_of::<[f32; 2]>() * 2) as u64,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: (std::mem::size_of::<[f32; 2]>() * 3) as u64,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: (std::mem::size_of::<[f32; 2]>() * 4) as u64,
                    shader_location: 4,
                },
            ],
        }
    }

    fn quad(
        rect: crate::ui::widget::Rect,
        width: f32,
        height: f32,
        corner_radius: f32,
        physical_width: f32,
        physical_height: f32,
        scale_factor: f32,
    ) -> [Self; 6] {
        let rect_x = rect.x.get();
        let rect_y = rect.y.get();
        let rect_width = rect.width.get();
        let rect_height = rect.height.get();
        let x0 = rect_x / width * 2.0 - 1.0;
        let x1 = (rect_x + rect_width) / width * 2.0 - 1.0;
        let y0 = 1.0 - rect_y / height * 2.0;
        let y1 = 1.0 - (rect_y + rect_height) / height * 2.0;
        let rect_width_physical = rect_width * scale_factor;
        let rect_height_physical = rect_height * scale_factor;
        let radius = (corner_radius.max(0.0) * scale_factor)
            .min(rect_width_physical * 0.5)
            .min(rect_height_physical * 0.5);
        let rect_size = [rect_width_physical, rect_height_physical];
        let local_tl = [0.0, 0.0];
        let local_tr = [rect_size[0], 0.0];
        let local_br = [rect_size[0], rect_size[1]];
        let local_bl = [0.0, rect_size[1]];

        [
            Self {
                position: [x0, y0],
                uv: [0.0, 0.0],
                local_position: local_tl,
                rect_size,
                corner_radius: radius,
                _padding: physical_width + physical_height - (physical_width + physical_height),
            },
            Self {
                position: [x1, y0],
                uv: [1.0, 0.0],
                local_position: local_tr,
                rect_size,
                corner_radius: radius,
                _padding: 0.0,
            },
            Self {
                position: [x1, y1],
                uv: [1.0, 1.0],
                local_position: local_br,
                rect_size,
                corner_radius: radius,
                _padding: 0.0,
            },
            Self {
                position: [x0, y0],
                uv: [0.0, 0.0],
                local_position: local_tl,
                rect_size,
                corner_radius: radius,
                _padding: 0.0,
            },
            Self {
                position: [x1, y1],
                uv: [1.0, 1.0],
                local_position: local_br,
                rect_size,
                corner_radius: radius,
                _padding: 0.0,
            },
            Self {
                position: [x0, y1],
                uv: [0.0, 1.0],
                local_position: local_bl,
                rect_size,
                corner_radius: radius,
                _padding: 0.0,
            },
        ]
    }
}
