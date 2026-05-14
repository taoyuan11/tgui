use crate::foundation::error::TguiError;

use super::{surface, MultisampleTarget, OffscreenTarget, Renderer};

pub(super) struct RendererTargets {
    pub(super) msaa_target: Option<MultisampleTarget>,
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
            msaa_target: surface::create_multisample_target(device, config, msaa_sample_count),
            scene_target: surface::create_offscreen_target(device, config, "tgui-scene-target"),
            blur_target: surface::create_offscreen_target(device, config, "tgui-blur-target"),
            blur_scratch_target: surface::create_offscreen_target(
                device,
                config,
                "tgui-blur-scratch-target",
            ),
            composite_target: surface::create_offscreen_target(
                device,
                config,
                "tgui-composite-target",
            ),
            composite_mask_target: surface::create_offscreen_target(
                device,
                config,
                "tgui-composite-mask-target",
            ),
        }
    }
}

impl Renderer {
    pub(super) fn recreate_multisample_target(&mut self) {
        self.msaa_target =
            surface::create_multisample_target(&self.device, &self.config, self.msaa_sample_count);
    }

    pub(super) fn recreate_offscreen_targets(&mut self) {
        self.scene_target =
            surface::create_offscreen_target(&self.device, &self.config, "tgui-scene-target");
        self.blur_target =
            surface::create_offscreen_target(&self.device, &self.config, "tgui-blur-target");
        self.blur_scratch_target = surface::create_offscreen_target(
            &self.device,
            &self.config,
            "tgui-blur-scratch-target",
        );
        self.composite_target =
            surface::create_offscreen_target(&self.device, &self.config, "tgui-composite-target");
        self.composite_mask_target = surface::create_offscreen_target(
            &self.device,
            &self.config,
            "tgui-composite-mask-target",
        );
    }

    pub(super) fn scene_target_view(&self) -> Result<&wgpu::TextureView, TguiError> {
        self.scene_target
            .as_ref()
            .map(|target| &target.view)
            .ok_or_else(|| TguiError::TextRender("scene target unavailable".into()))
    }
}
