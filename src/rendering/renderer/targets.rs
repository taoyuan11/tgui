use crate::foundation::error::TguiError;

use super::{surface, OffscreenTarget, Renderer};

pub(super) struct RendererTargets {
    pub(super) scene_target: Option<OffscreenTarget>,
    pub(super) blur_target: Option<OffscreenTarget>,
    pub(super) blur_scratch_target: Option<OffscreenTarget>,
    pub(super) composite_target: Option<OffscreenTarget>,
    pub(super) composite_mask_target: Option<OffscreenTarget>,
}

impl RendererTargets {
    pub(super) fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        msaa_sample_count: u32,
    ) -> Self {
        Self {
            scene_target: surface::create_offscreen_target(
                device,
                config,
                "tgui-scene-target",
                msaa_sample_count,
            ),
            blur_target: surface::create_offscreen_target(
                device,
                config,
                "tgui-blur-target",
                msaa_sample_count,
            ),
            blur_scratch_target: surface::create_offscreen_target(
                device,
                config,
                "tgui-blur-scratch-target",
                msaa_sample_count,
            ),
            composite_target: surface::create_offscreen_target(
                device,
                config,
                "tgui-composite-target",
                msaa_sample_count,
            ),
            composite_mask_target: surface::create_offscreen_target(
                device,
                config,
                "tgui-composite-mask-target",
                msaa_sample_count,
            ),
        }
    }
}

impl Renderer {
    pub(super) fn recreate_offscreen_targets(&mut self) {
        self.scene_target = surface::create_offscreen_target(
            &self.device,
            &self.config,
            "tgui-scene-target",
            self.msaa_sample_count,
        );
        self.blur_target = surface::create_offscreen_target(
            &self.device,
            &self.config,
            "tgui-blur-target",
            self.msaa_sample_count,
        );
        self.blur_scratch_target = surface::create_offscreen_target(
            &self.device,
            &self.config,
            "tgui-blur-scratch-target",
            self.msaa_sample_count,
        );
        self.composite_target = surface::create_offscreen_target(
            &self.device,
            &self.config,
            "tgui-composite-target",
            self.msaa_sample_count,
        );
        self.composite_mask_target = surface::create_offscreen_target(
            &self.device,
            &self.config,
            "tgui-composite-mask-target",
            self.msaa_sample_count,
        );
    }

    pub(super) fn scene_target_resolved_view(&self) -> Result<&wgpu::TextureView, TguiError> {
        self.scene_target
            .as_ref()
            .map(|target| &target.resolved_view)
            .ok_or_else(|| TguiError::TextRender("scene target unavailable".into()))
    }
}
