mod effects;

use crate::foundation::error::TguiError;

use super::prepare::PreparedCommand;
use super::surface::surface_clear_color;
use super::{OffscreenTarget, Renderer};

impl Renderer {
    pub(super) fn has_msaa(&self) -> bool {
        self.msaa_sample_count > 1
    }

    pub(super) fn offscreen_attachment_view<'a>(
        &self,
        target: &'a OffscreenTarget,
    ) -> &'a wgpu::TextureView {
        target.msaa_view.as_ref().unwrap_or(&target.single_view)
    }

    pub(super) fn offscreen_single_view<'a>(
        &self,
        target: &'a OffscreenTarget,
    ) -> &'a wgpu::TextureView {
        &target.single_view
    }

    pub(super) fn offscreen_resolve_target_for_draw<'a>(
        &self,
        target: &'a OffscreenTarget,
    ) -> Option<&'a wgpu::TextureView> {
        if self.has_msaa() {
            target.msaa_view.as_ref().map(|_| &target.single_view)
        } else {
            None
        }
    }

    fn offscreen_target<'a>(
        &self,
        target: Option<&'a OffscreenTarget>,
        name: &str,
    ) -> Result<&'a OffscreenTarget, TguiError> {
        target.ok_or_else(|| TguiError::TextRender(format!("{name} unavailable")))
    }

    pub(super) fn offscreen_sampled_view<'a>(
        &self,
        target: &'a OffscreenTarget,
    ) -> &'a wgpu::TextureView {
        let _ = self;
        self.offscreen_single_view(target)
    }

    fn clear_offscreen_target(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &OffscreenTarget,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tgui-offscreen-clear-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: self.offscreen_attachment_view(target),
                resolve_target: self.offscreen_resolve_target_for_draw(target),
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let _ = &mut pass;
    }

    fn snapshot_texture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        source: &OffscreenTarget,
        label: &str,
    ) -> wgpu::Texture {
        let snapshot = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &source.single_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &snapshot,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );
        snapshot
    }

    pub(super) fn execute_prepared_commands(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        commands: &[PreparedCommand],
        cleared_draw_target: &mut bool,
    ) -> Result<(), TguiError> {
        let scene_target = self
            .scene_target
            .as_ref()
            .cloned()
            .ok_or_else(|| TguiError::TextRender("scene target unavailable".into()))?;
        self.execute_prepared_commands_to_target(
            encoder,
            commands,
            &scene_target,
            cleared_draw_target,
        )
    }

    fn execute_prepared_commands_to_target(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        commands: &[PreparedCommand],
        target: &OffscreenTarget,
        cleared_draw_target: &mut bool,
    ) -> Result<(), TguiError> {
        let mut index = 0;
        while index < commands.len() {
            match &commands[index] {
                PreparedCommand::BackdropBlur(blur) => {
                    self.apply_backdrop_blur_to_target(encoder, blur, target, cleared_draw_target)?;
                    index += 1;
                    continue;
                }
                PreparedCommand::CanvasComposite(composite) => {
                    self.apply_canvas_composite_to_target(
                        encoder,
                        composite,
                        target,
                        cleared_draw_target,
                    )?;
                    index += 1;
                    continue;
                }
                _ => {}
            }

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-scene-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.offscreen_attachment_view(target),
                    resolve_target: self.offscreen_resolve_target_for_draw(target),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: if *cleared_draw_target {
                            wgpu::LoadOp::Load
                        } else {
                            *cleared_draw_target = true;
                            wgpu::LoadOp::Clear(surface_clear_color(self.clear_color))
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            while index < commands.len() {
                match &commands[index] {
                    PreparedCommand::BackdropBlur(_) | PreparedCommand::CanvasComposite(_) => break,
                    PreparedCommand::Rect(batch) => {
                        if self.apply_scissor(&mut pass, batch.clip_rect) {
                            pass.set_pipeline(&self.rect_pipeline);
                            pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                            pass.draw(0..batch.vertex_count, 0..1);
                        }
                    }
                    PreparedCommand::Brush(batch) => {
                        if self.apply_scissor(&mut pass, batch.clip_rect) {
                            pass.set_pipeline(&self.brush_pipeline);
                            pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                            pass.draw(0..batch.vertex_count, 0..1);
                        }
                    }
                    PreparedCommand::Mesh(batch) => {
                        if self.apply_scissor(&mut pass, batch.clip_rect) {
                            pass.set_pipeline(&self.mesh_pipeline);
                            pass.set_bind_group(0, &batch.clip_bind_group, &[]);
                            pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                            pass.draw(0..batch.vertex_count, 0..1);
                        }
                    }
                    PreparedCommand::Sprite(batch) => {
                        if self.apply_scissor(&mut pass, batch.clip_rect) {
                            pass.set_pipeline(&self.scene_text_pipeline);
                            pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                            pass.set_bind_group(0, &batch.bind_group, &[]);
                            pass.draw(0..batch.vertex_count, 0..1);
                        }
                    }
                }
                index += 1;
            }
        }

        Ok(())
    }
}
