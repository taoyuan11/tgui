use wgpu::util::DeviceExt;

use crate::foundation::error::TguiError;
use crate::text::font::FontManager;
use crate::ui::unit::Dp;
use crate::ui::widget::{BrushPrimitiveData, CanvasCompositePrimitive, Rect, RenderCommand};

use super::{
    physical_mesh_clip_mask_data, BrushVertex, CompositeVertex, MeshVertex, RectVertex, Renderer,
    TextVertex,
};

pub(super) struct PreparedRect {
    pub(super) clip_rect: Option<Rect>,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
}

pub(super) struct PreparedBrush {
    pub(super) clip_rect: Option<Rect>,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
}

pub(super) struct PreparedMesh {
    pub(super) clip_rect: Option<Rect>,
    pub(super) clip_bind_group: wgpu::BindGroup,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
}

pub(super) struct PreparedSprite {
    pub(super) bind_group: wgpu::BindGroup,
    pub(super) clip_rect: Option<Rect>,
    pub(super) vertex_offset: u64,
    pub(super) vertex_count: u32,
}

pub(super) struct PreparedBackdropBlur {
    pub(super) primitive: crate::ui::widget::BackdropBlurPrimitive,
    pub(super) composite_offset: u64,
    pub(super) composite_vertex_count: u32,
    pub(super) fullscreen_offset: u64,
    pub(super) fullscreen_vertex_count: u32,
}

pub(super) struct PreparedCanvasComposite {
    pub(super) primitive: CanvasCompositePrimitive,
    pub(super) composite_offset: u64,
    pub(super) composite_vertex_count: u32,
}

pub(super) enum PreparedCommand {
    BackdropBlur(PreparedBackdropBlur),
    Rect(PreparedRect),
    Brush(PreparedBrush),
    CanvasComposite(PreparedCanvasComposite),
    Mesh(PreparedMesh),
    Sprite(PreparedSprite),
}

pub(super) struct PreparedCommands(pub(super) Vec<PreparedCommand>);

