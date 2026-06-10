use bytemuck::{Pod, Zeroable};

use crate::ui::widget::{
    BrushPrimitiveData, ClipMask, MeshVertex as SceneMeshVertex, Point, Rect, RenderPrimitive,
};

mod clip;

pub(super) use self::clip::physical_mesh_clip_mask_data;
use self::clip::{logical_clip_mask_data, physical_clip_mask_at_position, physical_clip_mask_data};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct VertexViewport {
    pub(super) logical_size: [f32; 2],
    pub(super) physical_size: [f32; 2],
    pub(super) scale_factor: f32,
}

impl VertexViewport {
    pub(super) fn new(
        logical_width: f32,
        logical_height: f32,
        physical_width: f32,
        physical_height: f32,
        scale_factor: f32,
    ) -> Self {
        Self {
            logical_size: [logical_width, logical_height],
            physical_size: [physical_width, physical_height],
            scale_factor,
        }
    }

    fn logical_clip_position(self, position: [f32; 2]) -> [f32; 2] {
        [
            position[0] / self.logical_size[0] * 2.0 - 1.0,
            1.0 - position[1] / self.logical_size[1] * 2.0,
        ]
    }

    fn physical_clip_position(self, position: [f32; 2]) -> [f32; 2] {
        [
            position[0] / self.physical_size[0] * 2.0 - 1.0,
            1.0 - position[1] / self.physical_size[1] * 2.0,
        ]
    }

    fn logical_to_physical_point(self, point: Point) -> [f32; 2] {
        [
            point.x.get() * self.scale_factor,
            point.y.get() * self.scale_factor,
        ]
    }
}

#[derive(Clone, Copy)]
pub(super) struct BrushVertexSpec {
    pub(super) rect: Rect,
    pub(super) corner_radius: f32,
    pub(super) brush_data: BrushPrimitiveData,
}

#[derive(Clone, Copy)]
pub(super) struct CompositeQuadSpec {
    pub(super) rect: Rect,
    pub(super) corner_radius: f32,
    pub(super) clip_mask: Option<ClipMask>,
}

#[derive(Clone, Copy)]
pub(super) struct TextQuadSpec {
    pub(super) rect: Rect,
    pub(super) uv_rect: Option<Rect>,
    pub(super) corner_radius: f32,
    pub(super) clip_mask: Option<ClipMask>,
    pub(super) opacity: f32,
}

