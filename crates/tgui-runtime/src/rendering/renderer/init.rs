use super::init_pipelines::create_renderer_pipelines;
use super::*;

impl Renderer {
    pub(super) async fn new_async(
        window: Arc<dyn Window>,
        clear_color: TguiColor,
        requested_msaa_mode: MsaaMode,
    ) -> Result<Box<Self>, TguiError> {
        let size = window.surface_size();
        let instance = create_instance(clear_color);
        let surface = create_surface(&instance, window.clone())?;
        let adapter = request_adapter(&instance, &surface, clear_color).await?;
        let push_constants_supported = {
            let payload_size = core::mem::size_of::<PushTranslate>() as u32;
            adapter.features().contains(wgpu::Features::IMMEDIATES)
                && adapter.limits().max_immediate_size >= payload_size
        };
        let required_limits = required_device_limits(&adapter, push_constants_supported);

        let required_features = if push_constants_supported {
            wgpu::Features::IMMEDIATES
        } else {
            wgpu::Features::empty()
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tgui-device"),
                required_features,
                required_limits,
                memory_hints: wgpu::MemoryHints::MemoryUsage,
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

        let pipelines =
            create_renderer_pipelines(&device, format, msaa_sample_count, push_constants_supported);

        let scale_factor = 1.0_f32.max(window.scale_factor() as f32);

        surface.configure(&device, &config);
        let targets = RendererTargets::new(&device, &config, msaa_sample_count);
        let vertex_pool = super::vertex_pool::VertexBufferPool::new(&device);
        let text_atlas = super::text::TextAtlas::new(device.limits().max_texture_dimension_2d);

        let mut renderer = Box::new(Self {
            window: Some(window),
            surface: Some(surface),
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
            snapshot_target: None,
            blur_target: None,
            blur_scratch_target: None,
            canvas_composite_targets: Vec::new(),
            canvas_composite_mask_targets: Vec::new(),
            present_resources: None,
            clear_color,
            text_system: TextSystem::new(),
            text_upload_scratch: Vec::new(),
            text_atlas,
            text_cache: HashMap::new(),
            texture_cache: HashMap::new(),
            next_sprite_bind_group_id: 1,
            active_text_keys_scratch: HashSet::new(),
            active_texture_keys_scratch: HashSet::new(),
            cache_liveness_scene_serial: None,
            vertex_pool,
            retained_prepare_cache: prepare::RetainedPrepareCache::default(),
            prepared_command_scratch: prepare::PreparedCommandScratch::default(),
            scroll_translate_cache: prepare::ScrollTranslateCache::default(),
            mesh_clip_bind_group_cache: prepare::MeshClipBindGroupCache::default(),
            last_prepare_stats: HashMap::new(),
            #[cfg(any(test, feature = "bench-support"))]
            sprite_draw_batching_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            primitive_draw_batching_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            transparent_shape_skip_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            last_scene_draw_stats: super::draw::SceneDrawStats::default(),
            #[cfg(any(test, feature = "bench-support"))]
            text_cache_hits: 0,
            #[cfg(any(test, feature = "bench-support"))]
            text_cache_misses: 0,
            #[cfg(any(test, feature = "bench-support"))]
            text_atlas_releases: 0,
            #[cfg(any(test, feature = "bench-support"))]
            text_prepare_cache_clears: 0,
            #[cfg(any(test, feature = "bench-support"))]
            text_alpha_cache_normalization_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            text_mask_tint_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            text_r8_atlas_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            text_atlas_deferred_upload_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            text_blend_fast_path_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            text_blend_stats: super::text::TextBlendStats::default(),
            push_constants_supported,
        });
        renderer.recreate_present_resources();
        Ok(renderer)
    }

    #[cfg(feature = "bench-support")]
    pub(crate) async fn new_headless_for_bench(
        size: PhysicalSize<u32>,
        clear_color: TguiColor,
    ) -> Result<(Box<Self>, wgpu::AdapterInfo), TguiError> {
        let instance = create_instance(clear_color);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;
        let adapter_info = adapter.get_info();
        let push_constants_supported = {
            let payload_size = core::mem::size_of::<PushTranslate>() as u32;
            adapter.features().contains(wgpu::Features::IMMEDIATES)
                && adapter.limits().max_immediate_size >= payload_size
        };
        let required_features = if push_constants_supported {
            wgpu::Features::IMMEDIATES
        } else {
            wgpu::Features::empty()
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tgui-bench-headless-device"),
                required_features,
                required_limits: required_device_limits(&adapter, push_constants_supported),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                experimental_features: Default::default(),
                trace: Default::default(),
            })
            .await?;
        let format = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ]
        .into_iter()
        .find(|format| {
            adapter
                .get_texture_format_features(*format)
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        })
        .ok_or(TguiError::NoSurfaceFormat)?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        let msaa_sample_count = 1;
        let pipelines =
            create_renderer_pipelines(&device, format, msaa_sample_count, push_constants_supported);
        let targets = RendererTargets::new(&device, &config, msaa_sample_count);
        let vertex_pool = super::vertex_pool::VertexBufferPool::new(&device);
        let text_atlas_limit = if std::env::var_os("TGUI_BENCH_DISABLE_TEXT_ATLAS").is_some() {
            0
        } else {
            device.limits().max_texture_dimension_2d
        };
        let text_atlas = super::text::TextAtlas::new(text_atlas_limit);
        let mut renderer = Box::new(Self {
            window: None,
            surface: None,
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
            scale_factor: 1.0,
            msaa_sample_count,
            scene_target: targets.scene_target,
            snapshot_target: None,
            blur_target: None,
            blur_scratch_target: None,
            canvas_composite_targets: Vec::new(),
            canvas_composite_mask_targets: Vec::new(),
            present_resources: None,
            clear_color,
            text_system: TextSystem::new(),
            text_upload_scratch: Vec::new(),
            text_atlas,
            text_cache: HashMap::new(),
            texture_cache: HashMap::new(),
            next_sprite_bind_group_id: 1,
            active_text_keys_scratch: HashSet::new(),
            active_texture_keys_scratch: HashSet::new(),
            cache_liveness_scene_serial: None,
            vertex_pool,
            retained_prepare_cache: prepare::RetainedPrepareCache::default(),
            prepared_command_scratch: prepare::PreparedCommandScratch::default(),
            scroll_translate_cache: prepare::ScrollTranslateCache::default(),
            mesh_clip_bind_group_cache: prepare::MeshClipBindGroupCache::default(),
            last_prepare_stats: HashMap::new(),
            #[cfg(any(test, feature = "bench-support"))]
            sprite_draw_batching_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            primitive_draw_batching_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            transparent_shape_skip_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            last_scene_draw_stats: super::draw::SceneDrawStats::default(),
            #[cfg(any(test, feature = "bench-support"))]
            text_cache_hits: 0,
            #[cfg(any(test, feature = "bench-support"))]
            text_cache_misses: 0,
            #[cfg(any(test, feature = "bench-support"))]
            text_atlas_releases: 0,
            #[cfg(any(test, feature = "bench-support"))]
            text_prepare_cache_clears: 0,
            #[cfg(any(test, feature = "bench-support"))]
            text_alpha_cache_normalization_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            text_mask_tint_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            text_r8_atlas_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            text_atlas_deferred_upload_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            text_blend_fast_path_enabled: true,
            #[cfg(any(test, feature = "bench-support"))]
            text_blend_stats: super::text::TextBlendStats::default(),
            push_constants_supported,
        });
        renderer.recreate_present_resources();
        Ok((renderer, adapter_info))
    }
}
