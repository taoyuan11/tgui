use crate::foundation::error::TguiError;

use super::{surface, OffscreenTarget, Renderer};

const BACKDROP_BLUR_TARGET_SCALE: u32 = 2;

pub(super) struct RendererTargets {
    pub(super) scene_target: Option<OffscreenTarget>,
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
        }
    }
}

impl Renderer {
    fn create_target(&self, label: &str, sample_count: u32) -> Result<OffscreenTarget, TguiError> {
        surface::create_offscreen_target(&self.device, &self.config, label, sample_count)
            .ok_or_else(|| TguiError::TextRender(format!("{label} unavailable")))
    }

    fn create_scaled_target(
        &self,
        label: &str,
        sample_count: u32,
        scale: u32,
    ) -> Result<OffscreenTarget, TguiError> {
        let scale = scale.max(1);
        let width = self.config.width.div_ceil(scale).max(1);
        let height = self.config.height.div_ceil(scale).max(1);
        surface::create_offscreen_target_with_size(
            &self.device,
            &self.config,
            label,
            sample_count,
            width,
            height,
        )
        .ok_or_else(|| TguiError::TextRender(format!("{label} unavailable")))
    }

    pub(super) fn ensure_snapshot_target(&mut self) -> Result<OffscreenTarget, TguiError> {
        if self.snapshot_target.is_none() {
            self.snapshot_target = Some(self.create_target("tgui-snapshot-target", 1)?);
        }
        Ok(self
            .snapshot_target
            .as_ref()
            .expect("snapshot target initialized")
            .clone())
    }

    pub(super) fn ensure_blur_targets(
        &mut self,
    ) -> Result<(OffscreenTarget, OffscreenTarget), TguiError> {
        if self.blur_target.is_none() {
            self.blur_target = Some(self.create_scaled_target(
                "tgui-blur-target",
                1,
                BACKDROP_BLUR_TARGET_SCALE,
            )?);
        }
        if self.blur_scratch_target.is_none() {
            self.blur_scratch_target = Some(self.create_scaled_target(
                "tgui-blur-scratch-target",
                1,
                BACKDROP_BLUR_TARGET_SCALE,
            )?);
        }
        Ok((
            self.blur_target
                .as_ref()
                .expect("blur target initialized")
                .clone(),
            self.blur_scratch_target
                .as_ref()
                .expect("blur scratch target initialized")
                .clone(),
        ))
    }

    pub(super) fn ensure_canvas_composite_targets(
        &mut self,
        depth: usize,
    ) -> Result<(OffscreenTarget, OffscreenTarget), TguiError> {
        while self.canvas_composite_targets.len() <= depth {
            let index = self.canvas_composite_targets.len();
            let label = format!("tgui-composite-target-{index}");
            let target = self.create_target(&label, self.msaa_sample_count)?;
            self.canvas_composite_targets.push(target);
        }
        while self.canvas_composite_mask_targets.len() <= depth {
            let index = self.canvas_composite_mask_targets.len();
            let label = format!("tgui-composite-mask-target-{index}");
            let target = self.create_target(&label, self.msaa_sample_count)?;
            self.canvas_composite_mask_targets.push(target);
        }
        Ok((
            self.canvas_composite_targets[depth].clone(),
            self.canvas_composite_mask_targets[depth].clone(),
        ))
    }

    pub(super) fn recreate_offscreen_targets(&mut self) {
        self.scene_target = surface::create_offscreen_target(
            &self.device,
            &self.config,
            "tgui-scene-target",
            self.msaa_sample_count,
        );
        self.snapshot_target = None;
        self.blur_target = None;
        self.blur_scratch_target = None;
        self.canvas_composite_targets.clear();
        self.canvas_composite_mask_targets.clear();
    }
}
