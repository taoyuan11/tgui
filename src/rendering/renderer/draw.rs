use wgpu::util::DeviceExt;

use crate::foundation::error::TguiError;
use crate::ui::widget::{CanvasBlendMode, Rect};

use super::prepare::{PreparedBackdropBlur, PreparedCanvasComposite, PreparedCommand};
use super::surface::surface_clear_color;
use super::{
    BlurUniform, CompositeUniform, CompositeVertex, OffscreenTarget, Renderer, TextVertex,
};

impl Renderer {
    fn offscreen_target<'a>(
        &self,
        target: Option<&'a OffscreenTarget>,
        name: &str,
    ) -> Result<&'a OffscreenTarget, TguiError> {
        target.ok_or_else(|| TguiError::TextRender(format!("{name} unavailable")))
    }

    fn clear_offscreen_target(&self, encoder: &mut wgpu::CommandEncoder, target: &OffscreenTarget) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tgui-offscreen-clear-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
                resolve_target: None,
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
        source: &wgpu::Texture,
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
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: source,
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
            &scene_target.texture,
            &scene_target.view,
            cleared_draw_target,
        )
    }

    fn execute_prepared_commands_to_target(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        commands: &[PreparedCommand],
        target_texture: &wgpu::Texture,
        target_view: &wgpu::TextureView,
        cleared_draw_target: &mut bool,
    ) -> Result<(), TguiError> {
        let mut index = 0;
        while index < commands.len() {
            match &commands[index] {
                PreparedCommand::BackdropBlur(blur) => {
                    self.apply_backdrop_blur_to_target(
                        encoder,
                        blur,
                        target_texture,
                        target_view,
                        cleared_draw_target,
                    )?;
                    index += 1;
                    continue;
                }
                PreparedCommand::CanvasComposite(composite) => {
                    self.apply_canvas_composite_to_target(
                        encoder,
                        composite,
                        target_texture,
                        target_view,
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
                    view: target_view,
                    resolve_target: None,
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

    fn apply_backdrop_blur_to_target(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        blur: &PreparedBackdropBlur,
        target_texture: &wgpu::Texture,
        target_view: &wgpu::TextureView,
        _cleared_draw_target: &mut bool,
    ) -> Result<(), TguiError> {
        let blur_target = self.offscreen_target(self.blur_target.as_ref(), "blur target")?;
        let blur_scratch_target =
            self.offscreen_target(self.blur_scratch_target.as_ref(), "blur scratch target")?;

        let scene_snapshot = self.snapshot_texture(encoder, target_texture, "tgui-scene-snapshot");
        let scene_snapshot_view =
            scene_snapshot.create_view(&wgpu::TextureViewDescriptor::default());

        let full_screen = TextVertex::quad(
            Rect::new(
                0.0,
                0.0,
                self.config.width as f32 / self.scale_factor,
                self.config.height as f32 / self.scale_factor,
            ),
            self.config.width as f32 / self.scale_factor,
            self.config.height as f32 / self.scale_factor,
            0.0,
            None,
            self.config.width as f32,
            self.config.height as f32,
            1.0,
        );
        let full_screen_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tgui-backdrop-fullscreen-vertices"),
                    contents: bytemuck::cast_slice(&full_screen),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let texel_size = backdrop_texel_size(self.config.width, self.config.height);
        let horizontal_uniform =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tgui-backdrop-horizontal-uniform"),
                    contents: bytemuck::bytes_of(&BlurUniform {
                        direction: [1.0, 0.0],
                        texel_size,
                        radius: blur.primitive.blur_radius.max(0.0),
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
                    radius: blur.primitive.blur_radius.max(0.0),
                    _pad: 0.0,
                }),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let horizontal_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-backdrop-horizontal-bind-group"),
            layout: &self.backdrop_blur_bind_group_layout,
            entries: &[
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
            ],
        });
        let vertical_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-backdrop-vertical-bind-group"),
            layout: &self.backdrop_blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&blur_scratch_target.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: vertical_uniform.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-backdrop-horizontal-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &blur_scratch_target.view,
                    resolve_target: None,
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
            pass.set_vertex_buffer(0, full_screen_buffer.slice(..));
            pass.set_bind_group(0, &horizontal_bind_group, &[]);
            pass.draw(0..full_screen.len() as u32, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-backdrop-vertical-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &blur_target.view,
                    resolve_target: None,
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
            pass.set_vertex_buffer(0, full_screen_buffer.slice(..));
            pass.set_bind_group(0, &vertical_bind_group, &[]);
            pass.draw(0..full_screen.len() as u32, 0..1);
        }

        let composite_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-backdrop-composite-bind-group"),
            layout: &self.backdrop_composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&blur_target.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&scene_snapshot_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-backdrop-composite-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
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
                pass.set_vertex_buffer(0, blur.composite_buffer.slice(..));
                pass.set_bind_group(0, &composite_bind_group, &[]);
                pass.draw(0..blur.composite_vertex_count, 0..1);
            }
        }

        Ok(())
    }

    fn apply_canvas_composite_to_target(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        composite: &PreparedCanvasComposite,
        target_texture: &wgpu::Texture,
        target_view: &wgpu::TextureView,
        cleared_draw_target: &mut bool,
    ) -> Result<(), TguiError> {
        let composite_target = self
            .composite_target
            .as_ref()
            .cloned()
            .ok_or_else(|| TguiError::TextRender("composite target unavailable".into()))?;
        let composite_mask_target = self
            .composite_mask_target
            .as_ref()
            .cloned()
            .ok_or_else(|| TguiError::TextRender("composite mask target unavailable".into()))?;

        self.clear_offscreen_target(encoder, &composite_target);
        let content_prepared = self.prepare_commands(
            &composite.primitive.content_commands,
            self.config.width as f32 / self.scale_factor,
            self.config.height as f32 / self.scale_factor,
            self.config.width as f32,
            self.config.height as f32,
            self.scale_factor,
        )?;
        let mut composite_cleared = false;
        self.execute_prepared_commands_to_target(
            encoder,
            &content_prepared.0,
            &composite_target.texture,
            &composite_target.view,
            &mut composite_cleared,
        )?;

        if let Some(mask_commands) = composite.primitive.mask_commands.as_ref() {
            self.clear_offscreen_target(encoder, &composite_mask_target);
            let mask_prepared = self.prepare_commands(
                mask_commands,
                self.config.width as f32 / self.scale_factor,
                self.config.height as f32 / self.scale_factor,
                self.config.width as f32,
                self.config.height as f32,
                self.scale_factor,
            )?;
            let mut mask_cleared = false;
            self.execute_prepared_commands_to_target(
                encoder,
                &mask_prepared.0,
                &composite_mask_target.texture,
                &composite_mask_target.view,
                &mut mask_cleared,
            )?;
        } else {
            self.clear_offscreen_target(encoder, &composite_mask_target);
        }

        let scene_snapshot =
            self.snapshot_texture(encoder, target_texture, "tgui-composite-scene-snapshot");
        let scene_snapshot_view =
            scene_snapshot.create_view(&wgpu::TextureViewDescriptor::default());
        let content_view = composite_target
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mask_view = composite_mask_target
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let quad = CompositeVertex::quad(
            composite.primitive.bounds,
            self.config.width as f32 / self.scale_factor,
            self.config.height as f32 / self.scale_factor,
            0.0,
            composite.primitive.clip_mask,
        );
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tgui-canvas-composite-vertices"),
                contents: bytemuck::cast_slice(&quad),
                usage: wgpu::BufferUsages::VERTEX,
            });
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
                        0.0,
                    ],
                    data1: [0.0; 4],
                    data2: [0.0; 4],
                    data3: [0.0; 4],
                }),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-canvas-composite-bind-group"),
            layout: &self.canvas_composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&content_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&scene_snapshot_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&mask_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tgui-canvas-composite-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
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
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..quad.len() as u32, 0..1);
        }

        Ok(())
    }

    pub(super) fn blit_scene_to_surface(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scene_view: &wgpu::TextureView,
        color_attachment_view: &wgpu::TextureView,
        resolve_target: Option<&wgpu::TextureView>,
    ) {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-scene-present-bind-group"),
            layout: &self.text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
            ],
        });
        let quad = TextVertex::quad(
            Rect::new(
                0.0,
                0.0,
                self.config.width as f32 / self.scale_factor,
                self.config.height as f32 / self.scale_factor,
            ),
            self.config.width as f32 / self.scale_factor,
            self.config.height as f32 / self.scale_factor,
            0.0,
            None,
            self.config.width as f32,
            self.config.height as f32,
            1.0,
        );
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tgui-scene-present-vertices"),
                contents: bytemuck::cast_slice(&quad),
                usage: wgpu::BufferUsages::VERTEX,
            });
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
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..quad.len() as u32, 0..1);
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
