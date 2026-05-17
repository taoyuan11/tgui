use super::init_pipelines::create_renderer_pipelines;
use super::*;

impl Renderer {
    pub(super) async fn new_async(
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

        let pipelines = create_renderer_pipelines(&device, format, msaa_sample_count);

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
            rect_pipeline: pipelines.rect_pipeline,
            brush_pipeline: pipelines.brush_pipeline,
            mesh_pipeline: pipelines.mesh_pipeline,
            scene_text_pipeline: pipelines.scene_text_pipeline,
            text_pipeline: pipelines.text_pipeline,
            backdrop_blur_pipeline: pipelines.backdrop_blur_pipeline,
            backdrop_composite_pipeline: pipelines.backdrop_composite_pipeline,
            canvas_composite_pipeline: pipelines.canvas_composite_pipeline,
            text_bind_group_layout: pipelines.text_bind_group_layout,
            present_bind_group_layout: pipelines.present_bind_group_layout,
            mesh_clip_bind_group_layout: pipelines.mesh_clip_bind_group_layout,
            backdrop_blur_bind_group_layout: pipelines.backdrop_blur_bind_group_layout,
            backdrop_composite_bind_group_layout: pipelines.backdrop_composite_bind_group_layout,
            canvas_composite_bind_group_layout: pipelines.canvas_composite_bind_group_layout,
            text_sampler: pipelines.text_sampler,
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
}
