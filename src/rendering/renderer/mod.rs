mod draw;
mod prepare;
mod surface;
mod targets;
mod text;
mod texture;
mod vertex;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use self::surface::{
    create_instance, create_surface, pipeline_multisample_state, request_adapter,
    required_device_limits, resolve_surface_msaa_sample_count, surface_alpha_mode,
    surface_clear_color, surface_present_mode,
};
use self::targets::RendererTargets;
use self::vertex::{
    physical_mesh_clip_mask_data, BrushVertex, CompositeVertex, MeshVertex, RectVertex, TextVertex,
};
use crate::application::MsaaMode;
use crate::foundation::color::Color as TguiColor;
use crate::foundation::error::TguiError;
use crate::platform::backend::window::Window;
use crate::platform::dpi::PhysicalSize;
use crate::text::font::FontCatalog;
use crate::ui::widget::ScenePrimitives;
use bytemuck::{Pod, Zeroable};
use cosmic_text::{FontSystem, SwashCache};
#[cfg(all(target_env = "ohos", feature = "ohos"))]
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

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
    scene_text_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    backdrop_blur_pipeline: wgpu::RenderPipeline,
    backdrop_composite_pipeline: wgpu::RenderPipeline,
    canvas_composite_pipeline: wgpu::RenderPipeline,
    text_bind_group_layout: wgpu::BindGroupLayout,
    present_bind_group_layout: wgpu::BindGroupLayout,
    mesh_clip_bind_group_layout: wgpu::BindGroupLayout,
    backdrop_blur_bind_group_layout: wgpu::BindGroupLayout,
    backdrop_composite_bind_group_layout: wgpu::BindGroupLayout,
    canvas_composite_bind_group_layout: wgpu::BindGroupLayout,
    text_sampler: wgpu::Sampler,
    size: PhysicalSize<u32>,
    scale_factor: f32,
    msaa_sample_count: u32,
    scene_target: Option<OffscreenTarget>,
    blur_target: Option<OffscreenTarget>,
    blur_scratch_target: Option<OffscreenTarget>,
    composite_target: Option<OffscreenTarget>,
    composite_mask_target: Option<OffscreenTarget>,
    clear_color: TguiColor,
    text_system: TextSystem,
    text_cache: HashMap<TextCacheKey, TextCacheEntry>,
    texture_cache: HashMap<(u64, u64), TextureCacheEntry>,
}

struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

#[derive(Clone)]
struct OffscreenTarget {
    single_texture: wgpu::Texture,
    single_view: wgpu::TextureView,
    _msaa_texture: Option<wgpu::Texture>,
    msaa_view: Option<wgpu::TextureView>,
}

struct TextCacheEntry {
    bind_group: wgpu::BindGroup,
    _texture: wgpu::Texture,
}

