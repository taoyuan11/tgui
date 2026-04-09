use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
#[cfg(all(target_env = "ohos", feature = "ohos"))]
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use wgpu::util::DeviceExt;

use crate::foundation::color::Color as TguiColor;
use crate::foundation::error::TguiError;
use crate::platform::backend::window::Window;
use crate::platform::dpi::PhysicalSize;
use crate::text::font::{FontCatalog, FontWeight};
use crate::ui::widget::{Rect, RenderPrimitive, ScenePrimitives, TextPrimitive};

pub enum RenderStatus {
    Rendered,
    ReconfigureSurface,
    SkipFrame,
}

pub struct Renderer {
    window: Arc<dyn Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    rect_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    text_bind_group_layout: wgpu::BindGroupLayout,
    text_sampler: wgpu::Sampler,
    size: PhysicalSize<u32>,
    clear_color: TguiColor,
    text_system: TextSystem,
    text_cache: Vec<TextCacheEntry>,
}

struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

struct TextSprite {
    bind_group: wgpu::BindGroup,
    vertices: [TextVertex; 6],
    clip_rect: Option<Rect>,
}

struct TextCacheEntry {
    key: TextCacheKey,
    bind_group: wgpu::BindGroup,
    _texture: wgpu::Texture,
}

#[derive(Clone, PartialEq, Eq)]
struct TextCacheKey {
    content: String,
    font_family: Option<String>,
    width: u32,
    height: u32,
    color: [u8; 4],
    font_size_bits: u32,
    line_height_bits: u32,
    letter_spacing_bits: u32,
    font_weight: u16,
}

impl Renderer {
    pub fn new(
        window: Arc<dyn Window>,
        clear_color: TguiColor,
        fonts: &FontCatalog,
    ) -> Result<Self, TguiError> {
        pollster::block_on(Self::new_async(window, clear_color, fonts))
    }

    async fn new_async(
        window: Arc<dyn Window>,
        clear_color: TguiColor,
        fonts: &FontCatalog,
    ) -> Result<Self, TguiError> {
        let size = window.surface_size();
        let instance = create_instance(clear_color);
        let surface = create_surface(&instance, window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: adapter_power_preference(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;
        let required_limits = required_device_limits(&adapter);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tgui-device"),
                required_features: wgpu::Features::empty(),
                required_limits,
                memory_hints: wgpu::MemoryHints::Performance,
                experimental_features: Default::default(),
                trace: Default::default(),
            })
            .await?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| caps.formats.first().copied())
            .ok_or(TguiError::NoSurfaceFormat)?;

