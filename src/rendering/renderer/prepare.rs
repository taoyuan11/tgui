use wgpu::util::DeviceExt;

use crate::foundation::error::TguiError;
use crate::text::font::FontManager;
use crate::ui::unit::Dp;
use crate::ui::widget::{
    BrushPrimitiveData, CanvasCompositePrimitive, Rect, RenderCommand, RenderPrimitive,
    TransformChain, TransformRecord, WidgetId,
};
use std::collections::HashMap;

use super::{
    physical_mesh_clip_mask_data, BrushVertex, BrushVertexSpec, CompositeQuadSpec, CompositeVertex,
    MeshVertex, RectVertex, Renderer, TextQuadSpec, TextTransformSpec, TextVertex, VertexViewport,
};

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

pub(super) struct PreparedRect {
    pub(super) clip_rect: Option<Rect>,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
    /// Phase 4：该 draw 所属滚动容器的平移量。非滚动内容为 None。
    pub(super) scroll_translate: Option<super::PushTranslate>,
}

pub(super) struct PreparedBrush {
    pub(super) clip_rect: Option<Rect>,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
    pub(super) scroll_translate: Option<super::PushTranslate>,
}

pub(super) struct PreparedMesh {
    pub(super) clip_rect: Option<Rect>,
    pub(super) clip_bind_group: wgpu::BindGroup,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
    pub(super) scroll_translate: Option<super::PushTranslate>,
}

pub(super) struct PreparedSprite {
    pub(super) bind_group: wgpu::BindGroup,
    pub(super) clip_rect: Option<Rect>,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
    pub(super) scroll_translate: Option<super::PushTranslate>,
}

pub(super) struct PreparedBackdropBlur {
    pub(super) primitive: crate::ui::widget::BackdropBlurPrimitive,
    pub(super) composite_offset: u64,
    pub(super) composite_vertex_count: u32,
    pub(super) fullscreen_offset: u64,
    pub(super) fullscreen_vertex_count: u32,
}

