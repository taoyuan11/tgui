use wgpu::util::DeviceExt;

use crate::foundation::error::TguiError;
use crate::ui::widget::Rect;

use super::prepare::{PreparedBackdropBlur, PreparedCommand};
use super::surface::surface_clear_color;
use super::{BlurUniform, Renderer, TextVertex};

impl Renderer {
    pub(super) fn execute_prepared_commands(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        commands: &[PreparedCommand],
        cleared_draw_target: &mut bool,
    ) -> Result<(), TguiError> {
        let mut index = 0;
        while index < commands.len() {
            if let PreparedCommand::BackdropBlur(blur) = &commands[index] {
                self.apply_backdrop_blur(encoder, blur)?;
                index += 1;
                continue;
            }

            let scene_view = self.scene_target_view()?;
            let msaa_view = self.msaa_target.as_ref().map(|target| &target.view);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-scene-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: msaa_view.unwrap_or(scene_view),
                    resolve_target: msaa_view.map(|_| scene_view),
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
                    PreparedCommand::BackdropBlur(_) => break,
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

    pub(super) fn apply_backdrop_blur(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        blur: &PreparedBackdropBlur,
    ) -> Result<(), TguiError> {
        let scene_target = self
            .scene_target
            .as_ref()
            .ok_or_else(|| TguiError::TextRender("scene target unavailable".into()))?;
        let blur_target = self
            .blur_target
            .as_ref()
            .ok_or_else(|| TguiError::TextRender("blur target unavailable".into()))?;
        let blur_scratch_target = self
            .blur_scratch_target
            .as_ref()
            .ok_or_else(|| TguiError::TextRender("blur scratch target unavailable".into()))?;

        let scene_snapshot = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-scene-snapshot"),
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
                texture: &scene_target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &scene_snapshot,
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
                    view: &scene_target.view,
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
