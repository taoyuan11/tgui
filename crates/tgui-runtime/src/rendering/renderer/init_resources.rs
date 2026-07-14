pub(super) struct RendererPipelineResources {
    pub(super) rect_shader: wgpu::ShaderModule,
    pub(super) mesh_shader: wgpu::ShaderModule,
    pub(super) text_shader: wgpu::ShaderModule,
    #[cfg(feature = "video")]
    pub(super) video_yuv_shader: wgpu::ShaderModule,
    pub(super) brush_shader: wgpu::ShaderModule,
    pub(super) backdrop_blur_shader: wgpu::ShaderModule,
    pub(super) backdrop_composite_shader: wgpu::ShaderModule,
    pub(super) canvas_composite_shader: wgpu::ShaderModule,
    pub(super) present_shader: wgpu::ShaderModule,
    pub(super) text_bind_group_layout: wgpu::BindGroupLayout,
    #[cfg(feature = "video")]
    pub(super) video_yuv_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) present_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) mesh_clip_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) backdrop_blur_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) backdrop_composite_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) canvas_composite_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) text_sampler: wgpu::Sampler,
}

fn immediate_shader_source(source: &'static str, immediates_enabled: bool) -> String {
    const ZERO_DECLARATION: &str =
        "const pc: PushTranslate = PushTranslate(vec2<f32>(0.0), vec2<f32>(0.0));";
    if immediates_enabled {
        source.replace(ZERO_DECLARATION, "var<immediate> pc: PushTranslate;")
    } else {
        source.to_owned()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "video")]
    use super::immediate_shader_source;

    #[cfg(feature = "video")]
    #[test]
    fn video_yuv_shader_keeps_replaceable_immediate_declaration() {
        let source = include_str!("../shader/video_yuv.wgsl");

        let patched = immediate_shader_source(source, true);

        assert!(patched.contains("var<immediate> pc: PushTranslate;"));
        assert!(!patched.contains("const pc: PushTranslate = PushTranslate"));
        assert!(immediate_shader_source(source, false)
            .contains("const pc: PushTranslate = PushTranslate"));
    }
}

pub(super) fn create_renderer_pipeline_resources(
    device: &wgpu::Device,
    immediates_enabled: bool,
) -> RendererPipelineResources {
    let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tgui-rect-shader"),
        source: wgpu::ShaderSource::Wgsl(
            immediate_shader_source(include_str!("../shader/rect.wgsl"), immediates_enabled).into(),
        ),
    });
    let mesh_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tgui-mesh-shader"),
        source: wgpu::ShaderSource::Wgsl(
            immediate_shader_source(include_str!("../shader/mesh.wgsl"), immediates_enabled).into(),
        ),
    });
    let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tgui-text-shader"),
        source: wgpu::ShaderSource::Wgsl(
            immediate_shader_source(include_str!("../shader/text.wgsl"), immediates_enabled).into(),
        ),
    });
    #[cfg(feature = "video")]
    let video_yuv_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tgui-video-yuv-shader"),
        source: wgpu::ShaderSource::Wgsl(
            immediate_shader_source(include_str!("../shader/video_yuv.wgsl"), immediates_enabled)
                .into(),
        ),
    });
    let brush_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tgui-brush-shader"),
        source: wgpu::ShaderSource::Wgsl(
            immediate_shader_source(include_str!("../shader/brush.wgsl"), immediates_enabled)
                .into(),
        ),
    });
    let backdrop_blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tgui-backdrop-blur-shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shader/backdrop_blur.wgsl").into()),
    });
    let backdrop_composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tgui-backdrop-composite-shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shader/backdrop_composite.wgsl").into()),
    });
    let canvas_composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tgui-canvas-composite-shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shader/canvas_composite.wgsl").into()),
    });
    let present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tgui-present-shader"),
        source: wgpu::ShaderSource::Wgsl(
            immediate_shader_source(include_str!("../shader/text.wgsl"), immediates_enabled).into(),
        ),
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
    #[cfg(feature = "video")]
    let video_yuv_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tgui-video-yuv-bind-group-layout"),
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

    RendererPipelineResources {
        rect_shader,
        mesh_shader,
        text_shader,
        #[cfg(feature = "video")]
        video_yuv_shader,
        brush_shader,
        backdrop_blur_shader,
        backdrop_composite_shader,
        canvas_composite_shader,
        present_shader,
        text_bind_group_layout,
        #[cfg(feature = "video")]
        video_yuv_bind_group_layout,
        present_bind_group_layout,
        mesh_clip_bind_group_layout,
        backdrop_blur_bind_group_layout,
        backdrop_composite_bind_group_layout,
        canvas_composite_bind_group_layout,
        text_sampler,
    }
}
