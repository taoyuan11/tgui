use bytemuck::{Pod, Zeroable};

use crate::ui::widget::{
    BrushPrimitiveData, ClipMask, MeshVertex as SceneMeshVertex, Rect, RenderPrimitive,
};

mod clip;

pub(super) use self::clip::physical_mesh_clip_mask_data;
use self::clip::{logical_clip_mask_data, physical_clip_mask_at_position, physical_clip_mask_data};

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
        physical_width: f32,
        physical_height: f32,
        scale_factor: f32,
    ) -> [Self; 6] {
        let rect_x = primitive.rect.x.get() * scale_factor;
        let rect_y = primitive.rect.y.get() * scale_factor;
        let rect_width = primitive.rect.width.max(0.0).get() * scale_factor;
        let rect_height = primitive.rect.height.max(0.0).get() * scale_factor;
        let x0 = rect_x / physical_width * 2.0 - 1.0;
        let x1 = (rect_x + rect_width) / physical_width * 2.0 - 1.0;
        let y0 = 1.0 - rect_y / physical_height * 2.0;
        let y1 = 1.0 - (rect_y + rect_height) / physical_height * 2.0;
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

    pub(super) fn from_scene_vertex(vertex: SceneMeshVertex, width: f32, height: f32) -> Self {
        let x = vertex.position[0] / width * 2.0 - 1.0;
        let y = 1.0 - vertex.position[1] / height * 2.0;
        Self {
            position: [x, y],
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

    pub(super) fn from_primitive(
        rect: Rect,
        corner_radius: f32,
        brush_data: BrushPrimitiveData,
        physical_width: f32,
        physical_height: f32,
        scale_factor: f32,
    ) -> [Self; 6] {
        let rect_x = rect.x.get() * scale_factor;
        let rect_y = rect.y.get() * scale_factor;
        let rect_width = rect.width.max(0.0).get() * scale_factor;
        let rect_height = rect.height.max(0.0).get() * scale_factor;
        let x0 = rect_x / physical_width * 2.0 - 1.0;
        let x1 = (rect_x + rect_width) / physical_width * 2.0 - 1.0;
        let y0 = 1.0 - rect_y / physical_height * 2.0;
        let y1 = 1.0 - (rect_y + rect_height) / physical_height * 2.0;
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

    pub(super) fn quad(
        rect: Rect,
        width: f32,
        height: f32,
        corner_radius: f32,
        clip_mask: Option<ClipMask>,
    ) -> [Self; 6] {
        let rect_x = rect.x.get();
        let rect_y = rect.y.get();
        let rect_width = rect.width.get();
        let rect_height = rect.height.get();
        let x0 = rect_x / width * 2.0 - 1.0;
        let x1 = (rect_x + rect_width) / width * 2.0 - 1.0;
        let y0 = 1.0 - rect_y / height * 2.0;
        let y1 = 1.0 - (rect_y + rect_height) / height * 2.0;
        let uv_x0 = rect_x / width;
        let uv_x1 = (rect_x + rect_width) / width;
        let uv_y0 = rect_y / height;
        let uv_y1 = (rect_y + rect_height) / height;
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

    pub(super) fn quad(
        rect: Rect,
        _width: f32,
        _height: f32,
        uv_rect: Option<Rect>,
        corner_radius: f32,
        clip_mask: Option<ClipMask>,
        _physical_width: f32,
        _physical_height: f32,
        scale_factor: f32,
    ) -> [Self; 6] {
        let quad = [
            crate::ui::widget::Point::new(rect.x, rect.y),
            crate::ui::widget::Point::new(rect.right(), rect.y),
            crate::ui::widget::Point::new(rect.right(), rect.bottom()),
            crate::ui::widget::Point::new(rect.x, rect.bottom()),
        ];
        Self::transformed(
            rect,
            quad,
            uv_rect,
            corner_radius,
            clip_mask,
            _physical_width,
            _physical_height,
            scale_factor,
            1.0,
        )
    }

    pub(super) fn transformed(
        rect: Rect,
        quad: [crate::ui::widget::Point; 4],
        uv_rect: Option<Rect>,
        corner_radius: f32,
        clip_mask: Option<ClipMask>,
        physical_width: f32,
        physical_height: f32,
        scale_factor: f32,
        opacity: f32,
    ) -> [Self; 6] {
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

        let build = |point: crate::ui::widget::Point, uv: [f32; 2], local_position: [f32; 2]| {
            let point_x = point.x.get() * scale_factor;
            let point_y = point.y.get() * scale_factor;
            let clip_mask =
                physical_clip_mask_at_position(clip_mask, [point_x, point_y], scale_factor);
            Self {
                position: [
                    point_x / physical_width * 2.0 - 1.0,
                    1.0 - point_y / physical_height * 2.0,
                ],
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
    use super::TextVertex;
    use crate::ui::widget::Rect;

    #[test]
    fn fullscreen_quad_uses_scale_factor_for_clip_space() {
        let physical_width = 1280.0;
        let physical_height = 2856.0;
        let scale_factor = 3.5;
        let logical_width = physical_width / scale_factor;
        let logical_height = physical_height / scale_factor;
        let quad = TextVertex::quad(
            Rect::new(0.0, 0.0, logical_width, logical_height),
            logical_width,
            logical_height,
            None,
            0.0,
            None,
            physical_width,
            physical_height,
            scale_factor,
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
            assert!((vertex.position[0] - x).abs() < 1e-5);
            assert!((vertex.position[1] - y).abs() < 1e-5);
        }
    }
}