impl Renderer {
    pub(super) fn prepare_commands(
        &mut self,
        commands: &[RenderCommand],
        font_manager: &FontManager,
        logical_width: f32,
        logical_height: f32,
        physical_width: f32,
        physical_height: f32,
        scale_factor: f32,
    ) -> Result<PreparedCommands, TguiError> {
        let mut prepared = Vec::with_capacity(commands.len());

        for command in commands {
            match command {
                RenderCommand::BackdropBlur(primitive) => {
                    if primitive.rect.width <= Dp::ZERO || primitive.rect.height <= Dp::ZERO {
                        continue;
                    }
                    let fullscreen = TextVertex::quad(
                        Rect::new(0.0, 0.0, logical_width, logical_height),
                        logical_width,
                        logical_height,
                        None,
                        0.0,
                        None,
                        physical_width,
                        physical_height,
                        scale_factor,
                        1.0,
                    );
                    let fullscreen_offset =
                        self.vertex_pool.allocate(bytemuck::cast_slice(&fullscreen));
                    let vertices = CompositeVertex::quad(
                        primitive.rect,
                        logical_width,
                        logical_height,
                        primitive.corner_radius,
                        primitive.clip_mask,
                    );
                    let composite_offset =
                        self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                    prepared.push(PreparedCommand::BackdropBlur(PreparedBackdropBlur {
                        primitive: *primitive,
                        composite_offset,
                        composite_vertex_count: vertices.len() as u32,
                        fullscreen_offset,
                        fullscreen_vertex_count: fullscreen.len() as u32,
                    }));
                }
                RenderCommand::Brush(primitive) => {
                    if primitive.rect.width <= Dp::ZERO || primitive.rect.height <= Dp::ZERO {
                        continue;
                    }
                    let Some(brush_data) =
                        BrushPrimitiveData::from_background_brush(&primitive.brush, 1.0)
                    else {
                        continue;
                    };
                    let vertices = BrushVertex::from_primitive(
                        primitive.rect,
                        primitive.corner_radius,
                        brush_data,
                        physical_width,
                        physical_height,
                        scale_factor,
                    );
                    let vertex_offset = self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                    prepared.push(PreparedCommand::Brush(PreparedBrush {
                        clip_rect: primitive.clip_rect,
                        vertex_offset,
                        vertex_count: vertices.len() as u32,
                    }));
                }
                RenderCommand::CanvasComposite(primitive) => {
                    if primitive.bounds.width <= Dp::ZERO || primitive.bounds.height <= Dp::ZERO {
                        continue;
                    }
                    let vertices = CompositeVertex::quad(
                        primitive.bounds,
                        logical_width,
                        logical_height,
                        0.0,
                        primitive.clip_mask,
                    );
                    let composite_offset =
                        self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                    prepared.push(PreparedCommand::CanvasComposite(PreparedCanvasComposite {
                        primitive: (**primitive).clone(),
                        composite_offset,
                        composite_vertex_count: vertices.len() as u32,
                    }));
                }
                RenderCommand::Shape(primitive) => {
                    if primitive.rect.width <= Dp::ZERO || primitive.rect.height <= Dp::ZERO {
                        continue;
                    }
                    let vertices = RectVertex::from_primitive(
                        *primitive,
                        physical_width,
                        physical_height,
                        scale_factor,
                    );
                    let vertex_offset = self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                    prepared.push(PreparedCommand::Rect(PreparedRect {
                        clip_rect: primitive.clip_rect,
                        vertex_offset,
                        vertex_count: vertices.len() as u32,
                    }));
                }
                RenderCommand::Mesh(primitive) => {
                    if primitive.vertices.is_empty() {
                        continue;
                    }
                    let vertices: Vec<_> = primitive
                        .vertices
                        .iter()
                        .copied()
                        .map(|vertex| {
                            MeshVertex::from_scene_vertex(vertex, logical_width, logical_height)
                        })
                        .collect();
                    let vertex_offset = self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                    let clip_buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("tgui-mesh-clip-uniform"),
                                contents: bytemuck::bytes_of(&physical_mesh_clip_mask_data(
                                    primitive.clip_mask,
                                    scale_factor,
                                )),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });
                    let clip_bind_group =
                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("tgui-mesh-clip-bind-group"),
                            layout: &self.mesh_clip_bind_group_layout,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: clip_buffer.as_entire_binding(),
                            }],
                        });
                    prepared.push(PreparedCommand::Mesh(PreparedMesh {
                        clip_rect: primitive.clip_rect,
                        clip_bind_group,
                        vertex_offset,
                        vertex_count: vertices.len() as u32,
                    }));
                }
                RenderCommand::Texture(texture) => {
                    if let Some(bind_group) = self.texture_bind_group_for(&texture.texture)? {
                        let vertices = texture.quad.map_or_else(
                            || {
                                TextVertex::quad(
                                    texture.frame,
                                    logical_width,
                                    logical_height,
                                    texture.uv_rect,
                                    texture.corner_radius,
                                    texture.clip_mask,
                                    physical_width,
                                    physical_height,
                                    scale_factor,
                                    texture.opacity,
                                )
                            },
                            |quad| {
                                TextVertex::transformed(
                                    texture.frame,
                                    quad,
                                    texture.uv_rect,
                                    texture.corner_radius,
                                    texture.clip_mask,
                                    physical_width,
                                    physical_height,
                                    scale_factor,
                                    texture.opacity,
                                )
                            },
                        );
                        let vertex_offset =
                            self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                        prepared.push(PreparedCommand::Sprite(PreparedSprite {
                            bind_group,
                            clip_rect: texture.clip_rect,
                            vertex_offset,
                            vertex_count: vertices.len() as u32,
                        }));
                    }
                }
                #[cfg(feature = "video")]
                RenderCommand::VideoTexture(texture) => {
                    let Some(frame_texture) = texture.controller.current_frame() else {
                        continue;
                    };
                    if let Some(bind_group) = self.texture_bind_group_for(&frame_texture)? {
                        let vertices = texture.quad.map_or_else(
                            || {
                                TextVertex::quad(
                                    texture.frame,
                                    logical_width,
                                    logical_height,
                                    texture.uv_rect,
                                    texture.corner_radius,
                                    texture.clip_mask,
                                    physical_width,
                                    physical_height,
                                    scale_factor,
                                    texture.opacity,
                                )
                            },
                            |quad| {
                                TextVertex::transformed(
                                    texture.frame,
                                    quad,
                                    texture.uv_rect,
                                    texture.corner_radius,
                                    texture.clip_mask,
                                    physical_width,
                                    physical_height,
                                    scale_factor,
                                    texture.opacity,
                                )
                            },
                        );
                        let vertex_offset =
                            self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                        prepared.push(PreparedCommand::Sprite(PreparedSprite {
                            bind_group,
                            clip_rect: texture.clip_rect,
                            vertex_offset,
                            vertex_count: vertices.len() as u32,
                        }));
                    }
                }
                RenderCommand::Text(text) => {
                    let opacity = text.color.a as f32 / 255.0;
                    if opacity <= 0.0 {
                        continue;
                    }
                    if let Some(bind_group) = self.text_bind_group_for(text, font_manager)? {
                        let snapped_frame = self.snap_text_rect(text.frame);
                        let vertices = text.quad.map_or_else(
                            || {
                                TextVertex::quad(
                                    snapped_frame,
                                    logical_width,
                                    logical_height,
                                    None,
                                    0.0,
                                    text.clip_mask,
                                    physical_width,
                                    physical_height,
                                    scale_factor,
                                    opacity,
                                )
                            },
                            |quad| {
                                TextVertex::transformed(
                                    snapped_frame,
                                    quad,
                                    None,
                                    0.0,
                                    text.clip_mask,
                                    physical_width,
                                    physical_height,
                                    scale_factor,
                                    opacity,
                                )
                            },
                        );
                        let vertex_offset =
                            self.vertex_pool.allocate(bytemuck::cast_slice(&vertices));
                        prepared.push(PreparedCommand::Sprite(PreparedSprite {
                            bind_group,
                            clip_rect: text.clip_rect,
                            vertex_offset,
                            vertex_count: vertices.len() as u32,
                        }));
                    }
                }
            }
        }

        Ok(PreparedCommands(prepared))
    }

    pub(super) fn apply_scissor<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        clip_rect: Option<Rect>,
    ) -> bool {
        let Some((x, y, width, height)) = self.scissor_rect(clip_rect) else {
            return false;
        };
        pass.set_scissor_rect(x, y, width, height);
        true
    }

    pub(super) fn scissor_rect(&self, clip_rect: Option<Rect>) -> Option<(u32, u32, u32, u32)> {
        let (logical_width, logical_height) = self.logical_viewport_size();
        let clip_rect = clip_rect.unwrap_or(Rect::new(0.0, 0.0, logical_width, logical_height));
        let x = self.logical_to_physical(clip_rect.x.max(0.0).get()).floor() as u32;
        let y = self.logical_to_physical(clip_rect.y.max(0.0).get()).floor() as u32;
        let right = clip_rect.right().min(logical_width);
        let bottom = clip_rect.bottom().min(logical_height);
        let right = self.logical_to_physical(right.get()).ceil().max(x as f32) as u32;
        let bottom = self.logical_to_physical(bottom.get()).ceil().max(y as f32) as u32;
        let width = right.saturating_sub(x);
        let height = bottom.saturating_sub(y);
        (width > 0 && height > 0).then_some((x, y, width, height))
    }

    pub(super) fn logical_viewport_size(&self) -> (f32, f32) {
        (
            self.config.width as f32 / self.scale_factor,
            self.config.height as f32 / self.scale_factor,
        )
    }

    pub(super) fn logical_to_physical(&self, value: f32) -> f32 {
        value * self.scale_factor
    }
}
