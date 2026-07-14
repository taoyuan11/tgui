use wgpu::util::DeviceExt;

use crate::foundation::error::TguiError;
use crate::text::font::FontManager;
use crate::ui::unit::Dp;
use crate::ui::widget::{
    BrushPrimitiveData, CanvasCompositePrimitive, DirtyDrawRange, Rect, RenderCommand,
    RenderPrimitive, SceneDrawStream, TransformChain, TransformRecord, WidgetId,
};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::sync::Arc;

use super::{
    physical_mesh_clip_mask_data, BrushVertex, BrushVertexSpec, CompositeQuadSpec, CompositeVertex,
    MeshClipBindGroup, MeshClipBindGroupId, MeshClipMaskUniformData, MeshVertex, RectVertex,
    Renderer, SpriteBindGroup, TextQuadSpec, TextTransformSpec, TextVertex, VertexViewport,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum DrawStream {
    Main,
    Overlay,
    CompositeContent { depth: usize },
    CompositeMask { depth: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct DrawId {
    pub(super) stream: DrawStream,
    pub(super) command_index: usize,
}

impl DrawId {
    fn new(stream: DrawStream, command_index: usize) -> Self {
        Self {
            stream,
            command_index,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct PrepareReuseStats {
    pub(super) total: usize,
    pub(super) rebuild: usize,
    pub(super) reuse: usize,
}

#[cfg(test)]
pub(super) fn retained_prepare_stats(
    stream: DrawStream,
    command_count: usize,
    dirty_ranges: &[DirtyDrawRange],
) -> PrepareReuseStats {
    let Some(scene_stream) = stream.scene_stream() else {
        return PrepareReuseStats {
            total: command_count,
            rebuild: command_count,
            reuse: 0,
        };
    };

    let dirty = dirty_command_mask(scene_stream, command_count, dirty_ranges);
    let rebuild = dirty.into_iter().filter(|dirty| *dirty).count();
    PrepareReuseStats {
        total: command_count,
        rebuild,
        reuse: command_count.saturating_sub(rebuild),
    }
}

fn dirty_command_mask(
    scene_stream: SceneDrawStream,
    command_count: usize,
    dirty_ranges: &[DirtyDrawRange],
) -> Vec<bool> {
    let mut dirty = vec![false; command_count];
    for range in dirty_ranges
        .iter()
        .filter(|range| range.stream == scene_stream)
    {
        let start = range.range.start.min(command_count);
        let end = range.range.end.min(command_count);
        for slot in &mut dirty[start..end] {
            *slot = true;
        }
    }
    dirty
}

impl DrawStream {
    fn scene_stream(self) -> Option<SceneDrawStream> {
        match self {
            DrawStream::Main => Some(SceneDrawStream::Main),
            DrawStream::Overlay => Some(SceneDrawStream::Overlay),
            DrawStream::CompositeContent { .. } | DrawStream::CompositeMask { .. } => None,
        }
    }
}

#[cfg(test)]
fn compute_scroll_translate(
    gpu_scroll_container: Option<WidgetId>,
    scroll_regions: &[crate::ui::widget::ScrollRegion],
    viewport: VertexViewport,
) -> Option<super::PushTranslate> {
    let gpu_scroll_container = gpu_scroll_container?;
    let region = scroll_regions
        .iter()
        .rev()
        .find(|region| region.id == gpu_scroll_container)?;

    translate_from_logical_movement(
        crate::ui::widget::Point {
            x: region.gpu_base_scroll_offset.x - region.scroll_offset.x,
            y: region.gpu_base_scroll_offset.y - region.scroll_offset.y,
        },
        viewport,
    )
}

/// Per-frame memoization for GPU-scroll draw translations.
///
/// A retained scroll subtree can contain thousands of draw commands tagged with the same
/// container id. Looking that id up with `scroll_regions.iter().rev().find(...)` for every draw
/// turns prepare into `O(draws * regions)`. Scroll offsets are immutable for the duration of one
/// render call, so each distinct id only needs one scan; main and overlay streams share this cache.
#[derive(Default)]
pub(super) struct ScrollTranslateCache {
    // The GPU-scroll fast path currently admits a single container. Keep a few entries inline so
    // the common repeated lookup is one integer comparison and performs no heap allocation; the
    // small vector still preserves correct behavior if that eligibility expands later.
    translations: SmallVec<[(WidgetId, Option<super::PushTranslate>); 4]>,
    #[cfg(test)]
    region_visits: usize,
}

impl ScrollTranslateCache {
    pub(super) fn begin_frame(&mut self) {
        self.translations.clear();
        #[cfg(test)]
        {
            self.region_visits = 0;
        }
    }

    fn resolve(
        &mut self,
        gpu_scroll_container: Option<WidgetId>,
        scroll_regions: &[crate::ui::widget::ScrollRegion],
        viewport: VertexViewport,
    ) -> Option<super::PushTranslate> {
        let gpu_scroll_container = gpu_scroll_container?;
        if let Some((_, translate)) = self
            .translations
            .iter()
            .find(|(id, _)| *id == gpu_scroll_container)
        {
            return *translate;
        }

        let mut matched = None;
        for region in scroll_regions.iter().rev() {
            #[cfg(test)]
            {
                self.region_visits += 1;
            }
            if region.id == gpu_scroll_container {
                matched = translate_from_logical_movement(
                    crate::ui::widget::Point {
                        x: region.gpu_base_scroll_offset.x - region.scroll_offset.x,
                        y: region.gpu_base_scroll_offset.y - region.scroll_offset.y,
                    },
                    viewport,
                );
                break;
            }
        }
        self.translations.push((gpu_scroll_container, matched));
        matched
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MeshClipKey([u32; 8]);

fn mesh_clip_key(data: MeshClipMaskUniformData) -> MeshClipKey {
    MeshClipKey(bytemuck::cast(data))
}

#[derive(Default)]
pub(super) struct MeshClipBindGroupCache {
    bindings: HashMap<MeshClipKey, MeshClipBindGroup>,
    next_id: u64,
    #[cfg(test)]
    creations: usize,
}

impl MeshClipBindGroupCache {
    pub(super) fn begin_frame(&mut self) {
        self.bindings.clear();
        #[cfg(test)]
        {
            self.creations = 0;
        }
    }

    fn binding_for(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        data: MeshClipMaskUniformData,
    ) -> MeshClipBindGroup {
        let key = mesh_clip_key(data);
        if let Some(binding) = self.bindings.get(&key) {
            return binding.clone();
        }

        let clip_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tgui-mesh-clip-uniform"),
            contents: bytemuck::bytes_of(&data),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-mesh-clip-bind-group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: clip_buffer.as_entire_binding(),
            }],
        });
        let binding = MeshClipBindGroup {
            id: MeshClipBindGroupId(self.next_id),
            bind_group,
        };
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("tgui exhausted mesh clip bind-group identities");
        #[cfg(test)]
        {
            self.creations += 1;
        }
        self.bindings.insert(key, binding.clone());
        binding
    }
}

fn compute_transform_translate(
    transform_chain: Option<&TransformChain>,
    transform_records: &HashMap<WidgetId, TransformRecord>,
    viewport: VertexViewport,
) -> Option<super::PushTranslate> {
    let transform_chain = transform_chain?;
    let mut movement = crate::ui::widget::Point::ZERO;
    for id in transform_chain {
        if let Some(record) = transform_records.get(id) {
            let delta = (*record).delta();
            movement.x += delta.x;
            movement.y += delta.y;
        }
    }
    translate_from_logical_movement(movement, viewport)
}

fn translate_from_logical_movement(
    movement: crate::ui::widget::Point,
    viewport: VertexViewport,
) -> Option<super::PushTranslate> {
    if movement.x.abs() < Dp::new(0.01) && movement.y.abs() < Dp::new(0.01) {
        return None;
    }

    let physical_x = movement.x.get() * viewport.scale_factor;
    let physical_y = movement.y.get() * viewport.scale_factor;
    Some(super::PushTranslate {
        offset_ndc: [
            2.0 * physical_x / viewport.physical_size[0],
            -2.0 * physical_y / viewport.physical_size[1],
        ],
        offset_physical: [physical_x, physical_y],
    })
}

fn combine_translates(
    first: Option<super::PushTranslate>,
    second: Option<super::PushTranslate>,
) -> Option<super::PushTranslate> {
    match (first, second) {
        (None, None) => None,
        (Some(translate), None) | (None, Some(translate)) => Some(translate),
        (Some(first), Some(second)) => Some(super::PushTranslate {
            offset_ndc: [
                first.offset_ndc[0] + second.offset_ndc[0],
                first.offset_ndc[1] + second.offset_ndc[1],
            ],
            offset_physical: [
                first.offset_physical[0] + second.offset_physical[0],
                first.offset_physical[1] + second.offset_physical[1],
            ],
        }),
    }
}

fn texture_quad_vertices(
    rect: Rect,
    quad: Option<[crate::ui::widget::Point; 4]>,
    uv_rect: Option<Rect>,
    corner_radius: f32,
    clip_mask: Option<crate::ui::widget::ClipMask>,
    opacity: f32,
    tint: [u8; 4],
    viewport: VertexViewport,
) -> [TextVertex; 6] {
    match quad {
        Some(quad) => TextVertex::transformed(
            TextTransformSpec {
                rect,
                quad,
                uv_rect,
                corner_radius,
                clip_mask,
                opacity,
                tint,
            },
            viewport,
        ),
        None => TextVertex::quad(
            TextQuadSpec {
                rect,
                uv_rect,
                corner_radius,
                clip_mask,
                opacity,
                tint,
            },
            viewport,
        ),
    }
}

fn retained_prepare_cacheable(command: &RenderCommand) -> bool {
    match command {
        RenderCommand::BackdropBlur(_)
        | RenderCommand::Brush(_)
        | RenderCommand::CanvasComposite(_)
        | RenderCommand::Shape(_)
        | RenderCommand::TextDecoration(_)
        | RenderCommand::Mesh(_)
        | RenderCommand::Texture(_)
        | RenderCommand::Text(_) => true,
        #[cfg(feature = "video")]
        RenderCommand::VideoTexture(_) => false,
    }
}

fn should_prepare_shape(primitive: &RenderPrimitive, skip_transparent: bool) -> bool {
    primitive.rect.width > Dp::ZERO
        && primitive.rect.height > Dp::ZERO
        && (!skip_transparent || primitive.color.a != 0)
}

pub(super) struct PreparedRect {
    pub(super) draw_id: DrawId,
    pub(super) clip_rect: Option<Rect>,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
    /// 该 draw 所属滚动容器的平移量。非滚动内容为 None。
    pub(super) scroll_translate: Option<super::PushTranslate>,
}

pub(super) struct PreparedBrush {
    pub(super) draw_id: DrawId,
    pub(super) clip_rect: Option<Rect>,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
    pub(super) scroll_translate: Option<super::PushTranslate>,
}

pub(super) struct PreparedMesh {
    pub(super) draw_id: DrawId,
    pub(super) clip_rect: Option<Rect>,
    pub(super) clip_binding: MeshClipBindGroup,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
    pub(super) scroll_translate: Option<super::PushTranslate>,
}

pub(super) struct PreparedSprite {
    pub(super) draw_id: DrawId,
    pub(super) pipeline: PreparedSpritePipeline,
    pub(super) binding: SpriteBindGroup,
    pub(super) clip_rect: Option<Rect>,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
    pub(super) scroll_translate: Option<super::PushTranslate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PreparedSpritePipeline {
    Rgba,
    #[cfg(feature = "video")]
    VideoYuv,
}

pub(super) struct PreparedBackdropBlur {
    pub(super) draw_id: DrawId,
    pub(super) primitive: crate::ui::widget::BackdropBlurPrimitive,
    pub(super) composite_offset: u64,
    pub(super) composite_vertex_count: u32,
    pub(super) fullscreen_offset: u64,
    pub(super) fullscreen_vertex_count: u32,
}

pub(super) struct PreparedCanvasComposite {
    pub(super) draw_id: DrawId,
    pub(super) primitive: Arc<CanvasCompositePrimitive>,
    pub(super) composite_offset: u64,
    pub(super) composite_vertex_count: u32,
}

pub(super) enum PreparedCommand {
    BackdropBlur(PreparedBackdropBlur),
    Rect(PreparedRect),
    Brush(PreparedBrush),
    CanvasComposite(PreparedCanvasComposite),
    Mesh(PreparedMesh),
    Sprite(PreparedSprite),
}

impl PreparedCommand {
    pub(super) fn draw_id(&self) -> DrawId {
        match self {
            Self::BackdropBlur(command) => command.draw_id,
            Self::Rect(command) => command.draw_id,
            Self::Brush(command) => command.draw_id,
            Self::CanvasComposite(command) => command.draw_id,
            Self::Mesh(command) => command.draw_id,
            Self::Sprite(command) => command.draw_id,
        }
    }
}

pub(super) struct PreparedCommands {
    pub(super) stream: DrawStream,
    pub(super) commands: Vec<PreparedCommand>,
}

/// Reusable output storage for the two top-level scene streams.
///
/// Retained prepare avoids rebuilding vertex templates, but the prepared command list is still
/// materialized every frame because it carries frame-local vertex offsets and translations. A
/// large retained scene therefore used to allocate and free a correspondingly large `Vec` on
/// every stable frame. Main and overlay cannot be recursively re-entered while their output is in
/// use, so one retained buffer per stream removes that allocator traffic without retaining rare
/// canvas-composite peak storage.
#[derive(Default)]
pub(super) struct PreparedCommandScratch {
    main: Vec<PreparedCommand>,
    overlay: Vec<PreparedCommand>,
    #[cfg(test)]
    storage_growths: usize,
}

impl PreparedCommandScratch {
    fn acquire(&mut self, stream: DrawStream, command_count: usize) -> Vec<PreparedCommand> {
        let mut commands = match stream {
            DrawStream::Main => std::mem::take(&mut self.main),
            DrawStream::Overlay => std::mem::take(&mut self.overlay),
            DrawStream::CompositeContent { .. } | DrawStream::CompositeMask { .. } => {
                return Vec::with_capacity(command_count);
            }
        };
        commands.clear();
        if commands.capacity() < command_count {
            #[cfg(test)]
            {
                self.storage_growths += 1;
            }
            commands.reserve(command_count);
        }
        commands
    }

    fn recycle(&mut self, stream: DrawStream, mut commands: Vec<PreparedCommand>) {
        commands.clear();
        match stream {
            DrawStream::Main => self.main = commands,
            DrawStream::Overlay => self.overlay = commands,
            DrawStream::CompositeContent { .. } | DrawStream::CompositeMask { .. } => {}
        }
    }
}

#[cfg(test)]
pub(super) fn prepared_command_scratch_storage_probe(
    peak_commands: usize,
    stable_frames: usize,
) -> (usize, usize, usize, bool) {
    let mut scratch = PreparedCommandScratch::default();
    let mut first_storage = None;
    let mut same_storage = true;

    for _ in 0..stable_frames {
        let mut commands = scratch.acquire(DrawStream::Main, peak_commands);
        let storage = commands.as_ptr();
        if let Some(first_storage) = first_storage {
            same_storage &= storage == first_storage;
        } else {
            first_storage = Some(storage);
        }
        for command_index in 0..peak_commands {
            commands.push(PreparedCommand::Rect(PreparedRect {
                draw_id: DrawId::new(DrawStream::Main, command_index),
                clip_rect: None,
                vertex_offset: 0,
                vertex_count: 0,
                scroll_translate: None,
            }));
        }
        scratch.recycle(DrawStream::Main, commands);
    }

    let capacity = scratch.main.capacity();
    (
        scratch.storage_growths,
        capacity,
        capacity * std::mem::size_of::<PreparedCommand>(),
        same_storage,
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PrepareStreamSignature {
    scene_serial: u64,
    viewport: VertexViewport,
    command_count: usize,
}

#[derive(Clone)]
enum PreparedCommandTemplate {
    BackdropBlur {
        primitive: crate::ui::widget::BackdropBlurPrimitive,
        composite_vertices: Vec<u8>,
        composite_vertex_count: u32,
        fullscreen_vertices: Vec<u8>,
        fullscreen_vertex_count: u32,
    },
    Rect {
        clip_rect: Option<Rect>,
        vertices: Vec<u8>,
        vertex_count: u32,
    },
    Brush {
        clip_rect: Option<Rect>,
        vertices: Vec<u8>,
        vertex_count: u32,
    },
    CanvasComposite {
        primitive: Arc<CanvasCompositePrimitive>,
        vertices: Vec<u8>,
        vertex_count: u32,
    },
    Mesh {
        clip_rect: Option<Rect>,
        clip_binding: MeshClipBindGroup,
        vertices: Vec<u8>,
        vertex_count: u32,
    },
    Sprite {
        pipeline: PreparedSpritePipeline,
        binding: SpriteBindGroup,
        clip_rect: Option<Rect>,
        vertices: Vec<u8>,
        vertex_count: u32,
    },
}

impl PreparedCommandTemplate {
    fn prepare(
        &self,
        draw_id: DrawId,
        vertex_pool: &mut super::vertex_pool::VertexBufferPool,
        scroll_translate: Option<super::PushTranslate>,
    ) -> PreparedCommand {
        match self {
            Self::BackdropBlur {
                primitive,
                composite_vertices,
                composite_vertex_count,
                fullscreen_vertices,
                fullscreen_vertex_count,
            } => PreparedCommand::BackdropBlur(PreparedBackdropBlur {
                draw_id,
                primitive: *primitive,
                composite_offset: vertex_pool.allocate(composite_vertices),
                composite_vertex_count: *composite_vertex_count,
                fullscreen_offset: vertex_pool.allocate(fullscreen_vertices),
                fullscreen_vertex_count: *fullscreen_vertex_count,
            }),
            Self::Rect {
                clip_rect,
                vertices,
                vertex_count,
            } => PreparedCommand::Rect(PreparedRect {
                draw_id,
                clip_rect: *clip_rect,
                vertex_offset: vertex_pool
                    .allocate_aligned(vertices, std::mem::size_of::<RectVertex>() as u64),
                vertex_count: *vertex_count,
                scroll_translate,
            }),
            Self::Brush {
                clip_rect,
                vertices,
                vertex_count,
            } => PreparedCommand::Brush(PreparedBrush {
                draw_id,
                clip_rect: *clip_rect,
                vertex_offset: vertex_pool
                    .allocate_aligned(vertices, std::mem::size_of::<BrushVertex>() as u64),
                vertex_count: *vertex_count,
                scroll_translate,
            }),
            Self::CanvasComposite {
                primitive,
                vertices,
                vertex_count,
            } => PreparedCommand::CanvasComposite(PreparedCanvasComposite {
                draw_id,
                primitive: Arc::clone(primitive),
                composite_offset: vertex_pool.allocate(vertices),
                composite_vertex_count: *vertex_count,
            }),
            Self::Mesh {
                clip_rect,
                clip_binding,
                vertices,
                vertex_count,
            } => PreparedCommand::Mesh(PreparedMesh {
                draw_id,
                clip_rect: *clip_rect,
                clip_binding: clip_binding.clone(),
                vertex_offset: vertex_pool
                    .allocate_aligned(vertices, std::mem::size_of::<MeshVertex>() as u64),
                vertex_count: *vertex_count,
                scroll_translate,
            }),
            Self::Sprite {
                pipeline,
                binding,
                clip_rect,
                vertices,
                vertex_count,
            } => PreparedCommand::Sprite(PreparedSprite {
                draw_id,
                pipeline: *pipeline,
                binding: binding.clone(),
                clip_rect: *clip_rect,
                vertex_offset: vertex_pool
                    .allocate_aligned(vertices, std::mem::size_of::<TextVertex>() as u64),
                vertex_count: *vertex_count,
                scroll_translate,
            }),
        }
    }
}

struct PreparedTemplateBuild {
    template: PreparedCommandTemplate,
    cacheable: bool,
}

struct PrepareStreamCache {
    signature: PrepareStreamSignature,
    templates: Vec<Option<PreparedCommandTemplate>>,
}

impl PrepareStreamCache {
    fn new(signature: PrepareStreamSignature) -> Self {
        Self {
            signature,
            templates: vec![None; signature.command_count],
        }
    }
}

#[derive(Default)]
pub(super) struct RetainedPrepareCache {
    streams: HashMap<DrawStream, PrepareStreamCache>,
}

impl RetainedPrepareCache {
    pub(super) fn clear(&mut self) {
        self.streams.clear();
    }

    fn stream_reusable(&self, stream: DrawStream, signature: PrepareStreamSignature) -> bool {
        self.streams
            .get(&stream)
            .map(|cache| cache.signature == signature)
            .unwrap_or(false)
    }

    fn ensure_stream(&mut self, stream: DrawStream, signature: PrepareStreamSignature) {
        let reset = self
            .streams
            .get(&stream)
            .map(|cache| cache.signature != signature)
            .unwrap_or(true);
        if reset {
            self.streams
                .insert(stream, PrepareStreamCache::new(signature));
        }
    }

    fn template(
        &self,
        stream: DrawStream,
        command_index: usize,
    ) -> Option<&PreparedCommandTemplate> {
        self.streams
            .get(&stream)
            .and_then(|cache| cache.templates.get(command_index))
            .and_then(|template| template.as_ref())
    }

    fn store_template(
        &mut self,
        stream: DrawStream,
        command_index: usize,
        template: Option<PreparedCommandTemplate>,
    ) {
        let Some(cache) = self.streams.get_mut(&stream) else {
            return;
        };
        if let Some(slot) = cache.templates.get_mut(command_index) {
            *slot = template;
        }
    }
}

#[cfg(test)]
pub(super) fn retained_cache_lookup_storage_probe(payload_bytes: usize) -> (bool, usize) {
    let stream = DrawStream::Main;
    let signature = PrepareStreamSignature {
        scene_serial: 7,
        viewport: VertexViewport::new(100.0, 100.0, 100.0, 100.0, 1.0),
        command_count: 1,
    };
    let mut cache = RetainedPrepareCache::default();
    cache.ensure_stream(stream, signature);
    cache.store_template(
        stream,
        0,
        Some(PreparedCommandTemplate::Rect {
            clip_rect: None,
            vertices: vec![1; payload_bytes],
            vertex_count: 1,
        }),
    );

    let first = match cache.template(stream, 0) {
        Some(PreparedCommandTemplate::Rect { vertices, .. }) => vertices.as_ptr(),
        _ => return (false, 0),
    };
    let (second, stored_bytes) = match cache.template(stream, 0) {
        Some(PreparedCommandTemplate::Rect { vertices, .. }) => (vertices.as_ptr(), vertices.len()),
        _ => return (false, 0),
    };
    (first == second, stored_bytes)
}

impl Renderer {
    pub(super) fn prepare_commands(
        &mut self,
        stream: DrawStream,
        commands: &[RenderCommand],
        font_manager: &FontManager,
        viewport: VertexViewport,
        scroll_regions: &[crate::ui::widget::ScrollRegion],
        command_gpu_scroll_containers: &[Option<WidgetId>],
        command_transform_chains: &[TransformChain],
        transform_records: &HashMap<WidgetId, TransformRecord>,
        dirty_ranges: &[DirtyDrawRange],
        prepare_cache_serial: Option<u64>,
    ) -> Result<PreparedCommands, TguiError> {
        let scene_stream = stream.scene_stream();
        let signature = scene_stream.and_then(|_| {
            prepare_cache_serial.map(|scene_serial| PrepareStreamSignature {
                scene_serial,
                viewport,
                command_count: commands.len(),
            })
        });
        let stream_reusable = signature
            .map(|signature| {
                self.retained_prepare_cache
                    .stream_reusable(stream, signature)
            })
            .unwrap_or(false);
        if let Some(signature) = signature {
            self.retained_prepare_cache.ensure_stream(stream, signature);
        }
        let dirty_commands = scene_stream
            .map(|scene_stream| dirty_command_mask(scene_stream, commands.len(), dirty_ranges))
            .unwrap_or_else(|| vec![true; commands.len()]);
        let mut stats = PrepareReuseStats {
            total: commands.len(),
            rebuild: 0,
            reuse: 0,
        };
        let mut prepared = self
            .prepared_command_scratch
            .acquire(stream, commands.len());

        for (command_index, command) in commands.iter().enumerate() {
            let draw_id = DrawId::new(stream, command_index);
            let gpu_scroll_container = command_gpu_scroll_containers
                .get(command_index)
                .copied()
                .flatten();
            let draw_translate = combine_translates(
                self.scroll_translate_cache
                    .resolve(gpu_scroll_container, scroll_regions, viewport),
                compute_transform_translate(
                    command_transform_chains.get(command_index),
                    transform_records,
                    viewport,
                ),
            );

            if stream_reusable
                && !dirty_commands.get(command_index).copied().unwrap_or(true)
                && retained_prepare_cacheable(command)
            {
                let reused = {
                    let cache = &self.retained_prepare_cache;
                    let vertex_pool = &mut self.vertex_pool;
                    cache
                        .template(stream, command_index)
                        .map(|template| template.prepare(draw_id, vertex_pool, draw_translate))
                };
                if let Some(command) = reused {
                    stats.reuse += 1;
                    prepared.push(command);
                    continue;
                }
            }

            stats.rebuild += 1;
            let build = match self.build_prepared_template(command, font_manager, viewport) {
                Ok(build) => build,
                Err(error) => {
                    self.prepared_command_scratch.recycle(stream, prepared);
                    return Err(error);
                }
            };
            let Some(build) = build else {
                if signature.is_some() {
                    self.retained_prepare_cache
                        .store_template(stream, command_index, None);
                }
                continue;
            };
            prepared.push(
                build
                    .template
                    .prepare(draw_id, &mut self.vertex_pool, draw_translate),
            );
            if signature.is_some() {
                self.retained_prepare_cache.store_template(
                    stream,
                    command_index,
                    build.cacheable.then_some(build.template),
                );
            }
        }

        self.last_prepare_stats.insert(stream, stats);
        Ok(PreparedCommands {
            stream,
            commands: prepared,
        })
    }

    pub(super) fn recycle_prepared_commands(&mut self, prepared: PreparedCommands) {
        self.prepared_command_scratch
            .recycle(prepared.stream, prepared.commands);
    }

    fn build_prepared_template(
        &mut self,
        command: &RenderCommand,
        font_manager: &FontManager,
        viewport: VertexViewport,
    ) -> Result<Option<PreparedTemplateBuild>, TguiError> {
        let build = match command {
            RenderCommand::BackdropBlur(primitive) => {
                if primitive.blur_radius <= 0.0
                    || primitive.rect.width <= Dp::ZERO
                    || primitive.rect.height <= Dp::ZERO
                {
                    return Ok(None);
                }
                let fullscreen = TextVertex::quad(
                    TextQuadSpec {
                        rect: Rect::new(
                            0.0,
                            0.0,
                            viewport.logical_size[0],
                            viewport.logical_size[1],
                        ),
                        uv_rect: None,
                        corner_radius: 0.0,
                        clip_mask: None,
                        opacity: 1.0,
                        tint: [255; 4],
                    },
                    viewport,
                );
                let vertices = CompositeVertex::quad(
                    CompositeQuadSpec {
                        rect: primitive.rect,
                        corner_radius: primitive.corner_radius,
                        clip_mask: primitive.clip_mask,
                    },
                    viewport,
                );
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::BackdropBlur {
                        primitive: *primitive,
                        composite_vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        composite_vertex_count: vertices.len() as u32,
                        fullscreen_vertices: bytemuck::cast_slice(&fullscreen).to_vec(),
                        fullscreen_vertex_count: fullscreen.len() as u32,
                    },
                    cacheable: true,
                }
            }
            RenderCommand::Brush(primitive) => {
                if primitive.rect.width <= Dp::ZERO || primitive.rect.height <= Dp::ZERO {
                    return Ok(None);
                }
                let Some(brush_data) =
                    BrushPrimitiveData::from_background_brush(&primitive.brush, 1.0)
                else {
                    return Ok(None);
                };
                let vertices = BrushVertex::from_spec(
                    BrushVertexSpec {
                        rect: primitive.rect,
                        corner_radius: primitive.corner_radius,
                        brush_data,
                    },
                    viewport,
                );
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::Brush {
                        clip_rect: primitive.clip_rect,
                        vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        vertex_count: vertices.len() as u32,
                    },
                    cacheable: true,
                }
            }
            RenderCommand::CanvasComposite(primitive) => {
                if primitive.bounds.width <= Dp::ZERO || primitive.bounds.height <= Dp::ZERO {
                    return Ok(None);
                }
                let vertices = CompositeVertex::quad(
                    CompositeQuadSpec {
                        rect: primitive.bounds,
                        corner_radius: 0.0,
                        clip_mask: primitive.clip_mask,
                    },
                    viewport,
                );
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::CanvasComposite {
                        primitive: Arc::new((**primitive).clone()),
                        vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        vertex_count: vertices.len() as u32,
                    },
                    cacheable: true,
                }
            }
            RenderCommand::Shape(primitive) => {
                if !should_prepare_shape(primitive, self.transparent_shape_skip_enabled()) {
                    return Ok(None);
                }
                let vertices = RectVertex::from_primitive(*primitive, viewport);
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::Rect {
                        clip_rect: primitive.clip_rect,
                        vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        vertex_count: vertices.len() as u32,
                    },
                    cacheable: true,
                }
            }
            RenderCommand::TextDecoration(primitive) => {
                if primitive.segments.is_empty() || primitive.color.a == 0 {
                    return Ok(None);
                }
                let mut vertices = Vec::with_capacity(primitive.segments.len() * 6);
                for rect in primitive.segments.iter().copied() {
                    if rect.width <= Dp::ZERO || rect.height <= Dp::ZERO {
                        continue;
                    }
                    let segment = RenderPrimitive {
                        rect,
                        color: primitive.color,
                        corner_radius: primitive.corner_radius,
                        stroke_width: primitive.stroke_width,
                        clip_rect: primitive.clip_rect,
                        clip_mask: primitive.clip_mask,
                    };
                    vertices.extend_from_slice(&RectVertex::from_primitive(segment, viewport));
                }
                if vertices.is_empty() {
                    return Ok(None);
                }
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::Rect {
                        clip_rect: primitive.clip_rect,
                        vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        vertex_count: vertices.len() as u32,
                    },
                    cacheable: true,
                }
            }
            RenderCommand::Mesh(primitive) => {
                if primitive.vertices.is_empty() {
                    return Ok(None);
                }
                let vertices: Vec<_> = primitive
                    .vertices
                    .iter()
                    .copied()
                    .map(|vertex| MeshVertex::from_scene_vertex(vertex, viewport))
                    .collect();
                let clip_binding = self.mesh_clip_bind_group_cache.binding_for(
                    &self.device,
                    &self.mesh_clip_bind_group_layout,
                    physical_mesh_clip_mask_data(primitive.clip_mask, viewport.scale_factor),
                );
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::Mesh {
                        clip_rect: primitive.clip_rect,
                        clip_binding,
                        vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        vertex_count: vertices.len() as u32,
                    },
                    cacheable: true,
                }
            }
            RenderCommand::Texture(texture) => {
                let Some(binding) = self.texture_bind_group_for(&texture.texture)? else {
                    return Ok(None);
                };
                let vertices = texture_quad_vertices(
                    texture.frame,
                    texture.quad,
                    texture.uv_rect,
                    texture.corner_radius,
                    texture.clip_mask,
                    texture.opacity,
                    [255; 4],
                    viewport,
                );
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::Sprite {
                        pipeline: PreparedSpritePipeline::Rgba,
                        binding,
                        clip_rect: texture.clip_rect,
                        vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        vertex_count: vertices.len() as u32,
                    },
                    cacheable: true,
                }
            }
            #[cfg(feature = "video")]
            RenderCommand::VideoTexture(texture) => {
                let Some(render_frame) = texture.controller.current_render_frame() else {
                    return Ok(None);
                };
                let (pipeline, binding) = match render_frame {
                    crate::video::backend::VideoRenderFrame::Rgba(frame_texture) => {
                        let Some(binding) = self.texture_bind_group_for(&frame_texture)? else {
                            return Ok(None);
                        };
                        (PreparedSpritePipeline::Rgba, binding)
                    }
                    crate::video::backend::VideoRenderFrame::Yuv(frame) => {
                        let Some(binding) = self.video_yuv_bind_group_for(&frame)? else {
                            return Ok(None);
                        };
                        (PreparedSpritePipeline::VideoYuv, binding)
                    }
                };
                let vertices = texture_quad_vertices(
                    texture.frame,
                    texture.quad,
                    texture.uv_rect,
                    texture.corner_radius,
                    texture.clip_mask,
                    texture.opacity,
                    [255; 4],
                    viewport,
                );
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::Sprite {
                        pipeline,
                        binding,
                        clip_rect: texture.clip_rect,
                        vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        vertex_count: vertices.len() as u32,
                    },
                    cacheable: false,
                }
            }
            RenderCommand::Text(text) => {
                let opacity = text.color.a as f32 / 255.0;
                if opacity <= 0.0 {
                    return Ok(None);
                }
                let Some(draw) = self.text_bind_group_for(text, font_manager)? else {
                    return Ok(None);
                };
                let snapped_frame = self.snap_text_rect(text.frame);
                let vertices = texture_quad_vertices(
                    snapped_frame,
                    text.quad,
                    draw.uv_rect,
                    0.0,
                    text.clip_mask,
                    opacity,
                    if draw.tintable_mask {
                        let [red, green, blue, _] = text.color.to_rgba8();
                        // Tint alpha is otherwise redundant with the dedicated opacity
                        // attribute, so zero encodes an R8 coverage sample without growing
                        // TextVertex or selecting a second render pipeline.
                        [red, green, blue, if draw.r8_coverage { 0 } else { 255 }]
                    } else {
                        [255; 4]
                    },
                    viewport,
                );
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::Sprite {
                        pipeline: PreparedSpritePipeline::Rgba,
                        binding: draw.binding,
                        clip_rect: text.clip_rect,
                        vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        vertex_count: vertices.len() as u32,
                    },
                    cacheable: true,
                }
            }
        };
        Ok(Some(build))
    }

    pub(super) fn apply_scissor<'a>(
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

    pub(super) fn scissor_rect(&self, clip_rect: Option<Rect>) -> Option<(u32, u32, u32, u32)> {
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

    pub(super) fn logical_viewport_size(&self) -> (f32, f32) {
        (
            self.config.width as f32 / self.scale_factor,
            self.config.height as f32 / self.scale_factor,
        )
    }

    pub(super) fn vertex_viewport(&self) -> VertexViewport {
        let (logical_width, logical_height) = self.logical_viewport_size();
        VertexViewport::new(
            logical_width,
            logical_height,
            self.config.width as f32,
            self.config.height as f32,
            self.scale_factor,
        )
    }

    pub(super) fn logical_to_physical(&self, value: f32) -> f32 {
        value * self.scale_factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::color::Color;
    use crate::ui::layout::Overflow;
    use crate::ui::widget::{Point, Rect, ScrollRegion, TransformRecord, WidgetId};

    fn make_viewport(logical_width: f32, logical_height: f32, scale: f32) -> VertexViewport {
        let physical_width = logical_width * scale;
        let physical_height = logical_height * scale;
        VertexViewport::new(
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            scale,
        )
    }

    fn make_scroll_region(
        id: u64,
        visible_frame: Rect,
        gpu_base_scroll_offset: Point,
        scroll_offset: Point,
    ) -> ScrollRegion {
        ScrollRegion {
            id: WidgetId::from_raw(id),
            content_viewport: visible_frame,
            visible_frame,
            content_bounds: visible_frame,
            gpu_base_scroll_offset,
            scroll_offset,
            overflow_x: Overflow::Scroll,
            overflow_y: Overflow::Scroll,
            horizontal_track: None,
            horizontal_thumb: None,
            vertical_track: None,
            vertical_thumb: None,
        }
    }

    #[test]
    fn fully_transparent_shapes_have_no_prepared_pixels() {
        let mut shape = RenderPrimitive {
            rect: Rect::new(0.0, 0.0, 20.0, 20.0),
            color: Color::rgba(20, 40, 60, 0),
            corner_radius: 8.0,
            stroke_width: 2.0,
            clip_rect: Some(Rect::new(0.0, 0.0, 20.0, 20.0)),
            clip_mask: None,
        };
        assert!(!should_prepare_shape(&shape, true));
        assert!(should_prepare_shape(&shape, false));

        shape.color.a = 1;
        assert!(should_prepare_shape(&shape, true));
        shape.rect.width = Dp::ZERO;
        assert!(!should_prepare_shape(&shape, true));
        assert!(!should_prepare_shape(&shape, false));
    }

    #[test]
    fn mesh_clip_key_tracks_canonical_physical_uniform_data() {
        let clip = crate::ui::widget::ClipMask {
            rect: Rect::new(10.0, 20.0, 80.0, 40.0),
            corner_radius: 12.0,
        };
        let first = mesh_clip_key(physical_mesh_clip_mask_data(Some(clip), 2.0));
        let same = mesh_clip_key(physical_mesh_clip_mask_data(Some(clip), 2.0));
        let different_scale = mesh_clip_key(physical_mesh_clip_mask_data(Some(clip), 1.0));
        let no_clip = mesh_clip_key(physical_mesh_clip_mask_data(None, 2.0));

        assert_eq!(first, same);
        assert_ne!(first, different_scale);
        assert_ne!(first, no_clip);
    }

    #[test]
    fn test_compute_scroll_translate_no_region() {
        let scroll_regions = [];
        let viewport = make_viewport(800.0, 600.0, 2.0);

        let result =
            compute_scroll_translate(Some(WidgetId::from_raw(1)), &scroll_regions, viewport);
        assert!(result.is_none(), "无 ScrollRegion 时应返回 None");
    }

    #[test]
    fn test_compute_scroll_translate_zero_offset() {
        let scroll_regions = [make_scroll_region(
            1,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            Point::ZERO,
            Point {
                x: Dp::ZERO,
                y: Dp::ZERO,
            },
        )];
        let viewport = make_viewport(800.0, 600.0, 2.0);

        let result =
            compute_scroll_translate(Some(WidgetId::from_raw(1)), &scroll_regions, viewport);
        assert!(result.is_none(), "零偏移时应返回 None（优化）");
    }

    #[test]
    fn test_compute_scroll_translate_basic() {
        // Viewport: 800x600 逻辑 dp, scale=2.0 → 1600x1200 物理像素
        // ScrollRegion: visible_frame (0,0,800,600), offset (50, 30)
        let scroll_regions = [make_scroll_region(
            1,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            Point::ZERO,
            Point {
                x: Dp::new(50.0),
                y: Dp::new(30.0),
            },
        )];
        let viewport = make_viewport(800.0, 600.0, 2.0);

        let result =
            compute_scroll_translate(Some(WidgetId::from_raw(1)), &scroll_regions, viewport)
                .expect("应计算出平移量");

        // 物理偏移：50*2=100, 30*2=60
        // NDC：-2*100/1600 = -0.125, 2*60/1200 = 0.1
        assert!((result.offset_ndc[0] - (-0.125)).abs() < 0.001);
        assert!((result.offset_ndc[1] - 0.1).abs() < 0.001);
        assert!((result.offset_physical[0] - (-100.0)).abs() < 0.001);
        assert!((result.offset_physical[1] - (-60.0)).abs() < 0.001);
    }

    #[test]
    fn test_compute_scroll_translate_uses_explicit_region_id() {
        // 元数据明确指向内层 id=2，即使外层先出现也不靠几何猜测。
        let scroll_regions = [
            make_scroll_region(
                1,
                Rect::new(0.0, 0.0, 800.0, 600.0),
                Point::ZERO,
                Point {
                    x: Dp::new(10.0),
                    y: Dp::new(10.0),
                },
            ),
            make_scroll_region(
                2,
                Rect::new(100.0, 100.0, 200.0, 200.0),
                Point::ZERO,
                Point {
                    x: Dp::new(20.0),
                    y: Dp::new(5.0),
                },
            ),
        ];
        let viewport = make_viewport(800.0, 600.0, 2.0);

        let result =
            compute_scroll_translate(Some(WidgetId::from_raw(2)), &scroll_regions, viewport)
                .expect("应计算出平移量");

        // 应使用内层的 offset (20, 5)，物理 (40, 10)
        // NDC：-2*40/1600 = -0.05, 2*10/1200 ≈ 0.0167
        assert!((result.offset_ndc[0] - (-0.05)).abs() < 0.001);
        assert!((result.offset_ndc[1] - 0.01667).abs() < 0.001);
        assert!((result.offset_physical[0] - (-40.0)).abs() < 0.001);
        assert!((result.offset_physical[1] - (-10.0)).abs() < 0.001);
    }

    #[test]
    fn test_compute_scroll_translate_missing_metadata_or_region() {
        let scroll_regions = [make_scroll_region(
            1,
            Rect::new(0.0, 0.0, 200.0, 200.0),
            Point::ZERO,
            Point {
                x: Dp::new(50.0),
                y: Dp::new(30.0),
            },
        )];
        let viewport = make_viewport(800.0, 600.0, 2.0);

        assert!(compute_scroll_translate(None, &scroll_regions, viewport).is_none());
        assert!(
            compute_scroll_translate(Some(WidgetId::from_raw(2)), &scroll_regions, viewport)
                .is_none()
        );
    }

    #[test]
    fn scroll_translate_cache_scans_each_container_once_per_frame() {
        let viewport = make_viewport(800.0, 600.0, 2.0);
        let mut scroll_regions = Vec::with_capacity(100);
        // Put the requested id first so the reverse lookup must inspect all 100 regions on a
        // cache miss. Ten thousand tagged draws must still perform only that single scan.
        scroll_regions.push(make_scroll_region(
            0,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            Point::ZERO,
            Point::new(Dp::new(50.0), Dp::new(30.0)),
        ));
        for id in 1..100 {
            scroll_regions.push(make_scroll_region(
                id,
                Rect::new(0.0, 0.0, 800.0, 600.0),
                Point::ZERO,
                Point::ZERO,
            ));
        }

        let mut cache = ScrollTranslateCache::default();
        cache.begin_frame();
        for _ in 0..10_000 {
            assert!(cache
                .resolve(Some(WidgetId::from_raw(0)), &scroll_regions, viewport)
                .is_some());
        }

        assert_eq!(cache.region_visits, 100);
        assert_eq!(cache.translations.len(), 1);
        assert!(
            !cache.translations.spilled(),
            "the single-container fast path should not allocate"
        );
    }

    #[test]
    fn scroll_translate_cache_refreshes_offsets_at_frame_boundary() {
        let viewport = make_viewport(800.0, 600.0, 2.0);
        let mut scroll_regions = [make_scroll_region(
            1,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            Point::ZERO,
            Point::new(Dp::new(10.0), Dp::ZERO),
        )];
        let mut cache = ScrollTranslateCache::default();
        cache.begin_frame();
        let before = cache
            .resolve(Some(WidgetId::from_raw(1)), &scroll_regions, viewport)
            .expect("first frame should translate");

        scroll_regions[0].scroll_offset.x = Dp::new(40.0);
        // Within a frame, offsets are intentionally stable and reuse the cached value.
        let same_frame = cache
            .resolve(Some(WidgetId::from_raw(1)), &scroll_regions, viewport)
            .expect("same frame should reuse translation");
        assert_eq!(before.offset_physical, same_frame.offset_physical);

        cache.begin_frame();
        let next_frame = cache
            .resolve(Some(WidgetId::from_raw(1)), &scroll_regions, viewport)
            .expect("next frame should refresh translation");
        assert_ne!(before.offset_physical, next_frame.offset_physical);
        assert_eq!(cache.region_visits, 1);
    }

    #[test]
    fn test_compute_transform_translate_sums_chain_records() {
        let first_id = WidgetId::from_raw(10);
        let second_id = WidgetId::from_raw(11);
        let mut chain = TransformChain::new();
        chain.push(first_id);
        chain.push(second_id);
        let records = HashMap::from([
            (
                first_id,
                TransformRecord {
                    id: first_id,
                    base_offset: Point::ZERO,
                    current_offset: Point::new(Dp::new(10.0), Dp::new(20.0)),
                },
            ),
            (
                second_id,
                TransformRecord {
                    id: second_id,
                    base_offset: Point::new(Dp::new(4.0), Dp::new(6.0)),
                    current_offset: Point::new(Dp::new(9.0), Dp::new(1.0)),
                },
            ),
        ]);
        let viewport = make_viewport(800.0, 600.0, 2.0);

        let result = compute_transform_translate(Some(&chain), &records, viewport)
            .expect("non-zero transform chain should produce a translate");

        // Combined logical movement: (10, 20) + (5, -5) = (15, 15).
        assert!((result.offset_ndc[0] - 0.0375).abs() < 0.001);
        assert!((result.offset_ndc[1] - (-0.05)).abs() < 0.001);
        assert!((result.offset_physical[0] - 30.0).abs() < 0.001);
        assert!((result.offset_physical[1] - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_combine_translates_adds_scroll_and_transform() {
        let scroll = super::super::PushTranslate {
            offset_ndc: [-0.1, 0.2],
            offset_physical: [-20.0, -30.0],
        };
        let transform = super::super::PushTranslate {
            offset_ndc: [0.025, -0.05],
            offset_physical: [5.0, 10.0],
        };

        let result = combine_translates(Some(scroll), Some(transform)).expect("combined");

        assert!((result.offset_ndc[0] - (-0.075)).abs() < 0.001);
        assert!((result.offset_ndc[1] - 0.15).abs() < 0.001);
        assert!((result.offset_physical[0] - (-15.0)).abs() < 0.001);
        assert!((result.offset_physical[1] - (-20.0)).abs() < 0.001);
    }
}
