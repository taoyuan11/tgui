use super::*;

pub(super) mod collect;

#[cfg(feature = "bench-support")]
pub(crate) use collect::{
    with_legacy_background_blur_reactive_resolve, with_legacy_background_brush_reactive_resolve,
    with_legacy_background_reactive_resolve, with_legacy_border_color_reactive_resolve,
    with_legacy_border_radius_reactive_resolve, with_legacy_border_width_reactive_resolve,
    with_legacy_container_opacity_reactive_resolve, with_legacy_offset_reactive_resolve,
    with_legacy_progress_value_reactive_resolve, with_legacy_scale_reactive_resolve,
    with_legacy_scene_snapshots, with_legacy_slider_value_reactive_resolve,
    with_legacy_text_color_reactive_resolve, with_legacy_text_content_reactive_resolve,
    with_legacy_text_opacity_reactive_resolve, with_legacy_texture_mask_tint_reactive_resolve,
};

#[cfg(all(test, feature = "bench-support"))]
pub(crate) use collect::{
    background_blur_direct_probe, background_brush_direct_probe, border_color_direct_probe,
    border_radius_direct_probe, border_width_direct_probe, container_opacity_direct_probe,
    offset_direct_probe, scale_direct_probe, slider_value_direct_probe, text_opacity_direct_probe,
};

pub(crate) use collect::portal::{
    build_external_portal_overlay, collect_portal_content_scene, resolve_external_portal_anchor,
};

impl<VM> ResolvedElement<VM> {
    pub(in super::super) fn requires_runtime_lifecycle(&self) -> bool {
        #[cfg(feature = "audio")]
        {
            matches!(&self.kind, ResolvedWidgetKind::Audio { .. })
        }
        #[cfg(not(feature = "audio"))]
        {
            false
        }
    }
}
