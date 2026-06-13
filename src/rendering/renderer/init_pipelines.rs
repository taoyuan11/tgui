use super::init_resources::create_renderer_pipeline_resources;
use super::*;

pub(super) struct RendererPipelines {
    pub(super) rect_pipeline: wgpu::RenderPipeline,
    pub(super) brush_pipeline: wgpu::RenderPipeline,
    pub(super) mesh_pipeline: wgpu::RenderPipeline,
    pub(super) scene_text_pipeline: wgpu::RenderPipeline,
    pub(super) text_pipeline: wgpu::RenderPipeline,
    pub(super) backdrop_blur_pipeline: wgpu::RenderPipeline,
    pub(super) backdrop_composite_pipeline: wgpu::RenderPipeline,
    pub(super) canvas_composite_pipeline: wgpu::RenderPipeline,
    pub(super) text_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) present_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) mesh_clip_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) backdrop_blur_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) backdrop_composite_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) canvas_composite_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) text_sampler: wgpu::Sampler,
}

pub(super) fn create_renderer_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    msaa_sample_count: u32,
    #[cfg(feature = "transform-only-scroll-gpu")] immediates_enabled: bool,
) -> RendererPipelines {
    #[cfg(not(feature = "transform-only-scroll-gpu"))]
    let immediates_enabled = false;
    let resources = create_renderer_pipeline_resources(device, immediates_enabled);

    let immediate_size = if immediates_enabled {
        core::mem::size_of::<PushTranslate>() as u32
    } else {
        0
    };

    let rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tgui-rect-pipeline-layout"),
        bind_group_layouts: &[],
        immediate_size,
    });

    let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tgui-rect-pipeline"),
        layout: Some(&rect_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &resources.rect_shader,
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
            module: &resources.rect_shader,
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

    let brush_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tgui-brush-pipeline-layout"),
        bind_group_layouts: &[],
        immediate_size,
    });

    let brush_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tgui-brush-pipeline"),
        layout: Some(&brush_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &resources.brush_shader,
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
            module: &resources.brush_shader,
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
        bind_group_layouts: &[Some(&resources.mesh_clip_bind_group_layout)],
        immediate_size,
    });

    let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tgui-mesh-pipeline"),
        layout: Some(&mesh_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &resources.mesh_shader,
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
            module: &resources.mesh_shader,
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

    let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tgui-text-pipeline-layout"),
        bind_group_layouts: &[Some(&resources.text_bind_group_layout)],
        immediate_size,
    });

    let scene_text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tgui-scene-text-pipeline"),
        layout: Some(&text_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &resources.text_shader,
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
            module: &resources.text_shader,
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
                bind_group_layouts: &[Some(&resources.present_bind_group_layout)],
                immediate_size,
            }),
        ),
        vertex: wgpu::VertexState {
            module: &resources.present_shader,
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
            module: &resources.present_shader,
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
            bind_group_layouts: &[Some(&resources.backdrop_blur_bind_group_layout)],
            immediate_size: 0,
        });
    let backdrop_blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tgui-backdrop-blur-pipeline"),
        layout: Some(&backdrop_blur_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &resources.backdrop_blur_shader,
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
        multisample: pipeline_multisample_state(1),
        fragment: Some(wgpu::FragmentState {
            module: &resources.backdrop_blur_shader,
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
            bind_group_layouts: &[Some(&resources.backdrop_composite_bind_group_layout)],
            immediate_size: 0,
        });
    let backdrop_composite_pipeline =
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tgui-backdrop-composite-pipeline"),
            layout: Some(&backdrop_composite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &resources.backdrop_composite_shader,
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
                module: &resources.backdrop_composite_shader,
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
            bind_group_layouts: &[Some(&resources.canvas_composite_bind_group_layout)],
            immediate_size: 0,
        });
    let canvas_composite_pipeline =
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tgui-canvas-composite-pipeline"),
            layout: Some(&canvas_composite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &resources.backdrop_composite_shader,
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
                module: &resources.canvas_composite_shader,
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
    RendererPipelines {
        rect_pipeline,
        brush_pipeline,
        mesh_pipeline,
        scene_text_pipeline,
        text_pipeline,
        backdrop_blur_pipeline,
        backdrop_composite_pipeline,
        canvas_composite_pipeline,
        text_bind_group_layout: resources.text_bind_group_layout,
        present_bind_group_layout: resources.present_bind_group_layout,
        mesh_clip_bind_group_layout: resources.mesh_clip_bind_group_layout,
        backdrop_blur_bind_group_layout: resources.backdrop_blur_bind_group_layout,
        backdrop_composite_bind_group_layout: resources.backdrop_composite_bind_group_layout,
        canvas_composite_bind_group_layout: resources.canvas_composite_bind_group_layout,
        text_sampler: resources.text_sampler,
    }
}
