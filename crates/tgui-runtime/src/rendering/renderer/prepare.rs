use wgpu::util::DeviceExt;

use crate::foundation::error::TguiError;
use crate::text::font::FontManager;
use crate::ui::unit::Dp;
use crate::ui::widget::{
    BrushPrimitiveData, CanvasCompositePrimitive, DirtyDrawRange, Rect, RenderCommand,
    RenderPrimitive, SceneDrawStream, TransformChain, TransformRecord, WidgetId,
};
use std::collections::HashMap;

use super::{
    physical_mesh_clip_mask_data, BrushVertex, BrushVertexSpec, CompositeQuadSpec, CompositeVertex,
    MeshVertex, RectVertex, Renderer, TextQuadSpec, TextTransformSpec, TextVertex, VertexViewport,
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
    pub(super) clip_bind_group: wgpu::BindGroup,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
    pub(super) scroll_translate: Option<super::PushTranslate>,
}

pub(super) struct PreparedSprite {
    pub(super) draw_id: DrawId,
    pub(super) bind_group: wgpu::BindGroup,
    pub(super) clip_rect: Option<Rect>,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
    pub(super) scroll_translate: Option<super::PushTranslate>,
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
    pub(super) primitive: CanvasCompositePrimitive,
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

pub(super) struct PreparedCommands(pub(super) Vec<PreparedCommand>);

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
        primitive: CanvasCompositePrimitive,
        vertices: Vec<u8>,
        vertex_count: u32,
    },
    Mesh {
        clip_rect: Option<Rect>,
        clip_bind_group: wgpu::BindGroup,
        vertices: Vec<u8>,
        vertex_count: u32,
    },
    Sprite {
        bind_group: wgpu::BindGroup,
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
                vertex_offset: vertex_pool.allocate(vertices),
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
                vertex_offset: vertex_pool.allocate(vertices),
                vertex_count: *vertex_count,
                scroll_translate,
            }),
            Self::CanvasComposite {
                primitive,
                vertices,
                vertex_count,
            } => PreparedCommand::CanvasComposite(PreparedCanvasComposite {
                draw_id,
                primitive: primitive.clone(),
                composite_offset: vertex_pool.allocate(vertices),
                composite_vertex_count: *vertex_count,
            }),
            Self::Mesh {
                clip_rect,
                clip_bind_group,
                vertices,
                vertex_count,
            } => PreparedCommand::Mesh(PreparedMesh {
                draw_id,
                clip_rect: *clip_rect,
                clip_bind_group: clip_bind_group.clone(),
                vertex_offset: vertex_pool.allocate(vertices),
                vertex_count: *vertex_count,
                scroll_translate,
            }),
            Self::Sprite {
                bind_group,
                clip_rect,
                vertices,
                vertex_count,
            } => PreparedCommand::Sprite(PreparedSprite {
                draw_id,
                bind_group: bind_group.clone(),
                clip_rect: *clip_rect,
                vertex_offset: vertex_pool.allocate(vertices),
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
        let mut prepared = Vec::with_capacity(commands.len());

        for (command_index, command) in commands.iter().enumerate() {
            let draw_id = DrawId::new(stream, command_index);
            let gpu_scroll_container = command_gpu_scroll_containers
                .get(command_index)
                .copied()
                .flatten();
            let draw_translate = combine_translates(
                compute_scroll_translate(gpu_scroll_container, scroll_regions, viewport),
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
                if let Some(template) = self
                    .retained_prepare_cache
                    .template(stream, command_index)
                    .cloned()
                {
                    stats.reuse += 1;
                    prepared.push(template.prepare(draw_id, &mut self.vertex_pool, draw_translate));
                    continue;
                }
            }

            stats.rebuild += 1;
            let Some(build) = self.build_prepared_template(command, font_manager, viewport)? else {
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
        Ok(PreparedCommands(prepared))
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
                        primitive: (**primitive).clone(),
                        vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        vertex_count: vertices.len() as u32,
                    },
                    cacheable: true,
                }
            }
            RenderCommand::Shape(primitive) => {
                if primitive.rect.width <= Dp::ZERO || primitive.rect.height <= Dp::ZERO {
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
                let clip_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("tgui-mesh-clip-uniform"),
                            contents: bytemuck::bytes_of(&physical_mesh_clip_mask_data(
                                primitive.clip_mask,
                                viewport.scale_factor,
                            )),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });
                let clip_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("tgui-mesh-clip-bind-group"),
                    layout: &self.mesh_clip_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: clip_buffer.as_entire_binding(),
                    }],
                });
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::Mesh {
                        clip_rect: primitive.clip_rect,
                        clip_bind_group,
                        vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        vertex_count: vertices.len() as u32,
                    },
                    cacheable: true,
                }
            }
            RenderCommand::Texture(texture) => {
                let Some(bind_group) = self.texture_bind_group_for(&texture.texture)? else {
                    return Ok(None);
                };
                let vertices = texture_quad_vertices(
                    texture.frame,
                    texture.quad,
                    texture.uv_rect,
                    texture.corner_radius,
                    texture.clip_mask,
                    texture.opacity,
                    viewport,
                );
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::Sprite {
                        bind_group,
                        clip_rect: texture.clip_rect,
                        vertices: bytemuck::cast_slice(&vertices).to_vec(),
                        vertex_count: vertices.len() as u32,
                    },
                    cacheable: true,
                }
            }
            #[cfg(feature = "video")]
            RenderCommand::VideoTexture(texture) => {
                let Some(frame_texture) = texture.controller.current_frame() else {
                    return Ok(None);
                };
                let Some(bind_group) = self.texture_bind_group_for(&frame_texture)? else {
                    return Ok(None);
                };
                let vertices = texture_quad_vertices(
                    texture.frame,
                    texture.quad,
                    texture.uv_rect,
                    texture.corner_radius,
                    texture.clip_mask,
                    texture.opacity,
                    viewport,
                );
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::Sprite {
                        bind_group,
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
                let Some(bind_group) = self.text_bind_group_for(text, font_manager)? else {
                    return Ok(None);
                };
                let snapped_frame = self.snap_text_rect(text.frame);
                let vertices = texture_quad_vertices(
                    snapped_frame,
                    text.quad,
                    None,
                    0.0,
                    text.clip_mask,
                    opacity,
                    viewport,
                );
                PreparedTemplateBuild {
                    template: PreparedCommandTemplate::Sprite {
                        bind_group,
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
