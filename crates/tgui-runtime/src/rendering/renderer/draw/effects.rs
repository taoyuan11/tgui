use wgpu::util::DeviceExt;

use crate::foundation::error::TguiError;
use crate::text::font::FontManager;
use crate::ui::widget::CanvasBlendMode;

use super::super::prepare::{DrawStream, PreparedBackdropBlur, PreparedCanvasComposite};
use super::super::surface::surface_clear_color;
use super::super::{BlurUniform, CompositeUniform, OffscreenTarget, Renderer};

impl Renderer {
    pub(super) fn apply_backdrop_blur_to_target(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        blur: &PreparedBackdropBlur,
        target: &OffscreenTarget,
        cleared_draw_target: &mut bool,
    ) -> Result<(), TguiError> {
        let (blur_target, blur_scratch_target) = self.ensure_blur_targets()?;

        let scene_snapshot_view = self.copy_target_to_snapshot(encoder, target)?;

        let texel_size = backdrop_texel_size(blur_scratch_target.width, blur_scratch_target.height);
        let blur_scale_x =
            self.config.width.max(1) as f32 / blur_scratch_target.width.max(1) as f32;
        let blur_scale_y =
            self.config.height.max(1) as f32 / blur_scratch_target.height.max(1) as f32;
        let blur_scale = blur_scale_x.max(blur_scale_y).max(1.0);
        let blur_radius = blur.primitive.blur_radius.max(0.0) / blur_scale;
        let horizontal_uniform =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tgui-backdrop-horizontal-uniform"),
                    contents: bytemuck::bytes_of(&BlurUniform {
                        direction: [1.0, 0.0],
                        texel_size,
                        radius: blur_radius,
                        _pad: 0.0,
                    }),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        let vertical_uniform = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tgui-backdrop-vertical-uniform"),
                contents: bytemuck::bytes_of(&BlurUniform {
                    direction: [0.0, 1.0],
                    texel_size,
                    radius: blur_radius,
                    _pad: 0.0,
                }),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let horizontal_bind_group_entries = [
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&scene_snapshot_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&self.text_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: horizontal_uniform.as_entire_binding(),
            },
        ];