        let alpha_mode = surface_alpha_mode(&caps.alpha_modes);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_present_mode(&caps.present_modes),
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-rect-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader/rect.wgsl").into()),
        });
        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tgui-text-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader/text.wgsl").into()),
        });

        let rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tgui-rect-pipeline-layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tgui-rect-pipeline"),
            layout: Some(&rect_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &rect_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[RectVertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &rect_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let text_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tgui-text-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tgui-text-pipeline-layout"),
            bind_group_layouts: &[Some(&text_bind_group_layout)],
            immediate_size: 0,
        });

        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tgui-text-pipeline"),
            layout: Some(&text_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[TextVertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let text_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tgui-text-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let mut font_system = FontSystem::new();
        let _ = fonts.configure_font_system(&mut font_system);

        surface.configure(&device, &config);

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            rect_pipeline,
            text_pipeline,
            text_bind_group_layout,
            text_sampler,
            size,
            clear_color,
            text_system: TextSystem {
                font_system,
                swash_cache: SwashCache::new(),
            },
            text_cache: Vec::new(),
        })
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            self.size = new_size;
            return;
        }

        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(&mut self, scene: &ScenePrimitives) -> Result<RenderStatus, TguiError> {
        if self.config.width == 0 || self.config.height == 0 {
            return Ok(RenderStatus::SkipFrame);
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                return Ok(RenderStatus::ReconfigureSurface);
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(RenderStatus::SkipFrame),
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let text_sprites = scene
            .texts
            .iter()
            .filter_map(|text| {
                self.text_bind_group_for(text)
                    .transpose()
                    .map(|bind_group| {
                        bind_group.map(|bind_group| TextSprite {
                            bind_group,
                            vertices: TextVertex::quad(
                                text.frame,
                                self.config.width as f32,
                                self.config.height as f32,
                            ),
                            clip_rect: text.clip_rect,
                        })
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tgui-render-encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
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

            pass.set_pipeline(&self.rect_pipeline);
            self.draw_rect_primitives(&mut pass, &scene.shapes);

            if !text_sprites.is_empty() {
                pass.set_pipeline(&self.text_pipeline);
                for sprite in &text_sprites {
                    if !self.apply_scissor(&mut pass, sprite.clip_rect) {
                        continue;
                    }
                    let vertex_buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("tgui-text-vertices"),
                                contents: bytemuck::cast_slice(&sprite.vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            });
                    pass.set_bind_group(0, &sprite.bind_group, &[]);
                    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    pass.draw(0..6, 0..1);
                }
            }

            pass.set_pipeline(&self.rect_pipeline);
            self.draw_rect_primitives(&mut pass, &scene.overlay_shapes);
        }

        self.queue.submit(Some(encoder.finish()));
        self.window.pre_present_notify();
        frame.present();

        Ok(RenderStatus::Rendered)
    }

    fn draw_rect_primitives<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        primitives: &[RenderPrimitive],
    ) {
        for primitive in primitives {
            if primitive.rect.width <= 0.0 || primitive.rect.height <= 0.0 {
                continue;
            }
            if !self.apply_scissor(pass, primitive.clip_rect) {
                continue;
            }

            let vertices = RectVertex::from_primitive(
                *primitive,
                self.config.width as f32,
                self.config.height as f32,
            );
            let vertex_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tgui-rect-vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..6, 0..1);
        }
    }

    fn apply_scissor<'a>(
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

    fn scissor_rect(&self, clip_rect: Option<Rect>) -> Option<(u32, u32, u32, u32)> {
        let clip_rect = clip_rect.unwrap_or(Rect::new(
            0.0,
            0.0,
            self.config.width as f32,
            self.config.height as f32,
        ));
        let x = clip_rect.x.max(0.0).floor() as u32;
        let y = clip_rect.y.max(0.0).floor() as u32;
        let right = clip_rect
            .right()
            .min(self.config.width as f32)
            .ceil()
            .max(x as f32) as u32;
        let bottom = clip_rect
            .bottom()
            .min(self.config.height as f32)
            .ceil()
            .max(y as f32) as u32;
        let width = right.saturating_sub(x);
        let height = bottom.saturating_sub(y);
        (width > 0 && height > 0).then_some((x, y, width, height))
    }

    pub fn set_clear_color(&mut self, clear_color: TguiColor) {
        self.clear_color = clear_color;
    }

    pub fn reconfigure(&mut self) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }

        self.surface.configure(&self.device, &self.config);
    }

    fn text_cache_key(text: &TextPrimitive) -> Option<TextCacheKey> {
        let width = text.frame.width.ceil().max(1.0) as u32;
        let height = text.frame.height.ceil().max(1.0) as u32;
        if width == 0 || height == 0 || text.content.is_empty() {
            return None;
        }

        Some(TextCacheKey {
            content: text.content.clone(),
            font_family: text.font_family.clone(),
            width,
            height,
            color: text.color.to_rgba8(),
            font_size_bits: text.font_size.to_bits(),
            line_height_bits: text.line_height.to_bits(),
            letter_spacing_bits: text.letter_spacing.to_bits(),
            font_weight: text.font_weight.0,
        })
    }

    fn text_bind_group_for(
        &mut self,
        text: &TextPrimitive,
    ) -> Result<Option<wgpu::BindGroup>, TguiError> {
        let Some(key) = Self::text_cache_key(text) else {
            return Ok(None);
        };

        if let Some(entry) = self.text_cache.iter().find(|entry| entry.key == key) {
            return Ok(Some(entry.bind_group.clone()));
        }

        let bind_group = match self.rasterize_text(text)? {
            Some((texture, bind_group)) => {
                self.text_cache.push(TextCacheEntry {
                    key,
                    bind_group: bind_group.clone(),
                    _texture: texture,
                });
                bind_group
            }
            None => return Ok(None),
        };

        Ok(Some(bind_group))
    }

    fn rasterize_text(
        &mut self,
        text: &TextPrimitive,
    ) -> Result<Option<(wgpu::Texture, wgpu::BindGroup)>, TguiError> {
        let width = text.frame.width.ceil().max(1.0) as u32;
        let height = text.frame.height.ceil().max(1.0) as u32;
        if width == 0 || height == 0 || text.content.is_empty() {
            return Ok(None);
        }

        let mut buffer = Buffer::new(
            &mut self.text_system.font_system,
            Metrics::new(text.font_size, text.line_height),
        );
        buffer.set_size(
            &mut self.text_system.font_system,
            Some(width as f32),
            Some(height as f32),
        );
        buffer.set_wrap(
            &mut self.text_system.font_system,
            cosmic_text::Wrap::WordOrGlyph,
        );
        let attrs = attrs_for_text(text);
        buffer.set_text(
            &mut self.text_system.font_system,
            &text.content,
            &attrs,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.text_system.font_system, false);

        let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
        buffer.draw(
            &mut self.text_system.font_system,
            &mut self.text_system.swash_cache,
            color_to_text(text.color),
            |x, y, w, h, color| {
                let rgba = color.as_rgba();
                for dy in 0..h {
                    for dx in 0..w {
                        let px = x + dx as i32;
                        let py = y + dy as i32;
                        if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                            continue;
                        }
                        blend_pixel(
                            &mut pixels,
                            width,
                            px as u32,
                            py as u32,
                            [rgba[0], rgba[1], rgba[2], rgba[3]],
                        );
                    }
                }
            },
        );

        if pixels.chunks_exact(4).all(|pixel| pixel[3] == 0) {
            return Ok(None);
        }

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tgui-text-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tgui-text-bind-group"),
            layout: &self.text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
            ],
        });

        Ok(Some((texture, bind_group)))
    }
}

