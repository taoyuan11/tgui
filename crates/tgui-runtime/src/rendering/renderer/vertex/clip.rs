use super::*;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct ClipMaskVertexData {
    pub(super) clip_local_position: [f32; 2],
    pub(super) clip_rect_size: [f32; 2],
    pub(super) clip_corner_radius: f32,
    pub(super) clip_enabled: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct MeshClipMaskUniformData {
    pub(super) data0: [f32; 4],
    pub(super) data1: [f32; 4],
}

pub(crate) fn physical_clip_mask_data(
    clip_mask: Option<ClipMask>,
    rect_origin_physical: [f32; 2],
    local_position: [f32; 2],
    scale_factor: f32,
) -> ClipMaskVertexData {
    clip_mask
        .map(|clip_mask| {
            let rect = clip_mask.rect;
            let clip_origin = [rect.x.get() * scale_factor, rect.y.get() * scale_factor];
            let clip_rect_size = [
                rect.width.max(0.0).get() * scale_factor,
                rect.height.max(0.0).get() * scale_factor,
            ];
            let clip_corner_radius = (clip_mask.corner_radius.max(0.0) * scale_factor)
                .min(clip_rect_size[0] * 0.5)
                .min(clip_rect_size[1] * 0.5);
            ClipMaskVertexData {
                clip_local_position: [
                    rect_origin_physical[0] - clip_origin[0] + local_position[0],
                    rect_origin_physical[1] - clip_origin[1] + local_position[1],
                ],
                clip_rect_size,
                clip_corner_radius,
                clip_enabled: 1.0,
            }
        })
        .unwrap_or(ClipMaskVertexData {
            clip_local_position: [0.0, 0.0],
            clip_rect_size: [0.0, 0.0],
            clip_corner_radius: 0.0,
            clip_enabled: 0.0,
        })
}

pub(crate) fn physical_clip_mask_at_position(
    clip_mask: Option<ClipMask>,
    position_physical: [f32; 2],
    scale_factor: f32,
) -> ClipMaskVertexData {
    clip_mask
        .map(|clip_mask| {
            let rect = clip_mask.rect;
            let clip_origin = [rect.x.get() * scale_factor, rect.y.get() * scale_factor];
            let clip_rect_size = [
                rect.width.max(0.0).get() * scale_factor,
                rect.height.max(0.0).get() * scale_factor,
            ];
            let clip_corner_radius = (clip_mask.corner_radius.max(0.0) * scale_factor)
                .min(clip_rect_size[0] * 0.5)
                .min(clip_rect_size[1] * 0.5);
            ClipMaskVertexData {
                clip_local_position: [
                    position_physical[0] - clip_origin[0],
                    position_physical[1] - clip_origin[1],
                ],
                clip_rect_size,
                clip_corner_radius,
                clip_enabled: 1.0,
            }
        })
        .unwrap_or(ClipMaskVertexData {
            clip_local_position: [0.0, 0.0],
            clip_rect_size: [0.0, 0.0],
            clip_corner_radius: 0.0,
            clip_enabled: 0.0,
        })
}

pub(crate) fn physical_mesh_clip_mask_data(
    clip_mask: Option<ClipMask>,
    scale_factor: f32,
) -> MeshClipMaskUniformData {
    clip_mask
        .map(|clip_mask| {
            let rect = clip_mask.rect;
            let clip_rect_size = [
                rect.width.max(0.0).get() * scale_factor,
                rect.height.max(0.0).get() * scale_factor,
            ];
            let clip_corner_radius = (clip_mask.corner_radius.max(0.0) * scale_factor)
                .min(clip_rect_size[0] * 0.5)
                .min(clip_rect_size[1] * 0.5);
            MeshClipMaskUniformData {
                data0: [
                    rect.x.get() * scale_factor,
                    rect.y.get() * scale_factor,
                    clip_rect_size[0],
                    clip_rect_size[1],
                ],
                data1: [clip_corner_radius, 1.0, 0.0, 0.0],
            }
        })
        .unwrap_or(MeshClipMaskUniformData {
            data0: [0.0; 4],
            data1: [0.0; 4],
        })
}

pub(crate) fn logical_clip_mask_data(
    clip_mask: Option<ClipMask>,
    position: [f32; 2],
) -> ClipMaskVertexData {
    clip_mask
        .map(|clip_mask| {
            let rect = clip_mask.rect;
            let clip_rect_size = [rect.width.max(0.0).get(), rect.height.max(0.0).get()];
            let clip_corner_radius = clip_mask
                .corner_radius
                .max(0.0)
                .min(clip_rect_size[0] * 0.5)
                .min(clip_rect_size[1] * 0.5);
            ClipMaskVertexData {
                clip_local_position: [position[0] - rect.x.get(), position[1] - rect.y.get()],
                clip_rect_size,
                clip_corner_radius,
                clip_enabled: 1.0,
            }
        })
        .unwrap_or(ClipMaskVertexData {
            clip_local_position: [0.0, 0.0],
            clip_rect_size: [0.0, 0.0],
            clip_corner_radius: 0.0,
            clip_enabled: 0.0,
        })
}