pub(super) struct PreparedCanvasComposite {
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

pub(super) struct PreparedCommands(pub(super) Vec<PreparedCommand>);

impl Renderer {
    pub(super) fn prepare_commands(
        &mut self,
        commands: &[RenderCommand],
        font_manager: &FontManager,
        viewport: VertexViewport,
        scroll_regions: &[crate::ui::widget::ScrollRegion],
        command_gpu_scroll_containers: &[Option<WidgetId>],
        command_transform_chains: &[TransformChain],
        transform_records: &HashMap<WidgetId, TransformRecord>,
    ) -> Result<PreparedCommands, TguiError> {
        let mut prepared = Vec::with_capacity(commands.len());

        for (command_index, command) in commands.iter().enumerate() {
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
            match command {
                RenderCommand::BackdropBlur(primitive) => {
                    if primitive.blur_radius <= 0.0
                        || primitive.rect.width <= Dp::ZERO
                        || primitive.rect.height <= Dp::ZERO
                    {
                        continue;
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
                    let fullscreen_offset =
                        self.vertex_pool.allocate(bytemuck::cast_slice(&fullscreen));
                    let vertices = CompositeVertex::quad(
                        CompositeQuadSpec {
                            rect: primitive.rect,
                            corner_radius: primitive.corner_radius,
                            clip_mask: primitive.clip_mask,
                        },
                        viewport,
                    );
                    let composite_offset =
                        self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                    prepared.push(PreparedCommand::BackdropBlur(PreparedBackdropBlur {
                        primitive: *primitive,
                        composite_offset,
                        composite_vertex_count: vertices.len() as u32,
                        fullscreen_offset,
                        fullscreen_vertex_count: fullscreen.len() as u32,
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
                    let vertices = BrushVertex::from_spec(
                        BrushVertexSpec {
                            rect: primitive.rect,
                            corner_radius: primitive.corner_radius,
                            brush_data,
                        },
                        viewport,
                    );
                    let vertex_offset = self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                    prepared.push(PreparedCommand::Brush(PreparedBrush {
                        clip_rect: primitive.clip_rect,
                        vertex_offset,
                        vertex_count: vertices.len() as u32,
                        scroll_translate: draw_translate,
                    }));
                }
                RenderCommand::CanvasComposite(primitive) => {
                    if primitive.bounds.width <= Dp::ZERO || primitive.bounds.height <= Dp::ZERO {
                        continue;
                    }
                    let vertices = CompositeVertex::quad(
                        CompositeQuadSpec {
                            rect: primitive.bounds,
                            corner_radius: 0.0,
                            clip_mask: primitive.clip_mask,
                        },
                        viewport,
                    );
                    let composite_offset =
                        self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                    prepared.push(PreparedCommand::CanvasComposite(PreparedCanvasComposite {
                        primitive: (**primitive).clone(),
                        composite_offset,
                        composite_vertex_count: vertices.len() as u32,
                    }));
                }
                RenderCommand::Shape(primitive) => {
                    if primitive.rect.width <= Dp::ZERO || primitive.rect.height <= Dp::ZERO {
                        continue;
                    }
                    let vertices = RectVertex::from_primitive(*primitive, viewport);
                    let vertex_offset = self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                    prepared.push(PreparedCommand::Rect(PreparedRect {
                        clip_rect: primitive.clip_rect,
                        vertex_offset,
                        vertex_count: vertices.len() as u32,
                        scroll_translate: draw_translate,
                    }));
                }
                RenderCommand::TextDecoration(primitive) => {
                    if primitive.segments.is_empty() || primitive.color.a == 0 {
                        continue;
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
                        continue;
                    }
                    let vertex_offset = self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                    prepared.push(PreparedCommand::Rect(PreparedRect {
                        clip_rect: primitive.clip_rect,
                        vertex_offset,
                        vertex_count: vertices.len() as u32,
                        scroll_translate: draw_translate,
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
                        .map(|vertex| MeshVertex::from_scene_vertex(vertex, viewport))
                        .collect();
                    let vertex_offset = self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
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
                    let clip_bind_group =
                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("tgui-mesh-clip-bind-group"),
                            layout: &self.mesh_clip_bind_group_layout,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: clip_buffer.as_entire_binding(),
                            }],
                        });
                    prepared.push(PreparedCommand::Mesh(PreparedMesh {
                        clip_rect: primitive.clip_rect,
                        clip_bind_group,
                        vertex_offset,
                        vertex_count: vertices.len() as u32,
                        scroll_translate: draw_translate,
                    }));
                }
                RenderCommand::Texture(texture) => {
                    if let Some(bind_group) = self.texture_bind_group_for(&texture.texture)? {
                        let vertices = texture.quad.map_or_else(
                            || {
                                TextVertex::quad(
                                    TextQuadSpec {
                                        rect: texture.frame,
                                        uv_rect: texture.uv_rect,
                                        corner_radius: texture.corner_radius,
                                        clip_mask: texture.clip_mask,
                                        opacity: texture.opacity,
                                    },
                                    viewport,
                                )
                            },
                            |quad| {
                                TextVertex::transformed(
                                    TextTransformSpec {
                                        rect: texture.frame,
                                        quad,
                                        uv_rect: texture.uv_rect,
                                        corner_radius: texture.corner_radius,
                                        clip_mask: texture.clip_mask,
                                        opacity: texture.opacity,
                                    },
                                    viewport,
                                )
                            },
                        );
                        let vertex_offset =
                            self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                        prepared.push(PreparedCommand::Sprite(PreparedSprite {
                            bind_group,
                            clip_rect: texture.clip_rect,
                            vertex_offset,
                            vertex_count: vertices.len() as u32,
                            scroll_translate: draw_translate,
                        }));
                    }
                }
                #[cfg(feature = "video")]
                RenderCommand::VideoTexture(texture) => {
                    let Some(frame_texture) = texture.controller.current_frame() else {
                        continue;
                    };
                    if let Some(bind_group) = self.texture_bind_group_for(&frame_texture)? {
                        let vertices = texture.quad.map_or_else(
                            || {
                                TextVertex::quad(
                                    TextQuadSpec {
                                        rect: texture.frame,
                                        uv_rect: texture.uv_rect,
                                        corner_radius: texture.corner_radius,
                                        clip_mask: texture.clip_mask,
                                        opacity: texture.opacity,
                                    },
                                    viewport,
                                )
                            },
                            |quad| {
                                TextVertex::transformed(
                                    TextTransformSpec {
                                        rect: texture.frame,
                                        quad,
                                        uv_rect: texture.uv_rect,
                                        corner_radius: texture.corner_radius,
                                        clip_mask: texture.clip_mask,
                                        opacity: texture.opacity,
                                    },
                                    viewport,
                                )
                            },
                        );
                        let vertex_offset =
                            self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                        prepared.push(PreparedCommand::Sprite(PreparedSprite {
                            bind_group,
                            clip_rect: texture.clip_rect,
                            vertex_offset,
                            vertex_count: vertices.len() as u32,
                            scroll_translate: draw_translate,
                        }));
                    }
                }
                RenderCommand::Text(text) => {
                    let opacity = text.color.a as f32 / 255.0;
                    if opacity <= 0.0 {
                        continue;
                    }
                    if let Some(bind_group) = self.text_bind_group_for(text, font_manager)? {
                        let snapped_frame = self.snap_text_rect(text.frame);
                        let vertices = text.quad.map_or_else(
                            || {
                                TextVertex::quad(
                                    TextQuadSpec {
                                        rect: snapped_frame,
                                        uv_rect: None,
                                        corner_radius: 0.0,
                                        clip_mask: text.clip_mask,
                                        opacity,
                                    },
                                    viewport,
                                )
                            },
                            |quad| {
                                TextVertex::transformed(
                                    TextTransformSpec {
                                        rect: snapped_frame,
                                        quad,
                                        uv_rect: None,
                                        corner_radius: 0.0,
                                        clip_mask: text.clip_mask,
                                        opacity,
                                    },
                                    viewport,
                                )
                            },
                        );
                        let vertex_offset =
                            self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                        prepared.push(PreparedCommand::Sprite(PreparedSprite {
                            bind_group,
                            clip_rect: text.clip_rect,
                            vertex_offset,
                            vertex_count: vertices.len() as u32,
                            scroll_translate: draw_translate,
                        }));
                    }
                }
            }
        }

        Ok(PreparedCommands(prepared))
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