fn attrs_for_text(text: &TextPrimitive) -> Attrs<'_> {
    let family = text
        .font_family
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(Family::Name)
        .unwrap_or(Family::SansSerif);

    Attrs::new()
        .family(family)
        .weight(text_weight(text.font_weight))
        .letter_spacing(text.letter_spacing / text.font_size.max(1.0))
}

fn text_weight(weight: FontWeight) -> Weight {
    Weight(weight.0)
}

fn color_to_text(color: TguiColor) -> Color {
    Color::rgba(color.r, color.g, color.b, color.a)
}

fn blend_pixel(pixels: &mut [u8], width: u32, x: u32, y: u32, src: [u8; 4]) {
    let index = ((y * width + x) * 4) as usize;
    let dst = &mut pixels[index..index + 4];

    let src_alpha = src[3] as f32 / 255.0;
    let dst_alpha = dst[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

    if out_alpha <= 0.0 {
        dst.copy_from_slice(&[0, 0, 0, 0]);
        return;
    }

    for channel in 0..3 {
        let src_value = src[channel] as f32 / 255.0;
        let dst_value = dst[channel] as f32 / 255.0;
        let out = (src_value * src_alpha + dst_value * dst_alpha * (1.0 - src_alpha)) / out_alpha;
        dst[channel] = (out * 255.0).round() as u8;
    }
    dst[3] = (out_alpha * 255.0).round() as u8;
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RectVertex {
    position: [f32; 2],
    color: [f32; 4],
    local_position: [f32; 2],
    rect_size: [f32; 2],
    corner_radius: f32,
    stroke_width: f32,
}

impl RectVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: std::mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: (std::mem::size_of::<[f32; 2]>() + std::mem::size_of::<[f32; 4]>())
                        as u64,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: (std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 4]>()
                        + std::mem::size_of::<[f32; 2]>()) as u64,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: (std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 4]>()
                        + std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 2]>()) as u64,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: (std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 4]>()
                        + std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<f32>()) as u64,
                    shader_location: 5,
                },
            ],
        }
    }

    fn from_primitive(primitive: RenderPrimitive, width: f32, height: f32) -> [Self; 6] {
        let x0 = primitive.rect.x / width * 2.0 - 1.0;
        let x1 = (primitive.rect.x + primitive.rect.width) / width * 2.0 - 1.0;
        let y0 = 1.0 - primitive.rect.y / height * 2.0;
        let y1 = 1.0 - (primitive.rect.y + primitive.rect.height) / height * 2.0;
        let color = [
            primitive.color.r as f32 / 255.0,
            primitive.color.g as f32 / 255.0,
            primitive.color.b as f32 / 255.0,
            primitive.color.a as f32 / 255.0,
        ];
        let rect_size = [
            primitive.rect.width.max(0.0),
            primitive.rect.height.max(0.0),
        ];
        let radius = primitive
            .corner_radius
            .max(0.0)
            .min(rect_size[0] * 0.5)
            .min(rect_size[1] * 0.5);
        let stroke_width = primitive
            .stroke_width
            .max(0.0)
            .min(rect_size[0] * 0.5)
            .min(rect_size[1] * 0.5);

        [
            Self {
                position: [x0, y0],
                color,
                local_position: [0.0, 0.0],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
            Self {
                position: [x1, y0],
                color,
                local_position: [rect_size[0], 0.0],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
            Self {
                position: [x1, y1],
                color,
                local_position: [rect_size[0], rect_size[1]],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
            Self {
                position: [x0, y0],
                color,
                local_position: [0.0, 0.0],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
            Self {
                position: [x1, y1],
                color,
                local_position: [rect_size[0], rect_size[1]],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
            Self {
                position: [x0, y1],
                color,
                local_position: [0.0, rect_size[1]],
                rect_size,
                corner_radius: radius,
                stroke_width,
            },
        ]
    }
}

fn instance_descriptor(clear_color: TguiColor) -> wgpu::InstanceDescriptor {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = instance_backends(clear_color);
    #[cfg(all(target_os = "android", feature = "android"))]
    {
        descriptor.flags = wgpu::InstanceFlags::empty();
        descriptor.backend_options.gl.debug_fns = wgpu::GlDebugFns::Disabled;
    }
    descriptor
}

fn create_instance(clear_color: TguiColor) -> wgpu::Instance {
    let descriptor = instance_descriptor(clear_color);
    wgpu::Instance::new(descriptor)
}

fn create_surface(
    instance: &wgpu::Instance,
    window: Arc<dyn Window>,
) -> Result<wgpu::Surface<'static>, TguiError> {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        let raw_display_handle = window
            .display_handle()
            .map_err(|error| TguiError::TextRender(format!("display handle unavailable: {error}")))?;
        let raw_window_handle = window
            .window_handle()
            .map_err(|error| TguiError::TextRender(format!("window handle unavailable: {error}")))?;

        return Ok(unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display_handle.as_raw()),
                raw_window_handle: raw_window_handle.as_raw(),
            })?
        });
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        instance.create_surface(window).map_err(Into::into)
    }
}

