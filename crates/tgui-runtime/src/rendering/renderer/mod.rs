mod draw;
mod init;
mod init_pipelines;
mod init_resources;
mod prepare;
mod surface;
mod targets;
mod text;
mod texture;
mod types;
mod vertex;
mod vertex_pool;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use self::surface::{
    create_instance, create_surface, pipeline_multisample_state, request_adapter,
    required_device_limits, resolve_surface_msaa_sample_count, surface_alpha_mode,
    surface_clear_color, surface_present_mode,
};
use self::targets::RendererTargets;
use self::types::*;
use self::vertex::{
    physical_mesh_clip_mask_data, BrushVertex, BrushVertexSpec, CompositeQuadSpec, CompositeVertex,
    MeshClipMaskUniformData, MeshVertex, RectVertex, TextQuadSpec, TextTransformSpec, TextVertex,
    VertexViewport,
};
use crate::application::MsaaMode;
use crate::foundation::color::Color as TguiColor;
use crate::foundation::error::TguiError;
use crate::platform::backend::window::Window;
use crate::platform::dpi::PhysicalSize;
use crate::text::font::FontManager;
use crate::ui::widget::{RenderCommand, ScenePrimitives, TransformRecord, WidgetId};

/// 每-draw 滚动平移 immediate data 的载荷。
/// 需两套平移量:position 在 NDC、clip_local_position 在物理像素。16 字节,4 字节对齐。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct PushTranslate {
    /// NDC 空间平移量（clip-space delta），加到 output.position。非滚动 draw 传 [0,0]。
    pub(super) offset_ndc: [f32; 2],
    /// 物理像素空间平移量，加到 output.clip_local_position（使 clip mask 随内容平移，
    /// 避免圆角 clip 脱钩）。非滚动 draw 传 [0,0]。
    pub(super) offset_physical: [f32; 2],
}