        let horizontal_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-backdrop-horizontal-bind-group"),
            layout: &self.backdrop_blur_bind_group_layout,
            entries: &horizontal_bind_group_entries,
        });
        let vertical_bind_group_entries = [
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    self.offscreen_sampled_view(&blur_scratch_target),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&self.text_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: vertical_uniform.as_entire_binding(),
            },
        ];

        let vertical_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-backdrop-vertical-bind-group"),
            layout: &self.backdrop_blur_bind_group_layout,
            entries: &vertical_bind_group_entries,
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-backdrop-horizontal-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.offscreen_attachment_view(&blur_scratch_target),
                    resolve_target: self.offscreen_resolve_target_for_draw(&blur_scratch_target),
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
            pass.set_pipeline(&self.backdrop_blur_pipeline);
            pass.set_vertex_buffer(
                0,
                self.vertex_pool
                    .current_buffer()
                    .slice(blur.fullscreen_offset..),
            );
            pass.set_bind_group(0, &horizontal_bind_group, &[]);
            pass.draw(0..blur.fullscreen_vertex_count, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-backdrop-vertical-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.offscreen_attachment_view(&blur_target),
                    resolve_target: self.offscreen_resolve_target_for_draw(&blur_target),
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
            pass.set_pipeline(&self.backdrop_blur_pipeline);
            pass.set_vertex_buffer(
                0,
                self.vertex_pool
                    .current_buffer()
                    .slice(blur.fullscreen_offset..),
            );
            pass.set_bind_group(0, &vertical_bind_group, &[]);
            pass.draw(0..blur.fullscreen_vertex_count, 0..1);
        }

        let composite_bind_group_entries = [
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    self.offscreen_sampled_view(&blur_target),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&scene_snapshot_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&self.text_sampler),
            },
        ];

        let composite_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-backdrop-composite-bind-group"),
            layout: &self.backdrop_composite_bind_group_layout,
            entries: &composite_bind_group_entries,
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-backdrop-composite-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.offscreen_attachment_view(target),
                    resolve_target: self.offscreen_resolve_target_for_draw(target),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if self.apply_scissor(&mut pass, blur.primitive.clip_rect) {
                pass.set_pipeline(&self.backdrop_composite_pipeline);
                pass.set_vertex_buffer(
                    0,
                    self.vertex_pool
                        .current_buffer()
                        .slice(blur.composite_offset..),
                );
                pass.set_bind_group(0, &composite_bind_group, &[]);
                pass.draw(0..blur.composite_vertex_count, 0..1);
            }
        }

        *cleared_draw_target = true;
        Ok(())
    }

    pub(super) fn apply_canvas_composite_to_target(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        composite: &PreparedCanvasComposite,
        font_manager: &FontManager,
        target: &OffscreenTarget,
        cleared_draw_target: &mut bool,
        composite_depth: usize,
    ) -> Result<(), TguiError> {
        let (composite_target, composite_mask_target) =
            self.ensure_canvas_composite_targets(composite_depth)?;

        self.clear_offscreen_target(encoder, &composite_target);
        let content_prepared = self.prepare_commands(
            DrawStream::CompositeContent {
                depth: composite_depth,
            },
            &composite.primitive.content_commands,
            font_manager,
            self.vertex_viewport(),
            &[], // canvas composite 内容不受滚动影响
            &[],
            &[],
            &std::collections::HashMap::new(),
            &[],
            None,
        )?;
        self.vertex_pool.flush(&self.device, &self.queue);
        let mut composite_cleared = true;
        self.execute_prepared_commands_to_target(
            encoder,
            &content_prepared.commands,
            font_manager,
            &composite_target,
            &mut composite_cleared,
            composite_depth + 1,
        )?;

        if let Some(mask_commands) = composite.primitive.mask_commands.as_ref() {
            self.clear_offscreen_target(encoder, &composite_mask_target);
            let mask_prepared = self.prepare_commands(
                DrawStream::CompositeMask {
                    depth: composite_depth,
                },
                mask_commands,
                font_manager,
                self.vertex_viewport(),
                &[], // mask 不受滚动影响
                &[],
                &[],
                &std::collections::HashMap::new(),
                &[],
                None,
            )?;
            self.vertex_pool.flush(&self.device, &self.queue);
            let mut mask_cleared = true;
            self.execute_prepared_commands_to_target(
                encoder,
                &mask_prepared.commands,
                font_manager,
                &composite_mask_target,
                &mut mask_cleared,
                composite_depth + 1,
            )?;
        } else {
            self.clear_offscreen_target(encoder, &composite_mask_target);
        }

        let scene_snapshot_view = self.copy_target_to_snapshot(encoder, target)?;
        let inner_shadow_rgba = composite
            .primitive
            .inner_shadow_color
            .map(|color| color.to_linear_rgba_f32())
            .unwrap_or([0.0; 4]);
        let uniform = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tgui-canvas-composite-uniform"),
                contents: bytemuck::bytes_of(&CompositeUniform {
                    data0: [
                        composite.primitive.opacity,
                        composite_blend_mode_index(composite.primitive.blend_mode) as f32,
                        if composite.primitive.mask_commands.is_some() {
                            1.0
                        } else {
                            0.0
                        },
                        composite.primitive.blur_radius.max(0.0),
                    ],
                    data1: composite
                        .primitive
                        .color_filter
                        .map(|filter| filter.multiply)
                        .unwrap_or([1.0, 1.0, 1.0, 1.0]),
                    data2: composite
                        .primitive
                        .color_filter
                        .map(|filter| filter.add)
                        .unwrap_or([0.0; 4]),
                    data3: inner_shadow_rgba,
                    data4: [
                        composite.primitive.inner_shadow_offset.x.get(),
                        composite.primitive.inner_shadow_offset.y.get(),
                        composite.primitive.inner_shadow_blur_radius.max(0.0),
                        if composite.primitive.inner_shadow_color.is_some() {
                            1.0
                        } else {
                            0.0
                        },
                    ],
                }),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let composite_canvas_entries = [
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    self.offscreen_sampled_view(&composite_target),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&scene_snapshot_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(
                    self.offscreen_sampled_view(&composite_mask_target),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&self.text_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: uniform.as_entire_binding(),
            },
        ];

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-canvas-composite-bind-group"),
            layout: &self.canvas_composite_bind_group_layout,
            entries: &composite_canvas_entries,
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tgui-canvas-composite-pass"),
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
        if self.apply_scissor(&mut pass, composite.primitive.clip_rect) {
            pass.set_pipeline(&self.canvas_composite_pipeline);
            pass.set_vertex_buffer(
                0,
                self.vertex_pool
                    .current_buffer()
                    .slice(composite.composite_offset..),
            );
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..composite.composite_vertex_count, 0..1);
        }

        Ok(())
    }

    pub(in super::super) fn blit_scene_to_surface(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_attachment_view: &wgpu::TextureView,
        resolve_target: Option<&wgpu::TextureView>,
    ) {
        let present = self
            .present_resources
            .as_ref()
            .expect("present resources initialized with scene target");
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tgui-present-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_attachment_view,
                resolve_target,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(surface_clear_color(self.clear_color)),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.text_pipeline);
        self.set_scroll_translate(&mut pass, None);
        pass.set_vertex_buffer(0, present.vertex_buffer.slice(..));
        pass.set_bind_group(0, &present.bind_group, &[]);
        pass.draw(0..present.vertex_count, 0..1);
    }
}

fn backdrop_texel_size(width: u32, height: u32) -> [f32; 2] {
    [1.0 / width.max(1) as f32, 1.0 / height.max(1) as f32]
}

fn composite_blend_mode_index(mode: CanvasBlendMode) -> u32 {
    match mode {
        CanvasBlendMode::Normal => 0,
        CanvasBlendMode::Multiply => 1,
        CanvasBlendMode::Screen => 2,
        CanvasBlendMode::Overlay => 3,
        CanvasBlendMode::Darken => 4,
        CanvasBlendMode::Lighten => 5,
        CanvasBlendMode::ColorDodge => 6,
        CanvasBlendMode::ColorBurn => 7,
        CanvasBlendMode::HardLight => 8,
        CanvasBlendMode::SoftLight => 9,
        CanvasBlendMode::Difference => 10,
        CanvasBlendMode::Exclusion => 11,
        CanvasBlendMode::Plus => 12,
    }
}