#[derive(Clone, Copy)]
pub(super) struct TextTransformSpec {
    pub(super) rect: Rect,
    pub(super) quad: [Point; 4],
    pub(super) uv_rect: Option<Rect>,
    pub(super) corner_radius: f32,
    pub(super) clip_mask: Option<ClipMask>,
    pub(super) opacity: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct RectVertex {
    position: [f32; 2],
    color: [f32; 4],
    local_position: [f32; 2],
    rect_size: [f32; 2],
    corner_radius: f32,
    stroke_width: f32,
    clip_local_position: [f32; 2],
    clip_rect_size: [f32; 2],
    clip_corner_radius: f32,
    clip_enabled: f32,
}

impl RectVertex {
    pub(super) fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 10] = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x4,
            2 => Float32x2,
            3 => Float32x2,
            4 => Float32,
            5 => Float32,
            6 => Float32x2,
            7 => Float32x2,
            8 => Float32,
            9 => Float32
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }

    pub(super) fn from_primitive(
        primitive: RenderPrimitive,
        viewport: VertexViewport,
    ) -> [Self; 6] {
        let scale_factor = viewport.scale_factor;
        let rect_x = primitive.rect.x.get() * scale_factor;
        let rect_y = primitive.rect.y.get() * scale_factor;
        let rect_width = primitive.rect.width.max(0.0).get() * scale_factor;
        let rect_height = primitive.rect.height.max(0.0).get() * scale_factor;
        let [x0, y0] = viewport.physical_clip_position([rect_x, rect_y]);
        let [x1, y1] = viewport.physical_clip_position([rect_x + rect_width, rect_y + rect_height]);
        let color = primitive.color.to_linear_rgba_f32();
        let rect_size = [rect_width, rect_height];
        let radius = (primitive.corner_radius.max(0.0) * scale_factor)
            .min(rect_size[0] * 0.5)
            .min(rect_size[1] * 0.5);
        let stroke_width = (primitive.stroke_width.max(0.0) * scale_factor)
            .min(rect_size[0] * 0.5)
            .min(rect_size[1] * 0.5);
        let rect_origin = [rect_x, rect_y];

        let build = |position: [f32; 2], local_position: [f32; 2]| {
            let clip_mask = physical_clip_mask_data(
                primitive.clip_mask,
                rect_origin,
                local_position,
                scale_factor,
            );
            Self {
                position,
                color,
                local_position,
                rect_size,
                corner_radius: radius,
                stroke_width,
                clip_local_position: clip_mask.clip_local_position,
                clip_rect_size: clip_mask.clip_rect_size,
                clip_corner_radius: clip_mask.clip_corner_radius,
                clip_enabled: clip_mask.clip_enabled,
            }
        };

        [
            build([x0, y0], [0.0, 0.0]),
            build([x1, y0], [rect_size[0], 0.0]),
            build([x1, y1], [rect_size[0], rect_size[1]]),
            build([x0, y0], [0.0, 0.0]),
            build([x1, y1], [rect_size[0], rect_size[1]]),
            build([x0, y1], [0.0, rect_size[1]]),
        ]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct MeshVertex {
    position: [f32; 2],
    local_position: [f32; 2],
    brush_meta: [f32; 4],
    gradient_data0: [f32; 4],
    gradient_data1: [f32; 4],
    stop_offsets0: [f32; 4],
    stop_offsets1: [f32; 4],
    stop_color0: [f32; 4],
    stop_color1: [f32; 4],
    stop_color2: [f32; 4],
    stop_color3: [f32; 4],
    stop_color4: [f32; 4],
    stop_color5: [f32; 4],
    stop_color6: [f32; 4],
    stop_color7: [f32; 4],
}

impl MeshVertex {
    pub(super) fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 15] = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x4,
            3 => Float32x4,
            4 => Float32x4,
            5 => Float32x4,
            6 => Float32x4,
            7 => Float32x4,
            8 => Float32x4,
            9 => Float32x4,
            10 => Float32x4,
            11 => Float32x4,
            12 => Float32x4,
            13 => Float32x4,
            14 => Float32x4
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }

    pub(super) fn from_scene_vertex(vertex: SceneMeshVertex, viewport: VertexViewport) -> Self {
        let position = viewport.logical_clip_position(vertex.position);
        Self {
            position,
            local_position: vertex.local_position,
            brush_meta: vertex.brush_meta,
            gradient_data0: vertex.gradient_data0,
            gradient_data1: vertex.gradient_data1,
            stop_offsets0: vertex.stop_offsets0,
            stop_offsets1: vertex.stop_offsets1,
            stop_color0: vertex.stop_colors[0],
            stop_color1: vertex.stop_colors[1],
            stop_color2: vertex.stop_colors[2],
            stop_color3: vertex.stop_colors[3],
            stop_color4: vertex.stop_colors[4],
            stop_color5: vertex.stop_colors[5],
            stop_color6: vertex.stop_colors[6],
            stop_color7: vertex.stop_colors[7],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct BrushVertex {
    position: [f32; 2],
    local_position: [f32; 2],
    rect_size: [f32; 2],
    corner_radius: f32,
    brush_meta: [f32; 4],
    gradient_data0: [f32; 4],
    gradient_data1: [f32; 4],
    stop_offsets0: [f32; 4],
    stop_offsets1: [f32; 4],
    stop_color0: [f32; 4],
    stop_color1: [f32; 4],
    stop_color2: [f32; 4],
    stop_color3: [f32; 4],
    stop_color4: [f32; 4],
    stop_color5: [f32; 4],
    stop_color6: [f32; 4],
}

impl BrushVertex {
    pub(super) fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 16] = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x2,
            3 => Float32,
            4 => Float32x4,
            5 => Float32x4,
            6 => Float32x4,
            7 => Float32x4,
            8 => Float32x4,
            9 => Float32x4,
            10 => Float32x4,
            11 => Float32x4,
            12 => Float32x4,
            13 => Float32x4,
            14 => Float32x4,
            15 => Float32x4
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }

    pub(super) fn from_spec(spec: BrushVertexSpec, viewport: VertexViewport) -> [Self; 6] {
        let BrushVertexSpec {
            rect,
            corner_radius,
            brush_data,
        } = spec;
        let scale_factor = viewport.scale_factor;
        let rect_x = rect.x.get() * scale_factor;
        let rect_y = rect.y.get() * scale_factor;
        let rect_width = rect.width.max(0.0).get() * scale_factor;
        let rect_height = rect.height.max(0.0).get() * scale_factor;
        let [x0, y0] = viewport.physical_clip_position([rect_x, rect_y]);
        let [x1, y1] = viewport.physical_clip_position([rect_x + rect_width, rect_y + rect_height]);
        let rect_size = [rect_width, rect_height];
        let radius = (corner_radius.max(0.0) * scale_factor)
            .min(rect_size[0] * 0.5)
            .min(rect_size[1] * 0.5);

        let build = |position: [f32; 2], local_position: [f32; 2]| Self {
            position,
            local_position,
            rect_size,
            corner_radius: radius,
            brush_meta: brush_data.brush_meta,
            gradient_data0: scale_gradient_pair(brush_data.gradient_data0, scale_factor),
            gradient_data1: scale_gradient_pair(brush_data.gradient_data1, scale_factor),
            stop_offsets0: brush_data.stop_offsets0,
            stop_offsets1: brush_data.stop_offsets1,
            stop_color0: brush_data.stop_colors[0],
            stop_color1: brush_data.stop_colors[1],
            stop_color2: brush_data.stop_colors[2],
            stop_color3: brush_data.stop_colors[3],
            stop_color4: brush_data.stop_colors[4],
            stop_color5: brush_data.stop_colors[5],
            stop_color6: brush_data.stop_colors[6],
        };

        [
            build([x0, y0], [0.0, 0.0]),
            build([x1, y0], [rect_size[0], 0.0]),
            build([x1, y1], [rect_size[0], rect_size[1]]),
            build([x0, y0], [0.0, 0.0]),
            build([x1, y1], [rect_size[0], rect_size[1]]),
            build([x0, y1], [0.0, rect_size[1]]),
        ]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct CompositeVertex {
    position: [f32; 2],
    uv: [f32; 2],
    local_position: [f32; 2],
    rect_size: [f32; 2],
    corner_radius: f32,
    clip_local_position: [f32; 2],
    clip_rect_size: [f32; 2],
    clip_corner_radius: f32,
    clip_enabled: f32,
}

impl CompositeVertex {
    pub(super) fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x2,
            3 => Float32x2,
            4 => Float32,
            5 => Float32x2,
            6 => Float32x2,
            7 => Float32,
            8 => Float32
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }

    pub(super) fn quad(spec: CompositeQuadSpec, viewport: VertexViewport) -> [Self; 6] {
        let CompositeQuadSpec {
            rect,
            corner_radius,
            clip_mask,
        } = spec;
        let rect_x = rect.x.get();
        let rect_y = rect.y.get();
        let rect_width = rect.width.get();
        let rect_height = rect.height.get();
        let [x0, y0] = viewport.logical_clip_position([rect_x, rect_y]);
        let [x1, y1] = viewport.logical_clip_position([rect_x + rect_width, rect_y + rect_height]);
        let uv_x0 = rect_x / viewport.logical_size[0];
        let uv_x1 = (rect_x + rect_width) / viewport.logical_size[0];
        let uv_y0 = rect_y / viewport.logical_size[1];
        let uv_y1 = (rect_y + rect_height) / viewport.logical_size[1];
        let rect_size = [rect_width, rect_height];
        let radius = corner_radius.min(rect_width * 0.5).min(rect_height * 0.5);
        let rect_origin = [rect_x, rect_y];

        let build = |position: [f32; 2], uv: [f32; 2], local_position: [f32; 2]| {
            let clip_mask = logical_clip_mask_data(
                clip_mask,
                [
                    rect_origin[0] + local_position[0],
                    rect_origin[1] + local_position[1],
                ],
            );
            Self {
                position,
                uv,
                local_position,
                rect_size,
                corner_radius: radius,
                clip_local_position: clip_mask.clip_local_position,
                clip_rect_size: clip_mask.clip_rect_size,
                clip_corner_radius: clip_mask.clip_corner_radius,
                clip_enabled: clip_mask.clip_enabled,
            }
        };

        [
            build([x0, y0], [uv_x0, uv_y0], [0.0, 0.0]),
            build([x1, y0], [uv_x1, uv_y0], [rect_size[0], 0.0]),
            build([x1, y1], [uv_x1, uv_y1], [rect_size[0], rect_size[1]]),
            build([x0, y0], [uv_x0, uv_y0], [0.0, 0.0]),
            build([x1, y1], [uv_x1, uv_y1], [rect_size[0], rect_size[1]]),
            build([x0, y1], [uv_x0, uv_y1], [0.0, rect_size[1]]),
        ]
    }
}

fn scale_gradient_pair(mut pair: [f32; 4], scale_factor: f32) -> [f32; 4] {
    pair[0] *= scale_factor;
    pair[1] *= scale_factor;
    pair[2] *= scale_factor;
    pair[3] *= scale_factor;
    pair
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct TextVertex {
    position: [f32; 2],
    uv: [f32; 2],
    local_position: [f32; 2],
    rect_size: [f32; 2],
    corner_radius: f32,
    clip_local_position: [f32; 2],
    clip_rect_size: [f32; 2],
    clip_corner_radius: f32,
    clip_enabled: f32,
    opacity: f32,
}

impl TextVertex {
    pub(super) fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 10] = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x2,
            3 => Float32x2,
            4 => Float32,
            5 => Float32x2,
            6 => Float32x2,
            7 => Float32,
            8 => Float32,
            9 => Float32
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }

    pub(super) fn quad(spec: TextQuadSpec, viewport: VertexViewport) -> [Self; 6] {
        let rect = spec.rect;
        let quad = [
            Point::new(rect.x, rect.y),
            Point::new(rect.right(), rect.y),
            Point::new(rect.right(), rect.bottom()),
            Point::new(rect.x, rect.bottom()),
        ];
        Self::transformed(
            TextTransformSpec {
                rect,
                quad,
                uv_rect: spec.uv_rect,
                corner_radius: spec.corner_radius,
                clip_mask: spec.clip_mask,
                opacity: spec.opacity,
            },
            viewport,
        )
    }

    pub(super) fn transformed(spec: TextTransformSpec, viewport: VertexViewport) -> [Self; 6] {
        let TextTransformSpec {
            rect,
            quad,
            uv_rect,
            corner_radius,
            clip_mask,
            opacity,
        } = spec;
        let scale_factor = viewport.scale_factor;
        let rect_width = rect.width.get();
        let rect_height = rect.height.get();
        let rect_width_physical = rect_width * scale_factor;
        let rect_height_physical = rect_height * scale_factor;
        let radius = (corner_radius.max(0.0) * scale_factor)
            .min(rect_width_physical * 0.5)
            .min(rect_height_physical * 0.5);
        let rect_size = [rect_width_physical, rect_height_physical];
        let local_tl = [0.0, 0.0];
        let local_tr = [rect_size[0], 0.0];
        let local_br = [rect_size[0], rect_size[1]];
        let local_bl = [0.0, rect_size[1]];
        let uv_rect = uv_rect.unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0));
        let uv_tl = [uv_rect.x.get(), uv_rect.y.get()];
        let uv_tr = [uv_rect.right().get(), uv_rect.y.get()];
        let uv_br = [uv_rect.right().get(), uv_rect.bottom().get()];
        let uv_bl = [uv_rect.x.get(), uv_rect.bottom().get()];

        let build = |point: Point, uv: [f32; 2], local_position: [f32; 2]| {
            let [point_x, point_y] = viewport.logical_to_physical_point(point);
            let clip_mask =
                physical_clip_mask_at_position(clip_mask, [point_x, point_y], scale_factor);
            Self {
                position: viewport.physical_clip_position([point_x, point_y]),
                uv,
                local_position,
                rect_size,
                corner_radius: radius,
                clip_local_position: clip_mask.clip_local_position,
                clip_rect_size: clip_mask.clip_rect_size,
                clip_corner_radius: clip_mask.clip_corner_radius,
                clip_enabled: clip_mask.clip_enabled,
                opacity,
            }
        };

        [
            build(quad[0], uv_tl, local_tl),
            build(quad[1], uv_tr, local_tr),
            build(quad[2], uv_br, local_br),
            build(quad[0], uv_tl, local_tl),
            build(quad[2], uv_br, local_br),
            build(quad[3], uv_bl, local_bl),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompositeQuadSpec, CompositeVertex, TextQuadSpec, TextTransformSpec, TextVertex,
        VertexViewport,
    };
    use crate::ui::widget::{Point, Rect};

    fn viewport(physical_width: f32, physical_height: f32, scale_factor: f32) -> VertexViewport {
        VertexViewport::new(
            physical_width / scale_factor,
            physical_height / scale_factor,
            physical_width,
            physical_height,
            scale_factor,
        )
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-5,
            "expected {actual} to be close to {expected}"
        );
    }

    #[test]
    fn viewport_maps_logical_and_physical_clip_space() {
        let viewport = VertexViewport::new(100.0, 50.0, 200.0, 100.0, 2.0);

        assert_eq!(viewport.logical_clip_position([0.0, 0.0]), [-1.0, 1.0]);
        assert_eq!(viewport.logical_clip_position([100.0, 50.0]), [1.0, -1.0]);
        assert_eq!(viewport.physical_clip_position([100.0, 50.0]), [0.0, 0.0]);
        assert_eq!(
            viewport.logical_to_physical_point(Point::new(25.0, 10.0)),
            [50.0, 20.0]
        );
    }

    #[test]
    fn fullscreen_quad_uses_scale_factor_for_clip_space() {
        let physical_width = 1280.0;
        let physical_height = 2856.0;
        let scale_factor = 3.5;
        let logical_width = physical_width / scale_factor;
        let logical_height = physical_height / scale_factor;
        let quad = TextVertex::quad(
            TextQuadSpec {
                rect: Rect::new(0.0, 0.0, logical_width, logical_height),
                uv_rect: None,
                corner_radius: 0.0,
                clip_mask: None,
                opacity: 1.0,
            },
            viewport(physical_width, physical_height, scale_factor),
        );

        let expected = [
            (-1.0_f32, 1.0_f32),
            (1.0_f32, 1.0_f32),
            (1.0_f32, -1.0_f32),
            (-1.0_f32, 1.0_f32),
            (1.0_f32, -1.0_f32),
            (-1.0_f32, -1.0_f32),
        ];

        for (vertex, (x, y)) in quad.iter().zip(expected) {
            assert_close(vertex.position[0], x);
            assert_close(vertex.position[1], y);
        }
    }

    #[test]
    fn quad_preserves_opacity() {
        let quad = TextVertex::quad(
            TextQuadSpec {
                rect: Rect::new(0.0, 0.0, 100.0, 50.0),
                uv_rect: None,
                corner_radius: 0.0,
                clip_mask: None,
                opacity: 0.25,
            },
            VertexViewport::new(100.0, 50.0, 100.0, 50.0, 1.0),
        );

        assert!(quad.iter().all(|vertex| vertex.opacity == 0.25));
    }

    #[test]
    fn transformed_quad_preserves_uv_and_uses_physical_clip_space() {
        let quad = TextVertex::transformed(
            TextTransformSpec {
                rect: Rect::new(10.0, 20.0, 30.0, 40.0),
                quad: [
                    Point::new(10.0, 20.0),
                    Point::new(40.0, 20.0),
                    Point::new(45.0, 65.0),
                    Point::new(5.0, 60.0),
                ],
                uv_rect: Some(Rect::new(0.25, 0.1, 0.5, 0.4)),
                corner_radius: 0.0,
                clip_mask: None,
                opacity: 0.8,
            },
            VertexViewport::new(100.0, 100.0, 200.0, 200.0, 2.0),
        );

        assert_close(quad[0].position[0], -0.8);
        assert_close(quad[0].position[1], 0.6);
        assert_close(quad[2].position[0], -0.1);
        assert_close(quad[2].position[1], -0.3);
        assert_eq!(quad[0].uv, [0.25, 0.1]);
        assert_eq!(quad[1].uv, [0.75, 0.1]);
        assert_eq!(quad[2].uv, [0.75, 0.5]);
        assert_eq!(quad[5].uv, [0.25, 0.5]);
        assert!(quad.iter().all(|vertex| vertex.opacity == 0.8));
    }

    #[test]
    fn composite_quad_uses_logical_viewport_for_position_and_uv() {
        let quad = CompositeVertex::quad(
            CompositeQuadSpec {
                rect: Rect::new(50.0, 25.0, 100.0, 50.0),
                corner_radius: 0.0,
                clip_mask: None,
            },
            VertexViewport::new(200.0, 100.0, 400.0, 200.0, 2.0),
        );

        assert_close(quad[0].position[0], -0.5);
        assert_close(quad[0].position[1], 0.5);
        assert_close(quad[2].position[0], 0.5);
        assert_close(quad[2].position[1], -0.5);
        assert_eq!(quad[0].uv, [0.25, 0.25]);
        assert_eq!(quad[2].uv, [0.75, 0.75]);
    }
}