pub enum RenderStatus {
    Rendered,
    ReconfigureSurface,
    SkipFrame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InitialSceneCommandKind {
    Regular,
    SamplesExistingTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InitialSceneClear {
    FirstRegularPass,
    ExplicitBeforeTargetSampling,
    ExplicitBeforePresent,
}

impl InitialSceneClear {
    pub(super) fn dedicated_pass_count(self) -> usize {
        usize::from(self != Self::FirstRegularPass)
    }
}

fn initial_scene_command_kind(command: &prepare::PreparedCommand) -> InitialSceneCommandKind {
    match command {
        prepare::PreparedCommand::BackdropBlur(_)
        | prepare::PreparedCommand::CanvasComposite(_) => {
            InitialSceneCommandKind::SamplesExistingTarget
        }
        prepare::PreparedCommand::Rect(_)
        | prepare::PreparedCommand::Brush(_)
        | prepare::PreparedCommand::Mesh(_)
        | prepare::PreparedCommand::Sprite(_) => InitialSceneCommandKind::Regular,
    }
}

pub(super) fn first_initial_scene_command(
    main: Option<InitialSceneCommandKind>,
    overlay: Option<InitialSceneCommandKind>,
) -> Option<InitialSceneCommandKind> {
    main.or(overlay)
}

pub(super) fn initial_scene_clear_strategy(
    first_command: Option<InitialSceneCommandKind>,
) -> InitialSceneClear {
    match first_command {
        Some(InitialSceneCommandKind::Regular) => InitialSceneClear::FirstRegularPass,
        Some(InitialSceneCommandKind::SamplesExistingTarget) => {
            InitialSceneClear::ExplicitBeforeTargetSampling
        }
        None => InitialSceneClear::ExplicitBeforePresent,
    }
}

fn collect_active_texture_keys(scene: &ScenePrimitives, keys: &mut HashSet<u64>) {
    keys.clear();
    keys.extend(
        scene
            .textures
            .iter()
            .chain(scene.overlay_textures.iter())
            .map(|texture| texture.texture.id()),
    );

    #[cfg(feature = "video")]
    {
        keys.extend(
            scene
                .video_textures
                .iter()
                .filter_map(|texture| texture.controller.current_frame().map(|frame| frame.id())),
        );
    }
    collect_texture_keys_from_commands(&scene.commands, keys);
    collect_texture_keys_from_commands(&scene.overlay_commands, keys);
}

fn cache_liveness_needs_refresh(
    last_scene_serial: Option<u64>,
    scene_serial: u64,
    has_dirty_draws: bool,
) -> bool {
    last_scene_serial != Some(scene_serial) || has_dirty_draws
}

fn collect_texture_keys_from_commands(commands: &[RenderCommand], keys: &mut HashSet<u64>) {
    for command in commands {
        match command {
            RenderCommand::Texture(texture) => {
                keys.insert(texture.texture.id());
            }
            RenderCommand::CanvasComposite(composite) => {
                collect_texture_keys_from_commands(&composite.content_commands, keys);
                if let Some(mask_commands) = composite.mask_commands.as_ref() {
                    collect_texture_keys_from_commands(mask_commands, keys);
                }
            }
            RenderCommand::BackdropBlur(_)
            | RenderCommand::Brush(_)
            | RenderCommand::Shape(_)
            | RenderCommand::TextDecoration(_)
            | RenderCommand::Text(_)
            | RenderCommand::Mesh(_) => {}
            #[cfg(feature = "video")]
            RenderCommand::VideoTexture(texture) => {
                if let Some(frame) = texture.controller.current_frame() {
                    keys.insert(frame.id());
                }
            }
        }
    }
}

pub struct Renderer {
    window: Option<Arc<dyn Window>>,
    surface: Option<wgpu::Surface<'static>>,
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
    snapshot_target: Option<OffscreenTarget>,
    blur_target: Option<OffscreenTarget>,
    blur_scratch_target: Option<OffscreenTarget>,
    canvas_composite_targets: Vec<OffscreenTarget>,
    canvas_composite_mask_targets: Vec<OffscreenTarget>,
    present_resources: Option<PresentResources>,
    clear_color: TguiColor,
    text_system: TextSystem,
    /// Reused CPU raster/upload storage for text cache misses.
    ///
    /// Rows are aligned for `Queue::write_texture`, then the non-transparent
    /// bounds are packed in-place before upload. Keeping the allocation here
    /// avoids a full-frame `Vec` allocation for every distinct text texture.
    text_upload_scratch: Vec<u8>,
    text_atlas: text::TextAtlas,
    text_cache: HashMap<TextCacheKey, TextCacheEntry>,
    texture_cache: HashMap<u64, TextureCacheEntry>,
    /// Monotonic identity for actual sprite bind-group instances.
    ///
    /// Texture IDs identify logical media and may survive a size change that recreates the GPU
    /// bind group. Draw-state caching therefore uses this renderer-local resource identity.
    next_sprite_bind_group_id: u64,
    active_text_keys_scratch: HashSet<TextCacheKey>,
    active_texture_keys_scratch: HashSet<u64>,
    /// Scene serial whose text/static-texture cache liveness was last scanned.
    ///
    /// Stable retained frames reuse the same prepared bind groups, so walking every
    /// nested command merely to rebuild identical key sets is unnecessary. Dirty
    /// draw ranges deliberately bypass this shortcut because direct scene splices
    /// keep the serial stable while replacing commands.
    cache_liveness_scene_serial: Option<u64>,
    vertex_pool: self::vertex_pool::VertexBufferPool,
    retained_prepare_cache: prepare::RetainedPrepareCache,
    /// Reuses the large frame-local prepared command arrays for retained main/overlay scenes.
    prepared_command_scratch: prepare::PreparedCommandScratch,
    /// Frame-local GPU-scroll translation lookups shared by main and overlay prepare streams.
    scroll_translate_cache: prepare::ScrollTranslateCache,
    /// Frame-local deduplication for identical mesh clip uniforms. Retained templates keep their
    /// cloned binding alive after this lookup table is cleared.
    mesh_clip_bind_group_cache: prepare::MeshClipBindGroupCache,
    last_prepare_stats: HashMap<prepare::DrawStream, prepare::PrepareReuseStats>,
    /// Benchmark/test-only switch and counters for an isomorphic unbatched control path.
    /// Production builds compile the switch away and always use safe contiguous sprite batching.
    #[cfg(any(test, feature = "bench-support"))]
    sprite_draw_batching_enabled: bool,
    #[cfg(any(test, feature = "bench-support"))]
    primitive_draw_batching_enabled: bool,
    #[cfg(any(test, feature = "bench-support"))]
    transparent_shape_skip_enabled: bool,
    #[cfg(any(test, feature = "bench-support"))]
    last_scene_draw_stats: draw::SceneDrawStats,
    #[cfg(any(test, feature = "bench-support"))]
    text_cache_hits: usize,
    #[cfg(any(test, feature = "bench-support"))]
    text_cache_misses: usize,
    #[cfg(any(test, feature = "bench-support"))]
    text_atlas_releases: usize,
    #[cfg(any(test, feature = "bench-support"))]
    text_prepare_cache_clears: usize,
    #[cfg(any(test, feature = "bench-support"))]
    text_alpha_cache_normalization_enabled: bool,
    /// Test/benchmark-only isomorphic control for paragraph mask textures plus vertex tint.
    #[cfg(any(test, feature = "bench-support"))]
    text_mask_tint_enabled: bool,
    /// Test/benchmark-only isomorphic control for R8 coverage atlas pages.
    #[cfg(any(test, feature = "bench-support"))]
    text_r8_atlas_enabled: bool,
    #[cfg(any(test, feature = "bench-support"))]
    text_atlas_deferred_upload_enabled: bool,
    /// Test/benchmark-only isomorphic control for the former all-float source-over loop.
    #[cfg(any(test, feature = "bench-support"))]
    text_blend_fast_path_enabled: bool,
    #[cfg(any(test, feature = "bench-support"))]
    text_blend_stats: text::TextBlendStats,
    /// 本次运行的 adapter 是否实际支持 IMMEDIATES。
    /// adapter 不支持时为 false——此时 GPU 平移变体运行时降级,滚动回退到
    /// CPU 子树重收集。
    push_constants_supported: bool,
}

impl Renderer {
    #[inline]
    fn sprite_draw_batching_enabled(&self) -> bool {
        #[cfg(any(test, feature = "bench-support"))]
        {
            self.sprite_draw_batching_enabled
        }
        #[cfg(not(any(test, feature = "bench-support")))]
        {
            true
        }
    }

    #[inline]
    fn primitive_draw_batching_enabled(&self) -> bool {
        #[cfg(any(test, feature = "bench-support"))]
        {
            self.primitive_draw_batching_enabled
        }
        #[cfg(not(any(test, feature = "bench-support")))]
        {
            true
        }
    }

    #[inline]
    fn transparent_shape_skip_enabled(&self) -> bool {
        #[cfg(any(test, feature = "bench-support"))]
        {
            self.transparent_shape_skip_enabled
        }
        #[cfg(not(any(test, feature = "bench-support")))]
        {
            true
        }
    }

    fn allocate_sprite_bind_group_id(&mut self) -> SpriteBindGroupId {
        let id = SpriteBindGroupId(self.next_sprite_bind_group_id);
        self.next_sprite_bind_group_id = self
            .next_sprite_bind_group_id
            .checked_add(1)
            .expect("tgui exhausted sprite bind-group identities");
        id
    }

    pub(crate) fn push_constants_supported(&self) -> bool {
        self.push_constants_supported
    }

    pub fn new(
        window: Arc<dyn Window>,
        clear_color: TguiColor,
        requested_msaa_mode: MsaaMode,
    ) -> Result<Box<Self>, TguiError> {
        pollster::block_on(Self::new_async(window, clear_color, requested_msaa_mode))
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>, scale_factor: f32) {
        let previous_scale_factor = self.scale_factor;
        let previous_size = self.size;
        if new_size.width == 0 || new_size.height == 0 {
            self.size = new_size;
            self.scale_factor = scale_factor.max(1.0 / 64.0);
            if self.size != previous_size || self.scale_factor != previous_scale_factor {
                self.cache_liveness_scene_serial = None;
            }
            return;
        }

        self.size = new_size;
        self.scale_factor = scale_factor.max(1.0 / 64.0);
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
        self.recreate_offscreen_targets();
        if self.size != previous_size || self.scale_factor != previous_scale_factor {
            self.cache_liveness_scene_serial = None;
        }
    }

    pub fn render(
        &mut self,
        scene: &mut ScenePrimitives,
        font_manager: &FontManager,
        scroll_regions: &[crate::ui::widget::ScrollRegion],
        transform_records: &HashMap<WidgetId, TransformRecord>,
    ) -> Result<RenderStatus, TguiError> {
        if self.config.width == 0 || self.config.height == 0 {
            return Ok(RenderStatus::SkipFrame);
        }
        let surface = self
            .surface
            .as_ref()
            .expect("window renderer must own a surface");
        let frame = match surface.get_current_texture() {
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

        self.render_to_view(
            scene,
            font_manager,
            scroll_regions,
            transform_records,
            &view,
        )?;
        self.window
            .as_ref()
            .expect("window renderer must own a window")
            .pre_present_notify();
        frame.present();

        Ok(RenderStatus::Rendered)
    }

    fn render_to_view(
        &mut self,
        scene: &mut ScenePrimitives,
        font_manager: &FontManager,
        scroll_regions: &[crate::ui::widget::ScrollRegion],
        transform_records: &HashMap<WidgetId, TransformRecord>,
        view: &wgpu::TextureView,
    ) -> Result<(), TguiError> {
        #[cfg(any(test, feature = "bench-support"))]
        {
            self.last_scene_draw_stats = draw::SceneDrawStats::default();
        }
        if self
            .text_system
            .prepare_font_system(font_manager.cache_identity())
        {
            // cosmic-text font IDs are database-local. A renderer used with a different manager
            // must not reuse either glyph rasters or whole-text textures from the old database.
            self.clear_text_cache();
            self.cache_liveness_scene_serial = None;
        }
        let previous_glyph_image_entries = self.text_system.swash_cache.image_cache.len();
        let viewport = self.vertex_viewport();
        let scene_serial = scene.prepare_cache_serial();
        let refresh_static_liveness = cache_liveness_needs_refresh(
            self.cache_liveness_scene_serial,
            scene_serial,
            !scene.dirty_draw_ranges().is_empty(),
        );

        #[cfg(feature = "video")]
        let refresh_texture_liveness = true;
        #[cfg(not(feature = "video"))]
        let refresh_texture_liveness = refresh_static_liveness;

        if refresh_texture_liveness {
            let mut active_texture_keys = std::mem::take(&mut self.active_texture_keys_scratch);
            collect_active_texture_keys(scene, &mut active_texture_keys);
            self.texture_cache
                .retain(|key, _| active_texture_keys.contains(key));
            self.active_texture_keys_scratch = active_texture_keys;
        }
        if refresh_static_liveness {
            self.retain_active_text_cache(scene);
            self.cache_liveness_scene_serial = Some(scene_serial);
        }

        // 推进到下一个轮转池缓冲并清空 staging；prepare_commands 会 bump-allocate 进来。
        self.vertex_pool.begin_frame();
        self.scroll_translate_cache.begin_frame();
        self.mesh_clip_bind_group_cache.begin_frame();

        let command_buffers = self.prepare_commands(
            prepare::DrawStream::Main,
            &scene.commands,
            font_manager,
            viewport,
            scroll_regions,
            scene.command_gpu_scroll_containers(),
            scene.command_transform_chains(),
            transform_records,
            scene.dirty_draw_ranges(),
            Some(scene.prepare_cache_serial()),
        )?;
        let overlay_buffers = match self.prepare_commands(
            prepare::DrawStream::Overlay,
            &scene.overlay_commands,
            font_manager,
            viewport,
            scroll_regions,
            scene.overlay_command_gpu_scroll_containers(),
            scene.overlay_command_transform_chains(),
            transform_records,
            scene.dirty_draw_ranges(),
            Some(scene.prepare_cache_serial()),
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.recycle_prepared_commands(command_buffers);
                self.text_system
                    .finish_raster_batch(previous_glyph_image_entries);
                return Err(error);
            }
        };
        self.text_system
            .finish_raster_batch(previous_glyph_image_entries);
        self.text_atlas.flush_pending_uploads(&self.queue);
        // 两次 prepare 的顶点数据都已进 staging，这里一次性上传到 GPU。
        self.vertex_pool.flush(&self.device, &self.queue);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tgui-render-encoder"),
            });

        // 普通场景帧让首个 draw pass 直接用 LoadOp::Clear，避免每帧额外
        // 编码一个只清屏的 render pass。Backdrop blur / canvas composite 会在首次
        // 绘制前采样目标，空场景也没有 draw pass，这两类情况仍必须显式清屏。
        let first_command = first_initial_scene_command(
            command_buffers
                .commands
                .first()
                .map(initial_scene_command_kind),
            overlay_buffers
                .commands
                .first()
                .map(initial_scene_command_kind),
        );
        let initial_clear = initial_scene_clear_strategy(first_command);
        let execute_result = (|| {
            if initial_clear.dedicated_pass_count() != 0 {
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

            let mut cleared_draw_target = initial_clear.dedicated_pass_count() != 0;
            self.execute_prepared_commands(
                &mut encoder,
                &command_buffers.commands,
                font_manager,
                &mut cleared_draw_target,
            )?;
            self.execute_prepared_commands(
                &mut encoder,
                &overlay_buffers.commands,
                font_manager,
                &mut cleared_draw_target,
            )
        })();
        self.recycle_prepared_commands(command_buffers);
        self.recycle_prepared_commands(overlay_buffers);
        execute_result?;
        self.blit_scene_to_surface(&mut encoder, view, None);

        self.queue.submit(Some(encoder.finish()));
        self.text_system.finish_frame();
        scene.clear_dirty_draw_ranges();
        Ok(())
    }

    pub fn set_clear_color(&mut self, clear_color: TguiColor) {
        self.clear_color = clear_color;
    }

    pub fn reconfigure(&mut self) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }

        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
        self.recreate_offscreen_targets();
    }

    fn retain_active_text_cache(&mut self, scene: &ScenePrimitives) {
        let mut active_text_keys = std::mem::take(&mut self.active_text_keys_scratch);
        active_text_keys.clear();
        self.collect_text_cache_keys_from_commands(&scene.commands, &mut active_text_keys);
        self.collect_text_cache_keys_from_commands(&scene.overlay_commands, &mut active_text_keys);
        let stale_keys = self
            .text_cache
            .keys()
            .filter(|key| !active_text_keys.contains(*key))
            .cloned()
            .collect::<Vec<_>>();
        let mut released_atlas_region = false;
        for key in stale_keys {
            let Some(entry) = self.text_cache.remove(&key) else {
                continue;
            };
            if let TextTextureStorage::Atlas(allocation) = entry.storage {
                self.text_atlas.release(allocation);
                released_atlas_region = true;
                #[cfg(any(test, feature = "bench-support"))]
                {
                    self.text_atlas_releases = self.text_atlas_releases.saturating_add(1);
                }
            }
        }
        if released_atlas_region {
            // A retained template stores both the atlas page binding and its UVs.
            // Reusing a released slot is therefore only safe after every template
            // that could still reference the previous occupant has been discarded.
            self.retained_prepare_cache.clear();
            #[cfg(any(test, feature = "bench-support"))]
            {
                self.text_prepare_cache_clears = self.text_prepare_cache_clears.saturating_add(1);
            }
        }
        self.active_text_keys_scratch = active_text_keys;
    }

    fn clear_text_cache(&mut self) {
        let cache = std::mem::take(&mut self.text_cache);
        for entry in cache.into_values() {
            if let TextTextureStorage::Atlas(allocation) = entry.storage {
                self.text_atlas.release(allocation);
                #[cfg(any(test, feature = "bench-support"))]
                {
                    self.text_atlas_releases = self.text_atlas_releases.saturating_add(1);
                }
            }
        }
        self.retained_prepare_cache.clear();
        #[cfg(any(test, feature = "bench-support"))]
        {
            self.text_prepare_cache_clears = self.text_prepare_cache_clears.saturating_add(1);
        }
    }

    fn collect_text_cache_keys_from_commands(
        &self,
        commands: &[RenderCommand],
        active_text_keys: &mut HashSet<TextCacheKey>,
    ) {
        for command in commands {
            match command {
                RenderCommand::Text(text) => {
                    if let Some(key) = self.text_cache_key(text) {
                        active_text_keys.insert(key);
                    }
                }
                RenderCommand::CanvasComposite(composite) => {
                    self.collect_text_cache_keys_from_commands(
                        &composite.content_commands,
                        active_text_keys,
                    );
                    if let Some(mask_commands) = composite.mask_commands.as_ref() {
                        self.collect_text_cache_keys_from_commands(mask_commands, active_text_keys);
                    }
                }
                RenderCommand::BackdropBlur(_)
                | RenderCommand::Brush(_)
                | RenderCommand::Shape(_)
                | RenderCommand::TextDecoration(_)
                | RenderCommand::Texture(_)
                | RenderCommand::Mesh(_) => {}
                #[cfg(feature = "video")]
                RenderCommand::VideoTexture(_) => {}
            }
        }
    }
}