struct TextureCacheEntry {
    bind_group: wgpu::BindGroup,
    _texture: wgpu::Texture,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct TextCacheKey {
    content: String,
    font_family: Option<String>,
    width: u32,
    height: u32,
    color: [u8; 4],
    force_color: bool,
    font_size_bits: u32,
    line_height_bits: u32,
    letter_spacing_bits: u32,
    font_weight: u16,
    wrap_mode: u8,
    overflow_mode: u8,
    horizontal_align: u8,
    vertical_align: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurUniform {
    direction: [f32; 2],
    texel_size: [f32; 2],
    radius: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CompositeUniform {
    data0: [f32; 4],
    data1: [f32; 4],
    data2: [f32; 4],
    data3: [f32; 4],
    data4: [f32; 4],
}

impl Renderer {
    pub fn new(
        window: Arc<dyn Window>,
        clear_color: TguiColor,
        requested_msaa_mode: MsaaMode,
        fonts: &FontCatalog,
    ) -> Result<Self, TguiError> {
        pollster::block_on(Self::new_async(
            window,
            clear_color,
            requested_msaa_mode,
            fonts,
        ))
    }

    async fn new_async(
        window: Arc<dyn Window>,
        clear_color: TguiColor,
        requested_msaa_mode: MsaaMode,
        fonts: &FontCatalog,
    ) -> Result<Self, TguiError> {
        let size = window.surface_size();
        let instance = create_instance(clear_color);
        let surface = create_surface(&instance, window.clone())?;
        let adapter = request_adapter(&instance, &surface, clear_color).await?;
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

        let alpha_mode = surface_alpha_mode(&caps.alpha_modes, clear_color);
        let msaa_sample_count =
            resolve_surface_msaa_sample_count(&adapter, format, requested_msaa_mode);

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
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader/rect.wgsl").into()),
        });
        let mesh_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-mesh-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader/mesh.wgsl").into()),
        });
        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-text-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader/text.wgsl").into()),
        });
        let brush_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-brush-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader/brush.wgsl").into()),
        });
        let backdrop_blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-backdrop-blur-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader/backdrop_blur.wgsl").into()),
        });
        let backdrop_composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-backdrop-composite-shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shader/backdrop_composite.wgsl").into(),
            ),
        });
        let canvas_composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-canvas-composite-shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shader/canvas_composite.wgsl").into(),
            ),
        });
        let present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-present-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader/text.wgsl").into()),
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
            multisample: pipeline_multisample_state(msaa_sample_count),
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
            multisample: pipeline_multisample_state(msaa_sample_count),
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

        let mesh_clip_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tgui-mesh-clip-bind-group-layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let mesh_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tgui-mesh-pipeline-layout"),
            bind_group_layouts: &[Some(&mesh_clip_bind_group_layout)],
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
            multisample: pipeline_multisample_state(msaa_sample_count),
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
        let canvas_composite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tgui-canvas-composite-bind-group-layout"),
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
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
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

        let present_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tgui-present-bind-group-layout"),
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

        let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tgui-text-pipeline-layout"),
            bind_group_layouts: &[Some(&text_bind_group_layout)],
            immediate_size: 0,
        });

        let scene_text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tgui-scene-text-pipeline"),
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
            multisample: pipeline_multisample_state(msaa_sample_count),
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

        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tgui-text-pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("tgui-present-pipeline-layout"),
                    bind_group_layouts: &[Some(&present_bind_group_layout)],
                    immediate_size: 0,
                }),
            ),
            vertex: wgpu::VertexState {
                module: &present_shader,
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
                module: &present_shader,
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

        let backdrop_blur_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("tgui-backdrop-blur-pipeline-layout"),
                bind_group_layouts: &[Some(&backdrop_blur_bind_group_layout)],
                immediate_size: 0,
            });
        let backdrop_blur_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                multisample: pipeline_multisample_state(msaa_sample_count),
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
                multisample: pipeline_multisample_state(msaa_sample_count),
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
        let canvas_composite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("tgui-canvas-composite-pipeline-layout"),
                bind_group_layouts: &[Some(&canvas_composite_bind_group_layout)],
                immediate_size: 0,
            });
        let canvas_composite_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("tgui-canvas-composite-pipeline"),
                layout: Some(&canvas_composite_pipeline_layout),
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
                multisample: pipeline_multisample_state(msaa_sample_count),
                fragment: Some(wgpu::FragmentState {
                    module: &canvas_composite_shader,
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
        let targets = RendererTargets::new(&device, &config, msaa_sample_count);

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            rect_pipeline,
            brush_pipeline,
            mesh_pipeline,
            scene_text_pipeline,
            text_pipeline,
            backdrop_blur_pipeline,
            backdrop_composite_pipeline,
            canvas_composite_pipeline,
            text_bind_group_layout,
            present_bind_group_layout,
            mesh_clip_bind_group_layout,
            backdrop_blur_bind_group_layout,
            backdrop_composite_bind_group_layout,
            canvas_composite_bind_group_layout,
            text_sampler,
            size,
            scale_factor,
            msaa_sample_count,
            scene_target: targets.scene_target,
            blur_target: targets.blur_target,
            blur_scratch_target: targets.blur_scratch_target,
            composite_target: targets.composite_target,
            composite_mask_target: targets.composite_mask_target,
            clear_color,
            text_system: TextSystem {
                font_system,
                swash_cache: SwashCache::new(),
            },
            text_cache: HashMap::new(),
            texture_cache: HashMap::new(),
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
            .retain(|key, _| active_texture_keys.contains(key));

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

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tgui-render-encoder"),
            });

        {
            let scene_target = self
                .scene_target
                .as_ref()
                .ok_or_else(|| TguiError::TextRender("scene target unavailable".into()))?;
            let scene_clear_view = self.offscreen_attachment_view(scene_target);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-scene-clear-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_clear_view,
                    resolve_target: self.offscreen_resolve_target_for_draw(scene_target),
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

        let mut cleared_draw_target = false;
        self.execute_prepared_commands(&mut encoder, &command_buffers.0, &mut cleared_draw_target)?;
        self.execute_prepared_commands(&mut encoder, &overlay_buffers.0, &mut cleared_draw_target)?;
        let scene_target = self
            .scene_target
            .as_ref()
            .ok_or_else(|| TguiError::TextRender("scene target unavailable".into()))?;
        let scene_view = self.offscreen_sampled_view(scene_target);
        self.blit_scene_to_surface(&mut encoder, scene_view, &color_attachment_view, None);

        self.queue.submit(Some(encoder.finish()));
        self.window.pre_present_notify();
        frame.present();

        Ok(RenderStatus::Rendered)
    }

    pub fn set_clear_color(&mut self, clear_color: TguiColor) {
        self.clear_color = clear_color;
    }

    pub fn reconfigure(&mut self) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }

        self.surface.configure(&self.device, &self.config);
        self.recreate_offscreen_targets();
    }
}

#[cfg(test)]
mod tests;
