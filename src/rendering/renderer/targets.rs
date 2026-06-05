use super::{surface, OffscreenTarget, Renderer};

pub(super) struct RendererTargets {
    pub(super) scene_target: Option<OffscreenTarget>,
    pub(super) snapshot_target: Option<OffscreenTarget>,
    pub(super) blur_target: Option<OffscreenTarget>,
    pub(super) blur_scratch_target: Option<OffscreenTarget>,
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
            snapshot_target: surface::create_offscreen_target(
                device,
                config,
                "tgui-snapshot-target",
                1,
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
        self.snapshot_target =
            surface::create_offscreen_target(&self.device, &self.config, "tgui-snapshot-target", 1);
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
    }
}