#[cfg(feature = "bench-support")]
pub(crate) struct HeadlessBenchRenderer {
    renderer: Box<Renderer>,
    output_texture: wgpu::Texture,
    pub(crate) adapter_name: String,
    pub(crate) backend: String,
}

#[cfg(feature = "bench-support")]
impl HeadlessBenchRenderer {
    pub(crate) fn new(size: PhysicalSize<u32>) -> Result<Self, TguiError> {
        let (mut renderer, info) =
            pollster::block_on(Renderer::new_headless_for_bench(size, TguiColor::WHITE))?;
        let output_texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-bench-headless-output"),
            size: wgpu::Extent3d {
                width: renderer.config.width,
                height: renderer.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: renderer.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        // Make the first measured frame representative of retained steady state.
        renderer.cache_liveness_scene_serial = None;
        Ok(Self {
            renderer,
            output_texture,
            adapter_name: info.name,
            backend: format!("{:?}", info.backend),
        })
    }

    pub(crate) fn render_and_wait(
        &mut self,
        scene: &mut ScenePrimitives,
        font_manager: &FontManager,
        scroll_regions: &[crate::ui::widget::ScrollRegion],
        transform_records: &HashMap<WidgetId, TransformRecord>,
    ) -> Result<(), TguiError> {
        let view = self
            .output_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer.render_to_view(
            scene,
            font_manager,
            scroll_regions,
            transform_records,
            &view,
        )?;
        self.renderer
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| TguiError::TextRender(format!("headless GPU wait failed: {error}")))?;
        Ok(())
    }

