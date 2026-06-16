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
    MeshVertex, RectVertex, TextQuadSpec, TextTransformSpec, TextVertex, VertexViewport,
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

fn active_texture_keys(scene: &ScenePrimitives) -> HashSet<u64> {
    let mut keys: HashSet<_> = scene
        .textures
        .iter()
        .chain(scene.overlay_textures.iter())
        .map(|texture| texture.texture.id())
        .collect();

    #[cfg(feature = "video")]
    {
        keys.extend(
            scene
                .video_textures
                .iter()
                .filter_map(|texture| texture.controller.current_frame().map(|frame| frame.id())),
        );
    }
    collect_texture_keys_from_commands(&scene.commands, &mut keys);
    collect_texture_keys_from_commands(&scene.overlay_commands, &mut keys);
    keys
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
    snapshot_target: Option<OffscreenTarget>,
    blur_target: Option<OffscreenTarget>,
    blur_scratch_target: Option<OffscreenTarget>,
    canvas_composite_targets: Vec<OffscreenTarget>,
    canvas_composite_mask_targets: Vec<OffscreenTarget>,
    clear_color: TguiColor,
    text_system: TextSystem,
    text_cache: HashMap<TextCacheKey, TextCacheEntry>,
    texture_cache: HashMap<u64, TextureCacheEntry>,
    vertex_pool: self::vertex_pool::VertexBufferPool,
    /// 本次运行的 adapter 是否实际支持 IMMEDIATES。
    /// adapter 不支持时为 false——此时 GPU 平移变体运行时降级,滚动回退到
    /// CPU 子树重收集。
    push_constants_supported: bool,
}

impl Renderer {
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

    pub fn render(
        &mut self,
        scene: &ScenePrimitives,
        font_manager: &FontManager,
        scroll_regions: &[crate::ui::widget::ScrollRegion],
        transform_records: &HashMap<WidgetId, TransformRecord>,
    ) -> Result<RenderStatus, TguiError> {
        if self.config.width == 0 || self.config.height == 0 {
            return Ok(RenderStatus::SkipFrame);
        }
        let viewport = self.vertex_viewport();

        let active_texture_keys = active_texture_keys(scene);
        self.texture_cache
            .retain(|key, _| active_texture_keys.contains(key));
        self.retain_active_text_cache(scene);

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

        // 推进到下一个轮转池缓冲并清空 staging；prepare_commands 会 bump-allocate 进来。
        self.vertex_pool.begin_frame();

        let command_buffers = self.prepare_commands(
            &scene.commands,
            font_manager,
            viewport,
            scroll_regions,
            scene.command_gpu_scroll_containers(),
            scene.command_transform_chains(),
            transform_records,
        )?;
        let overlay_buffers = self.prepare_commands(
            &scene.overlay_commands,
            font_manager,
            viewport,
            scroll_regions,
            scene.overlay_command_gpu_scroll_containers(),
            scene.overlay_command_transform_chains(),
            transform_records,
        )?;
        // 两次 prepare 的顶点数据都已进 staging，这里一次性上传到 GPU。
        self.vertex_pool.flush(&self.device, &self.queue);
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

        let mut cleared_draw_target = true;
        self.execute_prepared_commands(
            &mut encoder,
            &command_buffers.0,
            font_manager,
            &mut cleared_draw_target,
        )?;
        self.execute_prepared_commands(
            &mut encoder,
            &overlay_buffers.0,
            font_manager,
            &mut cleared_draw_target,
        )?;
        let scene_target = self
            .scene_target
            .as_ref()
            .ok_or_else(|| TguiError::TextRender("scene target unavailable".into()))?;
        let scene_view = self.offscreen_sampled_view(scene_target);
        self.blit_scene_to_surface(&mut encoder, scene_view, &color_attachment_view, None);

        self.queue.submit(Some(encoder.finish()));
        self.text_system.release_frame_raster_cache();
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

    fn retain_active_text_cache(&mut self, scene: &ScenePrimitives) {
        let mut active_text_keys = HashSet::new();
        self.collect_text_cache_keys_from_commands(&scene.commands, &mut active_text_keys);
        self.collect_text_cache_keys_from_commands(&scene.overlay_commands, &mut active_text_keys);
        self.text_cache
            .retain(|key, _| active_text_keys.contains(key));
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

#[cfg(test)]
mod tests;