fn adapter_power_preference() -> wgpu::PowerPreference {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return wgpu::PowerPreference::HighPerformance;
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        wgpu::PowerPreference::default()
    }
}

fn required_device_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return adapter.limits();
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        let _ = adapter;
        wgpu::Limits::default()
    }
}

fn instance_backends(_clear_color: TguiColor) -> wgpu::Backends {
    #[cfg(target_os = "windows")]
    {
        if _clear_color.a < 255 {
            return wgpu::Backends::PRIMARY;
        }
    }

    default_backends()
}

fn default_backends() -> wgpu::Backends {
    #[cfg(target_arch = "wasm32")]
    {
        return wgpu::Backends::BROWSER_WEBGPU;
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        return wgpu::Backends::METAL;
    }

    #[cfg(all(
        target_os = "android",
        feature = "android",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    {
        return wgpu::Backends::GL;
    }

    #[cfg(any(
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos")),
        all(
            target_os = "android",
            feature = "android",
            not(any(target_arch = "x86", target_arch = "x86_64"))
        )
    ))]
    {
        return wgpu::Backends::VULKAN;
    }

    #[allow(unreachable_code)]
    wgpu::Backends::all()
}

fn surface_present_mode(modes: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Fifo)
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            })
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::AutoNoVsync)
            })
            .unwrap_or(wgpu::PresentMode::Fifo);
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::AutoNoVsync)
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            })
            .or_else(|| modes.iter().copied().find(|mode| *mode == wgpu::PresentMode::Fifo))
            .unwrap_or(wgpu::PresentMode::Fifo)
    }
}

fn surface_alpha_mode(modes: &[wgpu::CompositeAlphaMode]) -> wgpu::CompositeAlphaMode {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        modes
            .iter()
            .copied()
            .find(|mode| *mode != wgpu::CompositeAlphaMode::Opaque)
            .unwrap_or(wgpu::CompositeAlphaMode::Auto)
    }
}

fn surface_clear_color(color: TguiColor) -> wgpu::Color {
    let alpha = color.a as f64 / 255.0;
    wgpu::Color {
        r: (color.r as f64 / 255.0) * alpha,
        g: (color.g as f64 / 255.0) * alpha,
        b: (color.b as f64 / 255.0) * alpha,
        a: alpha,
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TextVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

impl TextVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: std::mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                },
            ],
        }
    }

    fn quad(rect: crate::ui::widget::Rect, width: f32, height: f32) -> [Self; 6] {
        let x0 = rect.x / width * 2.0 - 1.0;
        let x1 = (rect.x + rect.width) / width * 2.0 - 1.0;
        let y0 = 1.0 - rect.y / height * 2.0;
        let y1 = 1.0 - (rect.y + rect.height) / height * 2.0;

        [
            Self {
                position: [x0, y0],
                uv: [0.0, 0.0],
            },
            Self {
                position: [x1, y0],
                uv: [1.0, 0.0],
            },
            Self {
                position: [x1, y1],
                uv: [1.0, 1.0],
            },
            Self {
                position: [x0, y0],
                uv: [0.0, 0.0],
            },
            Self {
                position: [x1, y1],
                uv: [1.0, 1.0],
            },
            Self {
                position: [x0, y1],
                uv: [0.0, 1.0],
            },
        ]
    }
}