    pub(crate) fn read_output_rgba(&self) -> Result<Vec<u8>, TguiError> {
        let width = self.renderer.config.width;
        let height = self.renderer.config.height;
        let unpadded_bytes_per_row = width.saturating_mul(4);
        let bytes_per_row = unpadded_bytes_per_row
            .div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .saturating_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tgui-bench-output-readback"),
            size: u64::from(bytes_per_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder =
            self.renderer
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("tgui-bench-output-readback"),
                });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.renderer.queue.submit(Some(encoder.finish()));
        let slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.renderer
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| TguiError::TextRender(format!("GPU readback wait failed: {error}")))?;
        receiver
            .recv()
            .map_err(|error| {
                TguiError::TextRender(format!("GPU readback callback failed: {error}"))
            })?
            .map_err(|error| {
                TguiError::TextRender(format!("GPU readback mapping failed: {error}"))
            })?;
        let mapped = slice.get_mapped_range();
        let mut rgba = vec![0; unpadded_bytes_per_row as usize * height as usize];
        let bgra = matches!(
            self.renderer.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        for row in 0..height as usize {
            let source = row * bytes_per_row as usize;
            let destination = row * unpadded_bytes_per_row as usize;
            rgba[destination..destination + unpadded_bytes_per_row as usize]
                .copy_from_slice(&mapped[source..source + unpadded_bytes_per_row as usize]);
            if bgra {
                for pixel in rgba[destination..destination + unpadded_bytes_per_row as usize]
                    .chunks_exact_mut(4)
                {
                    pixel.swap(0, 2);
                }
            }
        }
        drop(mapped);
        buffer.unmap();
        Ok(rgba)
    }

    pub(crate) fn text_gpu_cache_stats(
        &self,
    ) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
        let atlas_allocations = self
            .renderer
            .text_cache
            .values()
            .filter(|entry| matches!(entry.storage, TextTextureStorage::Atlas(_)))
            .count();
        let dedicated_textures = self.renderer.text_cache.len() - atlas_allocations;
        let unique_bind_groups = self
            .renderer
            .text_cache
            .values()
            .map(|entry| entry.draw.binding.id.0)
            .collect::<HashSet<_>>()
            .len();
        let r8_allocations = self
            .renderer
            .text_cache
            .values()
            .filter(|entry| {
                matches!(
                    entry.storage,
                    TextTextureStorage::Atlas(allocation)
                        if allocation.format == TextAtlasFormat::R8Coverage
                )
            })
            .count();
        let rgba_allocations = atlas_allocations - r8_allocations;
        let (r8_atlas_pages, rgba_atlas_pages) = self.renderer.text_atlas.page_counts_by_format();
        (
            self.renderer.text_cache.len(),
            self.renderer.text_atlas.page_count(),
            dedicated_textures,
            unique_bind_groups,
            r8_atlas_pages,
            rgba_atlas_pages,
            r8_allocations,
            rgba_allocations,
        )
    }

    pub(crate) fn text_raster_cache_stats(&self) -> TextRasterCacheStats {
        self.renderer.text_system.stats()
    }

    pub(crate) fn set_text_raster_cache_retention(&mut self, enabled: bool) {
        self.renderer.text_system.set_retention(enabled);
    }

    pub(crate) fn reset_text_raster_cache_stats(&mut self) {
        self.renderer.text_system.reset_stats();
    }

    pub(crate) fn text_cache_activity_stats(&self) -> (usize, usize, usize, usize) {
        (
            self.renderer.text_cache_hits,
            self.renderer.text_cache_misses,
            self.renderer.text_atlas_releases,
            self.renderer.text_prepare_cache_clears,
        )
    }

    pub(crate) fn text_atlas_upload_stats(
        &self,
    ) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
        let stats = self.renderer.text_atlas.upload_stats();
        (
            stats.write_calls,
            stats.uploaded_bytes,
            stats.shadow_bytes,
            stats.shadow_budget_bytes,
            stats.r8_uploaded_bytes,
            stats.rgba_uploaded_bytes,
            stats.r8_shadow_bytes,
            stats.rgba_shadow_bytes,
        )
    }

    pub(crate) fn reset_text_atlas_upload_stats(&mut self) {
        self.renderer.text_atlas.reset_upload_stats();
    }

    pub(crate) fn text_blend_stats(&self) -> text::TextBlendStats {
        self.renderer.text_blend_stats
    }

    pub(crate) fn reset_text_blend_stats(&mut self) {
        self.renderer.text_blend_stats = text::TextBlendStats::default();
    }

    pub(crate) fn set_text_blend_fast_path(&mut self, enabled: bool) {
        if self.renderer.text_blend_fast_path_enabled == enabled {
            return;
        }
        self.renderer.text_blend_fast_path_enabled = enabled;
        self.renderer.clear_text_cache();
        self.renderer.cache_liveness_scene_serial = None;
    }

    pub(crate) fn set_text_atlas_deferred_upload(&mut self, enabled: bool) {
        if self.renderer.text_atlas_deferred_upload_enabled == enabled {
            return;
        }
        self.renderer.clear_text_cache();
        self.renderer.text_atlas_deferred_upload_enabled = enabled;
        self.renderer.cache_liveness_scene_serial = None;
    }

    pub(crate) fn reset_text_cache_activity_stats(&mut self) {
        self.renderer.text_cache_hits = 0;
        self.renderer.text_cache_misses = 0;
        self.renderer.text_atlas_releases = 0;
        self.renderer.text_prepare_cache_clears = 0;
    }

    pub(crate) fn set_text_alpha_cache_normalization(&mut self, enabled: bool) {
        if self.renderer.text_alpha_cache_normalization_enabled == enabled {
            return;
        }
        self.renderer.text_alpha_cache_normalization_enabled = enabled;
        self.renderer.clear_text_cache();
        self.renderer.cache_liveness_scene_serial = None;
    }

    pub(crate) fn set_text_mask_tint(&mut self, enabled: bool) {
        if self.renderer.text_mask_tint_enabled == enabled {
            return;
        }
        self.renderer.text_mask_tint_enabled = enabled;
        self.renderer.clear_text_cache();
        self.renderer.cache_liveness_scene_serial = None;
    }

    pub(crate) fn set_text_r8_atlas(&mut self, enabled: bool) {
        if self.renderer.text_r8_atlas_enabled == enabled {
            return;
        }
        self.renderer.text_r8_atlas_enabled = enabled;
        self.renderer.clear_text_cache();
        self.renderer.cache_liveness_scene_serial = None;
    }

    pub(crate) fn set_sprite_draw_batching(&mut self, enabled: bool) {
        self.renderer.sprite_draw_batching_enabled = enabled;
    }

    pub(crate) fn set_primitive_draw_batching(&mut self, enabled: bool) {
        self.renderer.primitive_draw_batching_enabled = enabled;
    }

    pub(crate) fn set_transparent_shape_skip(&mut self, enabled: bool) {
        self.renderer.transparent_shape_skip_enabled = enabled;
        self.renderer.retained_prepare_cache.clear();
    }

    pub(crate) fn scene_draw_stats(
        &self,
    ) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
        let stats = self.renderer.last_scene_draw_stats;
        (
            stats.rect_commands,
            stats.rect_draw_calls,
            stats.brush_commands,
            stats.brush_draw_calls,
            stats.mesh_commands,
            stats.mesh_draw_calls,
            stats.sprite_commands,
            stats.sprite_draw_calls,
        )
    }

    pub(crate) fn clear_text_gpu_cache(&mut self) {
        self.renderer.clear_text_cache();
    }
}

#[cfg(test)]
mod tests;
