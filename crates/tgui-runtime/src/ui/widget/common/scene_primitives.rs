use super::*;
#[cfg(feature = "video")]
use crate::video::VideoController;
use smallvec::SmallVec;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static NEXT_PREPARE_CACHE_SERIAL: AtomicU64 = AtomicU64::new(1);

fn next_prepare_cache_serial() -> u64 {
    NEXT_PREPARE_CACHE_SERIAL.fetch_add(1, Ordering::Relaxed)
}

pub(crate) type TransformChain = SmallVec<[WidgetId; 2]>;

/// A primitive whose bounding `rect` lies entirely outside its own `clip_rect`
/// is scissored to zero pixels by the renderer (see `Renderer::scissor_rect`),
/// so dropping it from the scene produces identical output. Returning `true`
/// here lets the collect phase cull off-screen scroll content cheaply, which
/// also keeps the per-container `ComputedScene` clones small.
#[inline]
fn clipped_out(rect: Rect, clip_rect: Option<Rect>) -> bool {
    match clip_rect {
        Some(clip) => rect.fully_outside(clip),
        None => false,
    }
}

#[derive(Clone, Copy)]
pub struct RenderPrimitive {
    pub rect: Rect,
    pub color: Color,
    pub corner_radius: f32,
    pub stroke_width: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ShapePrimitiveSlot {
    pub(crate) shape_index: usize,
    pub(crate) command_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OverlayShapePrimitiveSlot {
    pub(crate) shape_index: usize,
    pub(crate) command_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BackdropBlurPrimitiveSlot {
    pub(crate) backdrop_blur_index: usize,
    pub(crate) command_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TextPrimitiveSlot {
    pub(crate) text_index: usize,
    pub(crate) command_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OverlayTextPrimitiveSlot {
    pub(crate) text_index: usize,
    pub(crate) command_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TextDecorationPrimitiveSlot {
    pub(crate) text_decoration_index: usize,
    pub(crate) command_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OverlayTextDecorationPrimitiveSlot {
    pub(crate) text_decoration_index: usize,
    pub(crate) command_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TexturePrimitiveSlot {
    pub(crate) texture_index: usize,
    pub(crate) command_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OverlayTexturePrimitiveSlot {
    pub(crate) texture_index: usize,
    pub(crate) command_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BrushPrimitiveSlot {
    pub(crate) brush_index: usize,
    pub(crate) command_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SceneDrawStream {
    Main,
    Overlay,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirtyDrawRange {
    pub(crate) stream: SceneDrawStream,
    pub(crate) range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushPrimitive {
    pub rect: Rect,
    pub brush: BackgroundBrush,
    pub corner_radius: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackdropBlurPrimitive {
    pub rect: Rect,
    pub corner_radius: f32,
    pub blur_radius: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasTextSpanPrimitive {
    pub content: Arc<str>,
    pub font_family: Option<Arc<str>>,
    pub color: Color,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextPrimitive {
    pub content: Arc<str>,
    pub rich_spans: Option<Arc<[CanvasTextSpanPrimitive]>>,
    pub frame: Rect,
    pub quad: Option<[Point; 4]>,
    pub color: Color,
    pub force_color: bool,
    pub font_family: Option<Arc<str>>,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub wrap: CanvasTextWrap,
    pub overflow: CanvasTextOverflow,
    pub horizontal_align: CanvasTextHorizontalAlign,
    pub vertical_align: CanvasTextVerticalAlign,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextDecorationPrimitive {
    pub segments: Arc<[Rect]>,
    pub color: Color,
    pub corner_radius: f32,
    pub stroke_width: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone)]
pub struct TexturePrimitive {
    pub texture: Arc<TextureFrame>,
    pub media_key: Option<crate::media::MediaTextureKey>,
    pub(crate) media_layout: Option<crate::media::MediaTextureLayout>,
    pub frame: Rect,
    pub quad: Option<[Point; 4]>,
    pub uv_rect: Option<Rect>,
    pub corner_radius: f32,
    pub opacity: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[cfg(feature = "video")]
#[derive(Clone)]
pub struct VideoTexturePrimitive {
    pub controller: VideoController,
    pub frame: Rect,
    pub quad: Option<[Point; 4]>,
    pub uv_rect: Option<Rect>,
    pub corner_radius: f32,
    pub opacity: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone)]
pub struct CanvasCompositePrimitive {
    pub bounds: Rect,
    pub opacity: f32,
    pub blend_mode: CanvasBlendMode,
    pub blur_radius: f32,
    pub color_filter: Option<CanvasColorFilter>,
    pub inner_shadow_color: Option<Color>,
    pub inner_shadow_offset: Point,
    pub inner_shadow_blur_radius: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
    pub content_commands: Arc<[RenderCommand]>,
    pub mask_commands: Option<Arc<[RenderCommand]>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipMask {
    pub rect: Rect,
    pub corner_radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    pub position: [f32; 2],
    pub local_position: [f32; 2],
    pub brush_meta: [f32; 4],
    pub gradient_data0: [f32; 4],
    pub gradient_data1: [f32; 4],
    pub stop_offsets0: [f32; 4],
    pub stop_offsets1: [f32; 4],
    pub stop_colors: [[f32; 4]; 8],
}

#[derive(Clone)]
pub struct MeshPrimitive {
    pub vertices: Arc<[MeshVertex]>,
    pub(crate) triangles: Arc<[[Point; 3]]>,
    pub clip_rect: Option<Rect>,
    #[allow(dead_code)]
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone)]
pub(crate) enum RenderCommand {
    BackdropBlur(BackdropBlurPrimitive),
    Brush(BrushPrimitive),
    CanvasComposite(Box<CanvasCompositePrimitive>),
    Shape(RenderPrimitive),
    Texture(TexturePrimitive),
    #[cfg(feature = "video")]
    VideoTexture(VideoTexturePrimitive),
    Text(Box<TextPrimitive>),
    TextDecoration(TextDecorationPrimitive),
    Mesh(MeshPrimitive),
}

#[derive(Clone, Default)]
pub struct ScenePrimitives {
    pub backdrop_blurs: SmallVec<[BackdropBlurPrimitive; 1]>,
    pub brushes: SmallVec<[BrushPrimitive; 1]>,
    pub canvas_composites: SmallVec<[CanvasCompositePrimitive; 1]>,
    pub shapes: SmallVec<[RenderPrimitive; 1]>,
    pub meshes: SmallVec<[MeshPrimitive; 1]>,
    pub textures: SmallVec<[TexturePrimitive; 1]>,
    #[cfg(feature = "video")]
    pub video_textures: SmallVec<[VideoTexturePrimitive; 1]>,
    pub texts: SmallVec<[TextPrimitive; 1]>,
    pub text_decorations: SmallVec<[TextDecorationPrimitive; 1]>,
    pub overlay_shapes: SmallVec<[RenderPrimitive; 1]>,
    pub overlay_textures: SmallVec<[TexturePrimitive; 1]>,
    #[allow(dead_code)]
    pub overlay_meshes: SmallVec<[MeshPrimitive; 1]>,
    #[allow(dead_code)]
    pub overlay_texts: SmallVec<[TextPrimitive; 1]>,
    pub overlay_text_decorations: SmallVec<[TextDecorationPrimitive; 1]>,
    pub(crate) commands: SmallVec<[RenderCommand; 1]>,
    pub(crate) overlay_commands: SmallVec<[RenderCommand; 1]>,
    pub(crate) overlay_command_sources: SmallVec<[Option<WidgetId>; 1]>,
    pub(crate) command_gpu_scroll_containers: SmallVec<[Option<WidgetId>; 1]>,
    pub(crate) overlay_command_gpu_scroll_containers: SmallVec<[Option<WidgetId>; 1]>,
    pub(crate) command_transform_chains: SmallVec<[TransformChain; 1]>,
    pub(crate) overlay_command_transform_chains: SmallVec<[TransformChain; 1]>,
    pub(crate) dirty_draw_ranges: SmallVec<[DirtyDrawRange; 4]>,
    pub(crate) prepare_cache_serial: u64,
    active_gpu_scroll_container: Option<WidgetId>,
    active_transform_chain: TransformChain,
}

impl ScenePrimitives {
    pub(crate) fn new_prepare_cache_root() -> Self {
        Self {
            prepare_cache_serial: next_prepare_cache_serial(),
            ..Self::default()
        }
    }

    pub(crate) fn prepare_cache_serial(&self) -> u64 {
        self.prepare_cache_serial
    }

    pub(crate) fn assign_new_prepare_cache_serial(&mut self) {
        self.prepare_cache_serial = next_prepare_cache_serial();
    }

    pub(crate) fn clear_dirty_draw_ranges(&mut self) {
        self.dirty_draw_ranges.clear();
    }

    pub(crate) fn set_active_gpu_scroll_container(&mut self, id: Option<WidgetId>) {
        self.active_gpu_scroll_container = id;
    }

    pub(crate) fn fill_gpu_scroll_container(&mut self, id: WidgetId) {
        for slot in &mut self.command_gpu_scroll_containers {
            if slot.is_none() {
                *slot = Some(id);
            }
        }
        for slot in &mut self.overlay_command_gpu_scroll_containers {
            if slot.is_none() {
                *slot = Some(id);
            }
        }
    }

    pub(crate) fn command_gpu_scroll_containers(&self) -> &[Option<WidgetId>] {
        &self.command_gpu_scroll_containers
    }

    pub(crate) fn overlay_command_gpu_scroll_containers(&self) -> &[Option<WidgetId>] {
        &self.overlay_command_gpu_scroll_containers
    }

    #[allow(dead_code)]
    pub(crate) fn overlay_command_sources(&self) -> &[Option<WidgetId>] {
        &self.overlay_command_sources
    }

    pub(crate) fn command_transform_chains(&self) -> &[TransformChain] {
        &self.command_transform_chains
    }

    pub(crate) fn overlay_command_transform_chains(&self) -> &[TransformChain] {
        &self.overlay_command_transform_chains
    }

    pub(crate) fn dirty_draw_ranges(&self) -> &[DirtyDrawRange] {
        &self.dirty_draw_ranges
    }

    fn mark_dirty_draw(&mut self, stream: SceneDrawStream, command_index: usize) {
        if self.dirty_draw_ranges.iter().any(|range| {
            range.stream == stream
                && range.range.start <= command_index
                && command_index < range.range.end
        }) {
            return;
        }
        self.dirty_draw_ranges.push(DirtyDrawRange {
            stream,
            range: command_index..(command_index + 1),
        });
    }

    pub(crate) fn set_active_transform_chain(&mut self, chain: &[WidgetId]) {
        self.active_transform_chain.clear();
        self.active_transform_chain.extend(chain.iter().copied());
    }

    fn should_cull_clipped_out(&self) -> bool {
        self.active_gpu_scroll_container.is_none()
    }

    fn push_command(&mut self, command: RenderCommand) {
        self.commands.push(command);
        self.command_gpu_scroll_containers
            .push(self.active_gpu_scroll_container);
        self.command_transform_chains
            .push(self.active_transform_chain.clone());
    }

    fn push_overlay_command(&mut self, command: RenderCommand) {
        self.overlay_commands.push(command);
        self.overlay_command_sources.push(None);
        self.overlay_command_gpu_scroll_containers
            .push(self.active_gpu_scroll_container);
        self.overlay_command_transform_chains
            .push(self.active_transform_chain.clone());
    }

    pub(crate) fn delta_since(&self, base: &ScenePrimitives) -> ScenePrimitives {
        let mut delta = ScenePrimitives::default();
        delta.prepare_cache_serial = self.prepare_cache_serial;
        delta.backdrop_blurs.extend(
            self.backdrop_blurs
                .iter()
                .skip(base.backdrop_blurs.len())
                .copied(),
        );
        delta
            .brushes
            .extend(self.brushes.iter().skip(base.brushes.len()).cloned());
        delta.canvas_composites.extend(
            self.canvas_composites
                .iter()
                .skip(base.canvas_composites.len())
                .cloned(),
        );
        delta
            .shapes
            .extend(self.shapes.iter().skip(base.shapes.len()).copied());
        delta
            .meshes
            .extend(self.meshes.iter().skip(base.meshes.len()).cloned());
        delta
            .textures
            .extend(self.textures.iter().skip(base.textures.len()).cloned());
        #[cfg(feature = "video")]
        delta.video_textures.extend(
            self.video_textures
                .iter()
                .skip(base.video_textures.len())
                .cloned(),
        );
        delta
            .texts
            .extend(self.texts.iter().skip(base.texts.len()).cloned());
        delta.text_decorations.extend(
            self.text_decorations
                .iter()
                .skip(base.text_decorations.len())
                .cloned(),
        );
        delta.overlay_shapes.extend(
            self.overlay_shapes
                .iter()
                .skip(base.overlay_shapes.len())
                .copied(),
        );
        delta.overlay_textures.extend(
            self.overlay_textures
                .iter()
                .skip(base.overlay_textures.len())
                .cloned(),
        );
        delta.overlay_meshes.extend(
            self.overlay_meshes
                .iter()
                .skip(base.overlay_meshes.len())
                .cloned(),
        );
        delta.overlay_texts.extend(
            self.overlay_texts
                .iter()
                .skip(base.overlay_texts.len())
                .cloned(),
        );
        delta.overlay_text_decorations.extend(
            self.overlay_text_decorations
                .iter()
                .skip(base.overlay_text_decorations.len())
                .cloned(),
        );
        delta
            .commands
            .extend(self.commands.iter().skip(base.commands.len()).cloned());
        delta.overlay_commands.extend(
            self.overlay_commands
                .iter()
                .skip(base.overlay_commands.len())
                .cloned(),
        );
        delta.overlay_command_sources.extend(
            self.overlay_command_sources
                .iter()
                .skip(base.overlay_command_sources.len())
                .copied(),
        );
        {
            delta.command_gpu_scroll_containers.extend(
                self.command_gpu_scroll_containers
                    .iter()
                    .skip(base.command_gpu_scroll_containers.len())
                    .copied(),
            );
            delta.overlay_command_gpu_scroll_containers.extend(
                self.overlay_command_gpu_scroll_containers
                    .iter()
                    .skip(base.overlay_command_gpu_scroll_containers.len())
                    .copied(),
            );
            delta.command_transform_chains.extend(
                self.command_transform_chains
                    .iter()
                    .skip(base.command_transform_chains.len())
                    .cloned(),
            );
            delta.overlay_command_transform_chains.extend(
                self.overlay_command_transform_chains
                    .iter()
                    .skip(base.overlay_command_transform_chains.len())
                    .cloned(),
            );
        }
        delta
            .dirty_draw_ranges
            .extend(self.dirty_draw_ranges.iter().cloned());
        delta
    }

    pub(crate) fn push_render_command(&mut self, command: RenderCommand) {
        match command {
            RenderCommand::BackdropBlur(primitive) => self.push_backdrop_blur(primitive),
            RenderCommand::Brush(primitive) => self.push_brush(primitive),
            RenderCommand::CanvasComposite(primitive) => self.push_canvas_composite(*primitive),
            RenderCommand::Shape(primitive) => self.push_shape(primitive),
            RenderCommand::Texture(primitive) => self.push_texture(primitive),
            #[cfg(feature = "video")]
            RenderCommand::VideoTexture(primitive) => self.push_video_texture(primitive),
            RenderCommand::Text(primitive) => self.push_text(*primitive),
            RenderCommand::TextDecoration(primitive) => self.push_text_decoration(primitive),
            RenderCommand::Mesh(primitive) => self.push_mesh(primitive),
        }
    }

    pub(crate) fn push_backdrop_blur(&mut self, primitive: BackdropBlurPrimitive) {
        if self.should_cull_clipped_out() && clipped_out(primitive.rect, primitive.clip_rect) {
            return;
        }
        self.backdrop_blurs.push(primitive);
        self.push_command(RenderCommand::BackdropBlur(primitive));
    }

    pub(crate) fn push_brush(&mut self, primitive: BrushPrimitive) {
        if self.should_cull_clipped_out() && clipped_out(primitive.rect, primitive.clip_rect) {
            return;
        }
        self.brushes.push(primitive.clone());
        self.push_command(RenderCommand::Brush(primitive));
    }

    pub(crate) fn push_canvas_composite(&mut self, primitive: CanvasCompositePrimitive) {
        if self.should_cull_clipped_out() && clipped_out(primitive.bounds, primitive.clip_rect) {
            return;
        }
        self.canvas_composites.push(primitive.clone());
        self.push_command(RenderCommand::CanvasComposite(Box::new(primitive)));
    }

    pub(crate) fn push_shape(&mut self, primitive: RenderPrimitive) {
        if self.should_cull_clipped_out() && clipped_out(primitive.rect, primitive.clip_rect) {
            return;
        }
        self.shapes.push(primitive);
        self.push_command(RenderCommand::Shape(primitive));
    }

    pub(crate) fn push_mesh(&mut self, primitive: MeshPrimitive) {
        self.meshes.push(primitive.clone());
        self.push_command(RenderCommand::Mesh(primitive));
    }

    pub(crate) fn push_texture(&mut self, primitive: TexturePrimitive) {
        if self.should_cull_clipped_out() && clipped_out(primitive.frame, primitive.clip_rect) {
            return;
        }
        self.textures.push(primitive.clone());
        self.push_command(RenderCommand::Texture(primitive));
    }

    #[cfg(feature = "video")]
    pub(crate) fn push_video_texture(&mut self, primitive: VideoTexturePrimitive) {
        if self.should_cull_clipped_out() && clipped_out(primitive.frame, primitive.clip_rect) {
            return;
        }
        self.video_textures.push(primitive.clone());
        self.push_command(RenderCommand::VideoTexture(primitive));
    }

    pub(crate) fn push_text(&mut self, primitive: TextPrimitive) {
        if self.should_cull_clipped_out() && clipped_out(primitive.frame, primitive.clip_rect) {
            return;
        }
        self.texts.push(primitive.clone());
        self.push_command(RenderCommand::Text(Box::new(primitive)));
    }

    pub(crate) fn push_text_decoration(&mut self, primitive: TextDecorationPrimitive) {
        if primitive.segments.is_empty() {
            return;
        }
        if self.should_cull_clipped_out()
            && primitive
                .segments
                .iter()
                .all(|rect| clipped_out(*rect, primitive.clip_rect))
        {
            return;
        }
        self.text_decorations.push(primitive.clone());
        self.push_command(RenderCommand::TextDecoration(primitive));
    }

    pub(crate) fn push_overlay_shape(&mut self, primitive: RenderPrimitive) {
        self.overlay_shapes.push(primitive);
        self.push_overlay_command(RenderCommand::Shape(primitive));
    }

    #[allow(dead_code)]
    pub(crate) fn push_overlay_texture(&mut self, primitive: TexturePrimitive) {
        self.overlay_textures.push(primitive.clone());
        self.push_overlay_command(RenderCommand::Texture(primitive));
    }

    #[allow(dead_code)]
    pub(crate) fn push_overlay_mesh(&mut self, primitive: MeshPrimitive) {
        self.overlay_meshes.push(primitive.clone());
        self.push_overlay_command(RenderCommand::Mesh(primitive));
    }

    #[allow(dead_code)]
    pub(crate) fn push_overlay_text(&mut self, primitive: TextPrimitive) {
        self.overlay_texts.push(primitive.clone());
        self.push_overlay_command(RenderCommand::Text(Box::new(primitive)));
    }

    pub(crate) fn push_overlay_text_decoration(&mut self, primitive: TextDecorationPrimitive) {
        if primitive.segments.is_empty() {
            return;
        }
        self.overlay_text_decorations.push(primitive.clone());
        self.push_overlay_command(RenderCommand::TextDecoration(primitive));
    }

    pub(crate) fn matching_shape_slots(
        &self,
        mut matches: impl FnMut(&RenderPrimitive) -> bool,
    ) -> SmallVec<[ShapePrimitiveSlot; 4]> {
        let mut slots = SmallVec::new();
        let mut shape_index = 0usize;
        for (command_index, command) in self.commands.iter().enumerate() {
            if let RenderCommand::Shape(primitive) = command {
                if matches(primitive) {
                    slots.push(ShapePrimitiveSlot {
                        shape_index,
                        command_index,
                    });
                }
                shape_index += 1;
            }
        }
        debug_assert_eq!(
            shape_index,
            self.shapes.len(),
            "shape stream and command stream should remain in lockstep"
        );
        slots
    }

    pub(crate) fn matching_brush_slots(
        &self,
        mut matches: impl FnMut(&BrushPrimitive) -> bool,
    ) -> SmallVec<[BrushPrimitiveSlot; 2]> {
        let mut slots = SmallVec::new();
        let mut brush_index = 0usize;
        for (command_index, command) in self.commands.iter().enumerate() {
            if let RenderCommand::Brush(primitive) = command {
                if matches(primitive) {
                    slots.push(BrushPrimitiveSlot {
                        brush_index,
                        command_index,
                    });
                }
                brush_index += 1;
            }
        }
        debug_assert_eq!(
            brush_index,
            self.brushes.len(),
            "brush stream and command stream should remain in lockstep"
        );
        slots
    }

    #[allow(dead_code)]
    pub(crate) fn matching_overlay_shape_slots(
        &self,
        mut matches: impl FnMut(&RenderPrimitive) -> bool,
    ) -> SmallVec<[OverlayShapePrimitiveSlot; 4]> {
        let mut slots = SmallVec::new();
        let mut shape_index = 0usize;
        for (command_index, command) in self.overlay_commands.iter().enumerate() {
            if let RenderCommand::Shape(primitive) = command {
                if matches(primitive) {
                    slots.push(OverlayShapePrimitiveSlot {
                        shape_index,
                        command_index,
                    });
                }
                shape_index += 1;
            }
        }
        debug_assert_eq!(
            shape_index,
            self.overlay_shapes.len(),
            "overlay shape stream and command stream should remain in lockstep"
        );
        slots
    }

    pub(crate) fn matching_backdrop_blur_slots(
        &self,
        mut matches: impl FnMut(&BackdropBlurPrimitive) -> bool,
    ) -> SmallVec<[BackdropBlurPrimitiveSlot; 2]> {
        let mut slots = SmallVec::new();
        let mut backdrop_blur_index = 0usize;
        for (command_index, command) in self.commands.iter().enumerate() {
            if let RenderCommand::BackdropBlur(primitive) = command {
                if matches(primitive) {
                    slots.push(BackdropBlurPrimitiveSlot {
                        backdrop_blur_index,
                        command_index,
                    });
                }
                backdrop_blur_index += 1;
            }
        }
        debug_assert_eq!(
            backdrop_blur_index,
            self.backdrop_blurs.len(),
            "backdrop blur stream and command stream should remain in lockstep"
        );
        slots
    }

    #[allow(dead_code)]
    pub(crate) fn matching_overlay_text_slots(
        &self,
        mut matches: impl FnMut(&TextPrimitive) -> bool,
    ) -> SmallVec<[OverlayTextPrimitiveSlot; 2]> {
        let mut slots = SmallVec::new();
        let mut text_index = 0usize;
        for (command_index, command) in self.overlay_commands.iter().enumerate() {
            if let RenderCommand::Text(primitive) = command {
                if matches(primitive) {
                    slots.push(OverlayTextPrimitiveSlot {
                        text_index,
                        command_index,
                    });
                }
                text_index += 1;
            }
        }
        debug_assert_eq!(
            text_index,
            self.overlay_texts.len(),
            "overlay text stream and command stream should remain in lockstep"
        );
        slots
    }

    pub(crate) fn matching_text_slots(
        &self,
        mut matches: impl FnMut(&TextPrimitive) -> bool,
    ) -> SmallVec<[TextPrimitiveSlot; 2]> {
        let mut slots = SmallVec::new();
        let mut text_index = 0usize;
        for (command_index, command) in self.commands.iter().enumerate() {
            if let RenderCommand::Text(primitive) = command {
                if matches(primitive) {
                    slots.push(TextPrimitiveSlot {
                        text_index,
                        command_index,
                    });
                }
                text_index += 1;
            }
        }
        debug_assert_eq!(
            text_index,
            self.texts.len(),
            "text stream and command stream should remain in lockstep"
        );
        slots
    }

    pub(crate) fn matching_text_decoration_slots(
        &self,
        mut matches: impl FnMut(&TextDecorationPrimitive) -> bool,
    ) -> SmallVec<[TextDecorationPrimitiveSlot; 2]> {
        let mut slots = SmallVec::new();
        let mut text_decoration_index = 0usize;
        for (command_index, command) in self.commands.iter().enumerate() {
            if let RenderCommand::TextDecoration(primitive) = command {
                if matches(primitive) {
                    slots.push(TextDecorationPrimitiveSlot {
                        text_decoration_index,
                        command_index,
                    });
                }
                text_decoration_index += 1;
            }
        }
        debug_assert_eq!(
            text_decoration_index,
            self.text_decorations.len(),
            "text decoration stream and command stream should remain in lockstep"
        );
        slots
    }

    pub(crate) fn matching_overlay_text_decoration_slots(
        &self,
        mut matches: impl FnMut(&TextDecorationPrimitive) -> bool,
    ) -> SmallVec<[OverlayTextDecorationPrimitiveSlot; 2]> {
        let mut slots = SmallVec::new();
        let mut text_decoration_index = 0usize;
        for (command_index, command) in self.overlay_commands.iter().enumerate() {
            if let RenderCommand::TextDecoration(primitive) = command {
                if matches(primitive) {
                    slots.push(OverlayTextDecorationPrimitiveSlot {
                        text_decoration_index,
                        command_index,
                    });
                }
                text_decoration_index += 1;
            }
        }
        debug_assert_eq!(
            text_decoration_index,
            self.overlay_text_decorations.len(),
            "overlay text decoration stream and command stream should remain in lockstep"
        );
        slots
    }

    pub(crate) fn matching_texture_slots(
        &self,
        mut matches: impl FnMut(&TexturePrimitive) -> bool,
    ) -> SmallVec<[TexturePrimitiveSlot; 2]> {
        let mut slots = SmallVec::new();
        let mut texture_index = 0usize;
        for (command_index, command) in self.commands.iter().enumerate() {
            if let RenderCommand::Texture(primitive) = command {
                if matches(primitive) {
                    slots.push(TexturePrimitiveSlot {
                        texture_index,
                        command_index,
                    });
                }
                texture_index += 1;
            }
        }
        debug_assert_eq!(
            texture_index,
            self.textures.len(),
            "texture stream and command stream should remain in lockstep"
        );
        slots
    }

    #[allow(dead_code)]
    pub(crate) fn matching_overlay_texture_slots(
        &self,
        mut matches: impl FnMut(&TexturePrimitive) -> bool,
    ) -> SmallVec<[OverlayTexturePrimitiveSlot; 2]> {
        let mut slots = SmallVec::new();
        let mut texture_index = 0usize;
        for (command_index, command) in self.overlay_commands.iter().enumerate() {
            if let RenderCommand::Texture(primitive) = command {
                if matches(primitive) {
                    slots.push(OverlayTexturePrimitiveSlot {
                        texture_index,
                        command_index,
                    });
                }
                texture_index += 1;
            }
        }
        debug_assert_eq!(
            texture_index,
            self.overlay_textures.len(),
            "overlay texture stream and command stream should remain in lockstep"
        );
        slots
    }

    pub(crate) fn can_write_backdrop_blur_slot(
        &self,
        offset: &SceneCounts,
        slot: BackdropBlurPrimitiveSlot,
    ) -> bool {
        let Some(backdrop_blur_index) = offset.backdrop_blurs.checked_add(slot.backdrop_blur_index)
        else {
            return false;
        };
        let Some(command_index) = offset.commands.checked_add(slot.command_index) else {
            return false;
        };
        self.backdrop_blurs.get(backdrop_blur_index).is_some()
            && matches!(
                self.commands.get(command_index),
                Some(RenderCommand::BackdropBlur(_))
            )
    }

    pub(crate) fn write_backdrop_blur_slot(
        &mut self,
        offset: &SceneCounts,
        slot: BackdropBlurPrimitiveSlot,
        primitive: BackdropBlurPrimitive,
    ) -> bool {
        if !self.can_write_backdrop_blur_slot(offset, slot) {
            return false;
        }
        let backdrop_blur_index = offset.backdrop_blurs + slot.backdrop_blur_index;
        let command_index = offset.commands + slot.command_index;
        self.backdrop_blurs[backdrop_blur_index] = primitive;
        match &mut self.commands[command_index] {
            RenderCommand::BackdropBlur(target) => {
                *target = primitive;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn can_write_shape_color_slot(
        &self,
        offset: &SceneCounts,
        slot: ShapePrimitiveSlot,
    ) -> bool {
        let Some(shape_index) = offset.shapes.checked_add(slot.shape_index) else {
            return false;
        };
        let Some(command_index) = offset.commands.checked_add(slot.command_index) else {
            return false;
        };
        self.shapes.get(shape_index).is_some()
            && matches!(
                self.commands.get(command_index),
                Some(RenderCommand::Shape(_))
            )
    }

    pub(crate) fn write_shape_color_slot(
        &mut self,
        offset: &SceneCounts,
        slot: ShapePrimitiveSlot,
        color: Color,
    ) -> bool {
        if !self.can_write_shape_color_slot(offset, slot) {
            return false;
        }
        let shape_index = offset.shapes + slot.shape_index;
        let command_index = offset.commands + slot.command_index;
        self.shapes[shape_index].color = color;
        match &mut self.commands[command_index] {
            RenderCommand::Shape(primitive) => {
                primitive.color = color;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn can_write_brush_slot(
        &self,
        offset: &SceneCounts,
        slot: BrushPrimitiveSlot,
    ) -> bool {
        let Some(brush_index) = offset.brushes.checked_add(slot.brush_index) else {
            return false;
        };
        let Some(command_index) = offset.commands.checked_add(slot.command_index) else {
            return false;
        };
        self.brushes.get(brush_index).is_some()
            && matches!(
                self.commands.get(command_index),
                Some(RenderCommand::Brush(_))
            )
    }

    pub(crate) fn write_brush_slot(
        &mut self,
        offset: &SceneCounts,
        slot: BrushPrimitiveSlot,
        primitive: BrushPrimitive,
    ) -> bool {
        if !self.can_write_brush_slot(offset, slot) {
            return false;
        }
        let brush_index = offset.brushes + slot.brush_index;
        let command_index = offset.commands + slot.command_index;
        self.brushes[brush_index] = primitive.clone();
        match &mut self.commands[command_index] {
            RenderCommand::Brush(target) => {
                *target = primitive;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn write_shape_rect_slot(
        &mut self,
        offset: &SceneCounts,
        slot: ShapePrimitiveSlot,
        rect: Rect,
    ) -> bool {
        if !self.can_write_shape_color_slot(offset, slot) {
            return false;
        }
        let shape_index = offset.shapes + slot.shape_index;
        let command_index = offset.commands + slot.command_index;
        self.shapes[shape_index].rect = rect;
        match &mut self.commands[command_index] {
            RenderCommand::Shape(primitive) => {
                primitive.rect = rect;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn write_shape_corner_radius_slot(
        &mut self,
        offset: &SceneCounts,
        slot: ShapePrimitiveSlot,
        corner_radius: f32,
    ) -> bool {
        if !self.can_write_shape_color_slot(offset, slot) {
            return false;
        }
        let shape_index = offset.shapes + slot.shape_index;
        let command_index = offset.commands + slot.command_index;
        self.shapes[shape_index].corner_radius = corner_radius;
        match &mut self.commands[command_index] {
            RenderCommand::Shape(primitive) => {
                primitive.corner_radius = corner_radius;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn write_shape_stroke_width_slot(
        &mut self,
        offset: &SceneCounts,
        slot: ShapePrimitiveSlot,
        stroke_width: f32,
    ) -> bool {
        if !self.can_write_shape_color_slot(offset, slot) {
            return false;
        }
        let shape_index = offset.shapes + slot.shape_index;
        let command_index = offset.commands + slot.command_index;
        self.shapes[shape_index].stroke_width = stroke_width;
        match &mut self.commands[command_index] {
            RenderCommand::Shape(primitive) => {
                primitive.stroke_width = stroke_width;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn can_write_overlay_shape_slot(
        &self,
        offset: &SceneCounts,
        slot: OverlayShapePrimitiveSlot,
    ) -> bool {
        let Some(shape_index) = offset.overlay_shapes.checked_add(slot.shape_index) else {
            return false;
        };
        let Some(command_index) = offset.overlay_commands.checked_add(slot.command_index) else {
            return false;
        };
        self.overlay_shapes.get(shape_index).is_some()
            && matches!(
                self.overlay_commands.get(command_index),
                Some(RenderCommand::Shape(_))
            )
    }

    #[allow(dead_code)]
    pub(crate) fn write_overlay_shape_color_slot(
        &mut self,
        offset: &SceneCounts,
        slot: OverlayShapePrimitiveSlot,
        color: Color,
    ) -> bool {
        if !self.can_write_overlay_shape_slot(offset, slot) {
            return false;
        }
        let shape_index = offset.overlay_shapes + slot.shape_index;
        let command_index = offset.overlay_commands + slot.command_index;
        self.overlay_shapes[shape_index].color = color;
        match &mut self.overlay_commands[command_index] {
            RenderCommand::Shape(primitive) => {
                primitive.color = color;
                self.mark_dirty_draw(SceneDrawStream::Overlay, command_index);
                true
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_overlay_shape_rect_slot(
        &mut self,
        offset: &SceneCounts,
        slot: OverlayShapePrimitiveSlot,
        rect: Rect,
    ) -> bool {
        if !self.can_write_overlay_shape_slot(offset, slot) {
            return false;
        }
        let shape_index = offset.overlay_shapes + slot.shape_index;
        let command_index = offset.overlay_commands + slot.command_index;
        self.overlay_shapes[shape_index].rect = rect;
        match &mut self.overlay_commands[command_index] {
            RenderCommand::Shape(primitive) => {
                primitive.rect = rect;
                self.mark_dirty_draw(SceneDrawStream::Overlay, command_index);
                true
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_overlay_shape_corner_radius_slot(
        &mut self,
        offset: &SceneCounts,
        slot: OverlayShapePrimitiveSlot,
        corner_radius: f32,
    ) -> bool {
        if !self.can_write_overlay_shape_slot(offset, slot) {
            return false;
        }
        let shape_index = offset.overlay_shapes + slot.shape_index;
        let command_index = offset.overlay_commands + slot.command_index;
        self.overlay_shapes[shape_index].corner_radius = corner_radius;
        match &mut self.overlay_commands[command_index] {
            RenderCommand::Shape(primitive) => {
                primitive.corner_radius = corner_radius;
                self.mark_dirty_draw(SceneDrawStream::Overlay, command_index);
                true
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_overlay_shape_stroke_width_slot(
        &mut self,
        offset: &SceneCounts,
        slot: OverlayShapePrimitiveSlot,
        stroke_width: f32,
    ) -> bool {
        if !self.can_write_overlay_shape_slot(offset, slot) {
            return false;
        }
        let shape_index = offset.overlay_shapes + slot.shape_index;
        let command_index = offset.overlay_commands + slot.command_index;
        self.overlay_shapes[shape_index].stroke_width = stroke_width;
        match &mut self.overlay_commands[command_index] {
            RenderCommand::Shape(primitive) => {
                primitive.stroke_width = stroke_width;
                self.mark_dirty_draw(SceneDrawStream::Overlay, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn can_write_text_color_slot(
        &self,
        offset: &SceneCounts,
        slot: TextPrimitiveSlot,
    ) -> bool {
        let Some(text_index) = offset.texts.checked_add(slot.text_index) else {
            return false;
        };
        let Some(command_index) = offset.commands.checked_add(slot.command_index) else {
            return false;
        };
        self.texts.get(text_index).is_some()
            && matches!(
                self.commands.get(command_index),
                Some(RenderCommand::Text(_))
            )
    }

    pub(crate) fn write_text_color_slot(
        &mut self,
        offset: &SceneCounts,
        slot: TextPrimitiveSlot,
        color: Color,
    ) -> bool {
        if !self.can_write_text_color_slot(offset, slot) {
            return false;
        }
        let text_index = offset.texts + slot.text_index;
        let command_index = offset.commands + slot.command_index;
        self.texts[text_index].color = color;
        match &mut self.commands[command_index] {
            RenderCommand::Text(primitive) => {
                primitive.color = color;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn write_text_slot(
        &mut self,
        offset: &SceneCounts,
        slot: TextPrimitiveSlot,
        primitive: TextPrimitive,
    ) -> bool {
        if !self.can_write_text_color_slot(offset, slot) {
            return false;
        }
        let text_index = offset.texts + slot.text_index;
        let command_index = offset.commands + slot.command_index;
        self.texts[text_index] = primitive.clone();
        match &mut self.commands[command_index] {
            RenderCommand::Text(target) => {
                **target = primitive;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn write_text_content_slot(
        &mut self,
        offset: &SceneCounts,
        slot: TextPrimitiveSlot,
        content: Arc<str>,
        font_family: Option<Arc<str>>,
    ) -> bool {
        if !self.can_write_text_color_slot(offset, slot) {
            return false;
        }
        let text_index = offset.texts + slot.text_index;
        let command_index = offset.commands + slot.command_index;
        self.texts[text_index].content = content.clone();
        self.texts[text_index].font_family = font_family.clone();
        match &mut self.commands[command_index] {
            RenderCommand::Text(primitive) => {
                primitive.content = content;
                primitive.font_family = font_family;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn can_write_overlay_text_color_slot(
        &self,
        offset: &SceneCounts,
        slot: OverlayTextPrimitiveSlot,
    ) -> bool {
        let Some(text_index) = offset.overlay_texts.checked_add(slot.text_index) else {
            return false;
        };
        let Some(command_index) = offset.overlay_commands.checked_add(slot.command_index) else {
            return false;
        };
        self.overlay_texts.get(text_index).is_some()
            && matches!(
                self.overlay_commands.get(command_index),
                Some(RenderCommand::Text(_))
            )
    }

    #[allow(dead_code)]
    pub(crate) fn write_overlay_text_color_slot(
        &mut self,
        offset: &SceneCounts,
        slot: OverlayTextPrimitiveSlot,
        color: Color,
    ) -> bool {
        if !self.can_write_overlay_text_color_slot(offset, slot) {
            return false;
        }
        let text_index = offset.overlay_texts + slot.text_index;
        let command_index = offset.overlay_commands + slot.command_index;
        self.overlay_texts[text_index].color = color;
        match &mut self.overlay_commands[command_index] {
            RenderCommand::Text(primitive) => {
                primitive.color = color;
                self.mark_dirty_draw(SceneDrawStream::Overlay, command_index);
                true
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_overlay_text_slot(
        &mut self,
        offset: &SceneCounts,
        slot: OverlayTextPrimitiveSlot,
        primitive: TextPrimitive,
    ) -> bool {
        if !self.can_write_overlay_text_color_slot(offset, slot) {
            return false;
        }
        let text_index = offset.overlay_texts + slot.text_index;
        let command_index = offset.overlay_commands + slot.command_index;
        self.overlay_texts[text_index] = primitive.clone();
        match &mut self.overlay_commands[command_index] {
            RenderCommand::Text(target) => {
                **target = primitive;
                self.mark_dirty_draw(SceneDrawStream::Overlay, command_index);
                true
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_overlay_text_content_slot(
        &mut self,
        offset: &SceneCounts,
        slot: OverlayTextPrimitiveSlot,
        content: Arc<str>,
        font_family: Option<Arc<str>>,
    ) -> bool {
        if !self.can_write_overlay_text_color_slot(offset, slot) {
            return false;
        }
        let text_index = offset.overlay_texts + slot.text_index;
        let command_index = offset.overlay_commands + slot.command_index;
        self.overlay_texts[text_index].content = content.clone();
        self.overlay_texts[text_index].font_family = font_family.clone();
        match &mut self.overlay_commands[command_index] {
            RenderCommand::Text(primitive) => {
                primitive.content = content;
                primitive.font_family = font_family;
                self.mark_dirty_draw(SceneDrawStream::Overlay, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn can_write_text_decoration_slot(
        &self,
        offset: &SceneCounts,
        slot: TextDecorationPrimitiveSlot,
    ) -> bool {
        let Some(text_decoration_index) = offset
            .text_decorations
            .checked_add(slot.text_decoration_index)
        else {
            return false;
        };
        let Some(command_index) = offset.commands.checked_add(slot.command_index) else {
            return false;
        };
        self.text_decorations.get(text_decoration_index).is_some()
            && matches!(
                self.commands.get(command_index),
                Some(RenderCommand::TextDecoration(_))
            )
    }

    pub(crate) fn write_text_decoration_slot(
        &mut self,
        offset: &SceneCounts,
        slot: TextDecorationPrimitiveSlot,
        primitive: TextDecorationPrimitive,
    ) -> bool {
        if !self.can_write_text_decoration_slot(offset, slot) {
            return false;
        }
        let text_decoration_index = offset.text_decorations + slot.text_decoration_index;
        let command_index = offset.commands + slot.command_index;
        self.text_decorations[text_decoration_index] = primitive.clone();
        match &mut self.commands[command_index] {
            RenderCommand::TextDecoration(target) => {
                *target = primitive;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn can_write_overlay_text_decoration_slot(
        &self,
        offset: &SceneCounts,
        slot: OverlayTextDecorationPrimitiveSlot,
    ) -> bool {
        let Some(text_decoration_index) = offset
            .overlay_text_decorations
            .checked_add(slot.text_decoration_index)
        else {
            return false;
        };
        let Some(command_index) = offset.overlay_commands.checked_add(slot.command_index) else {
            return false;
        };
        self.overlay_text_decorations
            .get(text_decoration_index)
            .is_some()
            && matches!(
                self.overlay_commands.get(command_index),
                Some(RenderCommand::TextDecoration(_))
            )
    }

    pub(crate) fn write_overlay_text_decoration_slot(
        &mut self,
        offset: &SceneCounts,
        slot: OverlayTextDecorationPrimitiveSlot,
        primitive: TextDecorationPrimitive,
    ) -> bool {
        if !self.can_write_overlay_text_decoration_slot(offset, slot) {
            return false;
        }
        let text_decoration_index = offset.overlay_text_decorations + slot.text_decoration_index;
        let command_index = offset.overlay_commands + slot.command_index;
        self.overlay_text_decorations[text_decoration_index] = primitive.clone();
        match &mut self.overlay_commands[command_index] {
            RenderCommand::TextDecoration(target) => {
                *target = primitive;
                self.mark_dirty_draw(SceneDrawStream::Overlay, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn can_write_texture_opacity_slot(
        &self,
        offset: &SceneCounts,
        slot: TexturePrimitiveSlot,
    ) -> bool {
        let Some(texture_index) = offset.textures.checked_add(slot.texture_index) else {
            return false;
        };
        let Some(command_index) = offset.commands.checked_add(slot.command_index) else {
            return false;
        };
        self.textures.get(texture_index).is_some()
            && matches!(
                self.commands.get(command_index),
                Some(RenderCommand::Texture(_))
            )
    }

    pub(crate) fn texture_slot(
        &self,
        offset: &SceneCounts,
        slot: TexturePrimitiveSlot,
    ) -> Option<&TexturePrimitive> {
        let texture_index = offset.textures.checked_add(slot.texture_index)?;
        let command_index = offset.commands.checked_add(slot.command_index)?;
        let texture = self.textures.get(texture_index)?;
        match self.commands.get(command_index) {
            Some(RenderCommand::Texture(_)) => Some(texture),
            _ => None,
        }
    }

    pub(crate) fn write_texture_opacity_slot(
        &mut self,
        offset: &SceneCounts,
        slot: TexturePrimitiveSlot,
        opacity: f32,
    ) -> bool {
        if !self.can_write_texture_opacity_slot(offset, slot) {
            return false;
        }
        let texture_index = offset.textures + slot.texture_index;
        let command_index = offset.commands + slot.command_index;
        self.textures[texture_index].opacity = opacity;
        match &mut self.commands[command_index] {
            RenderCommand::Texture(primitive) => {
                primitive.opacity = opacity;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn write_texture_slot(
        &mut self,
        offset: &SceneCounts,
        slot: TexturePrimitiveSlot,
        primitive: TexturePrimitive,
    ) -> bool {
        if !self.can_write_texture_opacity_slot(offset, slot) {
            return false;
        }
        let texture_index = offset.textures + slot.texture_index;
        let command_index = offset.commands + slot.command_index;
        self.textures[texture_index] = primitive.clone();
        match &mut self.commands[command_index] {
            RenderCommand::Texture(target) => {
                *target = primitive;
                self.mark_dirty_draw(SceneDrawStream::Main, command_index);
                true
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn can_write_overlay_texture_opacity_slot(
        &self,
        offset: &SceneCounts,
        slot: OverlayTexturePrimitiveSlot,
    ) -> bool {
        let Some(texture_index) = offset.overlay_textures.checked_add(slot.texture_index) else {
            return false;
        };
        let Some(command_index) = offset.overlay_commands.checked_add(slot.command_index) else {
            return false;
        };
        self.overlay_textures.get(texture_index).is_some()
            && matches!(
                self.overlay_commands.get(command_index),
                Some(RenderCommand::Texture(_))
            )
    }

    #[allow(dead_code)]
    pub(crate) fn overlay_texture_slot(
        &self,
        offset: &SceneCounts,
        slot: OverlayTexturePrimitiveSlot,
    ) -> Option<&TexturePrimitive> {
        let texture_index = offset.overlay_textures.checked_add(slot.texture_index)?;
        let command_index = offset.overlay_commands.checked_add(slot.command_index)?;
        let texture = self.overlay_textures.get(texture_index)?;
        match self.overlay_commands.get(command_index) {
            Some(RenderCommand::Texture(_)) => Some(texture),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_overlay_texture_opacity_slot(
        &mut self,
        offset: &SceneCounts,
        slot: OverlayTexturePrimitiveSlot,
        opacity: f32,
    ) -> bool {
        if !self.can_write_overlay_texture_opacity_slot(offset, slot) {
            return false;
        }
        let texture_index = offset.overlay_textures + slot.texture_index;
        let command_index = offset.overlay_commands + slot.command_index;
        self.overlay_textures[texture_index].opacity = opacity;
        match &mut self.overlay_commands[command_index] {
            RenderCommand::Texture(primitive) => {
                primitive.opacity = opacity;
                self.mark_dirty_draw(SceneDrawStream::Overlay, command_index);
                true
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_overlay_texture_slot(
        &mut self,
        offset: &SceneCounts,
        slot: OverlayTexturePrimitiveSlot,
        primitive: TexturePrimitive,
    ) -> bool {
        if !self.can_write_overlay_texture_opacity_slot(offset, slot) {
            return false;
        }
        let texture_index = offset.overlay_textures + slot.texture_index;
        let command_index = offset.overlay_commands + slot.command_index;
        self.overlay_textures[texture_index] = primitive.clone();
        match &mut self.overlay_commands[command_index] {
            RenderCommand::Texture(target) => {
                *target = primitive;
                self.mark_dirty_draw(SceneDrawStream::Overlay, command_index);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn extend(&mut self, other: &ScenePrimitives) {
        let command_offset = self.commands.len();
        let overlay_command_offset = self.overlay_commands.len();
        self.backdrop_blurs
            .extend(other.backdrop_blurs.iter().copied());
        self.brushes.extend(other.brushes.iter().cloned());
        self.canvas_composites
            .extend(other.canvas_composites.iter().cloned());
        self.shapes.extend(other.shapes.iter().copied());
        self.meshes.extend(other.meshes.iter().cloned());
        self.textures.extend(other.textures.iter().cloned());
        #[cfg(feature = "video")]
        self.video_textures
            .extend(other.video_textures.iter().cloned());
        self.texts.extend(other.texts.iter().cloned());
        self.text_decorations
            .extend(other.text_decorations.iter().cloned());
        self.overlay_shapes
            .extend(other.overlay_shapes.iter().copied());
        self.overlay_textures
            .extend(other.overlay_textures.iter().cloned());
        self.overlay_meshes
            .extend(other.overlay_meshes.iter().cloned());
        self.overlay_texts
            .extend(other.overlay_texts.iter().cloned());
        self.overlay_text_decorations
            .extend(other.overlay_text_decorations.iter().cloned());
        self.commands.extend(other.commands.iter().cloned());
        self.overlay_commands
            .extend(other.overlay_commands.iter().cloned());
        self.overlay_command_sources
            .extend(other.overlay_command_sources.iter().copied());
        {
            self.command_gpu_scroll_containers
                .extend(other.command_gpu_scroll_containers.iter().copied());
            self.overlay_command_gpu_scroll_containers
                .extend(other.overlay_command_gpu_scroll_containers.iter().copied());
            self.command_transform_chains
                .extend(other.command_transform_chains.iter().cloned());
            self.overlay_command_transform_chains
                .extend(other.overlay_command_transform_chains.iter().cloned());
        }
        self.dirty_draw_ranges
            .extend(other.dirty_draw_ranges.iter().cloned().map(|mut range| {
                let offset = match range.stream {
                    SceneDrawStream::Main => command_offset,
                    SceneDrawStream::Overlay => overlay_command_offset,
                };
                range.range = (range.range.start + offset)..(range.range.end + offset);
                range
            }));
    }

    /// 各渲染流当前的命令数量快照。Splice 快路径用它在
    /// 「祖先链向上 `extend` 拼接」的纯连接模型下，给每个子树在根扁平场景里
    /// 定位出稳定的命令区间起点。
    pub(crate) fn counts(&self) -> SceneCounts {
        SceneCounts {
            backdrop_blurs: self.backdrop_blurs.len(),
            brushes: self.brushes.len(),
            canvas_composites: self.canvas_composites.len(),
            shapes: self.shapes.len(),
            meshes: self.meshes.len(),
            textures: self.textures.len(),
            #[cfg(feature = "video")]
            video_textures: self.video_textures.len(),
            texts: self.texts.len(),
            text_decorations: self.text_decorations.len(),
            commands: self.commands.len(),
            overlay_shapes: self.overlay_shapes.len(),
            overlay_textures: self.overlay_textures.len(),
            overlay_meshes: self.overlay_meshes.len(),
            overlay_texts: self.overlay_texts.len(),
            overlay_text_decorations: self.overlay_text_decorations.len(),
            overlay_commands: self.overlay_commands.len(),
        }
    }

    /// 把 `chunk` 的各主渲染流原地覆盖到 `self` 从 `offset` 起、长度为 `chunk` 流长度的
    /// 区间上。调用方必须保证 `offset + chunk.len == old_chunk.len`（数量一致，纯属性变化），
    /// 否则区间越界返回 `false`、`self` 不被修改，调用方回退到 recompose。
    ///
    /// 仅覆盖主渲染流（overlay_* 由 finalize 阶段独立维护，splice 快路径只在子树
    /// 不含 overlay 内容时启用，见 `ComputedScene::is_simple_for_splice`）。
    pub(crate) fn splice_in_place(
        &mut self,
        offset: &SceneCounts,
        chunk: &ScenePrimitives,
    ) -> bool {
        fn overwrite<T: Clone>(dst: &mut [T], start: usize, src: &[T]) -> bool {
            let Some(end) = start.checked_add(src.len()) else {
                return false;
            };
            if end > dst.len() {
                return false;
            }
            dst[start..end].clone_from_slice(src);
            true
        }
        let ok = overwrite(
            &mut self.backdrop_blurs,
            offset.backdrop_blurs,
            &chunk.backdrop_blurs,
        ) && overwrite(&mut self.brushes, offset.brushes, &chunk.brushes)
            && overwrite(
                &mut self.canvas_composites,
                offset.canvas_composites,
                &chunk.canvas_composites,
            )
            && overwrite(&mut self.shapes, offset.shapes, &chunk.shapes)
            && overwrite(&mut self.meshes, offset.meshes, &chunk.meshes)
            && overwrite(&mut self.textures, offset.textures, &chunk.textures)
            && {
                #[cfg(feature = "video")]
                {
                    overwrite(
                        &mut self.video_textures,
                        offset.video_textures,
                        &chunk.video_textures,
                    )
                }
                #[cfg(not(feature = "video"))]
                {
                    true
                }
            }
            && overwrite(&mut self.texts, offset.texts, &chunk.texts)
            && overwrite(
                &mut self.text_decorations,
                offset.text_decorations,
                &chunk.text_decorations,
            )
            && overwrite(&mut self.commands, offset.commands, &chunk.commands)
            && overwrite(
                &mut self.command_gpu_scroll_containers,
                offset.commands,
                &chunk.command_gpu_scroll_containers,
            )
            && overwrite(
                &mut self.command_transform_chains,
                offset.commands,
                &chunk.command_transform_chains,
            );
        if ok {
            if !chunk.commands.is_empty() {
                self.dirty_draw_ranges.push(DirtyDrawRange {
                    stream: SceneDrawStream::Main,
                    range: offset.commands..(offset.commands + chunk.commands.len()),
                });
            }
            self.dirty_draw_ranges
                .extend(chunk.dirty_draw_ranges.iter().cloned().map(|mut range| {
                    let offset = match range.stream {
                        SceneDrawStream::Main => offset.commands,
                        SceneDrawStream::Overlay => offset.overlay_commands,
                    };
                    range.range = (range.range.start + offset)..(range.range.end + offset);
                    range
                }));
        }
        ok
    }
}

/// 每个渲染流的命令数量。既用作 splice 的区间起点偏移（沿 root→target 路径累加），
/// 也用作 splice 资格判定的「数量一致性」对比。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SceneCounts {
    pub backdrop_blurs: usize,
    pub brushes: usize,
    pub canvas_composites: usize,
    pub shapes: usize,
    pub meshes: usize,
    pub textures: usize,
    #[cfg(feature = "video")]
    pub video_textures: usize,
    pub texts: usize,
    pub text_decorations: usize,
    pub commands: usize,
    pub overlay_shapes: usize,
    pub overlay_textures: usize,
    pub overlay_meshes: usize,
    pub overlay_texts: usize,
    pub overlay_text_decorations: usize,
    pub overlay_commands: usize,
}

impl SceneCounts {
    /// 累加另一段的各流数量（offset 沿子树路径向前推进时用）。
    pub(crate) fn add_assign(&mut self, other: &SceneCounts) {
        self.backdrop_blurs += other.backdrop_blurs;
        self.brushes += other.brushes;
        self.canvas_composites += other.canvas_composites;
        self.shapes += other.shapes;
        self.meshes += other.meshes;
        self.textures += other.textures;
        #[cfg(feature = "video")]
        {
            self.video_textures += other.video_textures;
        }
        self.texts += other.texts;
        self.text_decorations += other.text_decorations;
        self.commands += other.commands;
        self.overlay_shapes += other.overlay_shapes;
        self.overlay_textures += other.overlay_textures;
        self.overlay_meshes += other.overlay_meshes;
        self.overlay_texts += other.overlay_texts;
        self.overlay_text_decorations += other.overlay_text_decorations;
        self.overlay_commands += other.overlay_commands;
    }

    /// 该段是否完全没有 overlay 流内容。splice 快路径要求子树不产生 overlay/portal，
    /// 否则 `finalize_portals` / `finalize_overlay_layers` 会改变流偏移，原地覆盖不成立。
    pub(crate) fn has_no_overlay(&self) -> bool {
        self.overlay_shapes == 0
            && self.overlay_textures == 0
            && self.overlay_meshes == 0
            && self.overlay_texts == 0
            && self.overlay_text_decorations == 0
            && self.overlay_commands == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BrushPrimitiveData {
    pub brush_meta: [f32; 4],
    pub gradient_data0: [f32; 4],
    pub gradient_data1: [f32; 4],
    pub stop_offsets0: [f32; 4],
    pub stop_offsets1: [f32; 4],
    pub stop_colors: [[f32; 4]; 7],
}

impl BrushPrimitiveData {
    pub(crate) fn from_background_brush(brush: &BackgroundBrush, opacity: f32) -> Option<Self> {
        match brush {
            BackgroundBrush::Solid(color) => Some(Self {
                brush_meta: [0.0, 2.0, 0.0, 0.0],
                gradient_data0: [0.0; 4],
                gradient_data1: [0.0; 4],
                stop_offsets0: [0.0, 1.0, 0.0, 0.0],
                stop_offsets1: [0.0; 4],
                stop_colors: solid_stop_colors(color.with_alpha_factor(opacity)),
            }),
            BackgroundBrush::LinearGradient(gradient) => {
                let stops = normalized_background_stops(&gradient.stops, opacity)?;
                Some(Self::gradient(
                    1.0,
                    stops.len() as f32,
                    [
                        gradient.start.x.get(),
                        gradient.start.y.get(),
                        gradient.end.x.get(),
                        gradient.end.y.get(),
                    ],
                    [0.0; 4],
                    stops,
                ))
            }
            BackgroundBrush::RadialGradient(gradient) => {
                let stops = normalized_background_stops(&gradient.stops, opacity)?;
                Some(Self::gradient(
                    2.0,
                    stops.len() as f32,
                    [0.0; 4],
                    [
                        gradient.center.x.get(),
                        gradient.center.y.get(),
                        gradient.radius.get().max(0.0001),
                        0.0,
                    ],
                    stops,
                ))
            }
        }
    }

    fn gradient(
        kind: f32,
        stop_count: f32,
        gradient_data0: [f32; 4],
        gradient_data1: [f32; 4],
        stops: Vec<BackgroundGradientStopData>,
    ) -> Self {
        let mut stop_offsets0 = [0.0; 4];
        let mut stop_offsets1 = [0.0; 4];
        let mut stop_colors = [[0.0; 4]; 7];

        for (index, stop) in stops.iter().enumerate() {
            if index < 4 {
                stop_offsets0[index] = stop.offset;
            } else {
                stop_offsets1[index - 4] = stop.offset;
            }
            stop_colors[index] = stop.color;
        }

        Self {
            brush_meta: [kind, stop_count, 0.0, 0.0],
            gradient_data0,
            gradient_data1,
            stop_offsets0,
            stop_offsets1,
            stop_colors,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BackgroundGradientStopData {
    offset: f32,
    color: [f32; 4],
}

fn normalized_background_stops(
    stops: &[BackgroundGradientStop],
    opacity: f32,
) -> Option<Vec<BackgroundGradientStopData>> {
    if stops.is_empty() || stops.len() > 7 {
        return None;
    }

    Some(
        stops
            .iter()
            .map(|stop| {
                let color = stop.color.with_alpha_factor(opacity);
                BackgroundGradientStopData {
                    offset: stop.offset,
                    color: color.to_linear_rgba_f32(),
                }
            })
            .collect(),
    )
}

fn solid_stop_colors(color: Color) -> [[f32; 4]; 7] {
    let rgba = color.to_linear_rgba_f32();
    let mut colors = [[0.0; 4]; 7];
    colors[0] = rgba;
    colors[1] = rgba;
    colors
}

#[cfg(test)]
mod culling_tests {
    use super::*;

    fn shape(rect: Rect, clip_rect: Option<Rect>) -> RenderPrimitive {
        RenderPrimitive {
            rect,
            color: Color::WHITE,
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect,
            clip_mask: None,
        }
    }

    fn text(content: &str, color: Color) -> TextPrimitive {
        TextPrimitive {
            content: Arc::from(content),
            rich_spans: None,
            frame: Rect::new(0.0, 0.0, 48.0, 20.0),
            quad: None,
            color,
            force_color: false,
            font_family: None,
            font_size: 14.0,
            font_weight: FontWeight::NORMAL,
            line_height: 18.0,
            letter_spacing: 0.0,
            wrap: CanvasTextWrap::Word,
            overflow: CanvasTextOverflow::Clip,
            horizontal_align: CanvasTextHorizontalAlign::Start,
            vertical_align: CanvasTextVerticalAlign::Start,
            clip_rect: None,
            clip_mask: None,
        }
    }

    fn texture(opacity: f32) -> TexturePrimitive {
        TexturePrimitive {
            texture: Arc::new(crate::media::TextureFrame::new(
                1,
                1,
                vec![255, 255, 255, 255],
            )),
            media_key: None,
            media_layout: None,
            frame: Rect::new(0.0, 0.0, 12.0, 12.0),
            quad: None,
            uv_rect: None,
            corner_radius: 0.0,
            opacity,
            clip_rect: None,
            clip_mask: None,
        }
    }

    #[test]
    fn shape_inside_its_clip_is_kept() {
        let mut scene = ScenePrimitives::default();
        let clip = Rect::new(0.0, 0.0, 100.0, 100.0);
        scene.push_shape(shape(Rect::new(10.0, 10.0, 20.0, 20.0), Some(clip)));
        assert_eq!(scene.shapes.len(), 1);
        assert_eq!(scene.commands.len(), 1);
    }

    #[test]
    fn shape_fully_outside_its_clip_is_dropped() {
        let mut scene = ScenePrimitives::default();
        let clip = Rect::new(0.0, 0.0, 100.0, 100.0);
        // Far below the clip region — a scrolled-away row.
        scene.push_shape(shape(Rect::new(10.0, 500.0, 20.0, 20.0), Some(clip)));
        assert!(scene.shapes.is_empty());
        // The parallel command stream must stay in sync, or the renderer would
        // draw a stale command with no backing primitive.
        assert!(scene.commands.is_empty());
    }

    #[test]
    fn shape_without_clip_is_never_culled() {
        let mut scene = ScenePrimitives::default();
        scene.push_shape(shape(Rect::new(10.0, 5000.0, 20.0, 20.0), None));
        assert_eq!(scene.shapes.len(), 1);
        assert_eq!(scene.commands.len(), 1);
    }

    #[test]
    fn gpu_scroll_active_keeps_clipped_out_shape_and_tags_command() {
        let mut scene = ScenePrimitives::default();
        let scroll_id = WidgetId::from_raw(42);
        let clip = Rect::new(0.0, 0.0, 100.0, 100.0);

        scene.set_active_gpu_scroll_container(Some(scroll_id));
        scene.push_shape(shape(Rect::new(10.0, 500.0, 20.0, 20.0), Some(clip)));

        assert_eq!(scene.shapes.len(), 1);
        assert_eq!(scene.commands.len(), 1);
        assert_eq!(scene.command_gpu_scroll_containers(), &[Some(scroll_id)]);
    }

    #[test]
    fn overlay_shape_slot_writes_parallel_array_and_command_stream() {
        let mut scene = ScenePrimitives::default();
        scene.push_overlay_shape(shape(Rect::new(1.0, 2.0, 3.0, 4.0), None));
        let slot = scene
            .matching_overlay_shape_slots(|shape| shape.color == Color::WHITE)
            .pop()
            .expect("overlay shape slot");

        assert!(scene.write_overlay_shape_color_slot(&SceneCounts::default(), slot, Color::RED));
        assert!(scene.write_overlay_shape_rect_slot(
            &SceneCounts::default(),
            slot,
            Rect::new(5.0, 6.0, 7.0, 8.0)
        ));
        assert!(scene.write_overlay_shape_corner_radius_slot(&SceneCounts::default(), slot, 3.0));
        assert!(scene.write_overlay_shape_stroke_width_slot(&SceneCounts::default(), slot, 2.0));

        let shape = scene.overlay_shapes[0];
        assert_eq!(shape.color, Color::RED);
        assert_eq!(shape.rect, Rect::new(5.0, 6.0, 7.0, 8.0));
        assert_eq!(shape.corner_radius, 3.0);
        assert_eq!(shape.stroke_width, 2.0);
        match &scene.overlay_commands[0] {
            RenderCommand::Shape(command) => {
                assert_eq!(command.color, shape.color);
                assert_eq!(command.rect, shape.rect);
                assert_eq!(command.corner_radius, shape.corner_radius);
                assert_eq!(command.stroke_width, shape.stroke_width);
            }
            _ => panic!("expected overlay shape command"),
        }
    }

    #[test]
    fn overlay_text_slot_writes_parallel_array_and_command_stream() {
        let mut scene = ScenePrimitives::default();
        scene.push_overlay_text(text("before", Color::WHITE));
        let slot = scene
            .matching_overlay_text_slots(|text| text.content.as_ref() == "before")
            .pop()
            .expect("overlay text slot");

        assert!(scene.write_overlay_text_color_slot(&SceneCounts::default(), slot, Color::BLUE));
        assert!(scene.write_overlay_text_content_slot(
            &SceneCounts::default(),
            slot,
            Arc::from("after"),
            Some(Arc::from("Inter")),
        ));

        let text = scene.overlay_texts[0].clone();
        assert_eq!(text.color, Color::BLUE);
        assert_eq!(text.content.as_ref(), "after");
        assert_eq!(text.font_family.as_deref(), Some("Inter"));
        match &scene.overlay_commands[0] {
            RenderCommand::Text(command) => assert_eq!(**command, text),
            _ => panic!("expected overlay text command"),
        }
    }

    #[test]
    fn overlay_texture_slot_writes_parallel_array_and_command_stream() {
        let mut scene = ScenePrimitives::default();
        scene.push_overlay_texture(texture(0.25));
        let slot = scene
            .matching_overlay_texture_slots(|texture| (texture.opacity - 0.25).abs() < f32::EPSILON)
            .pop()
            .expect("overlay texture slot");

        assert!(scene.write_overlay_texture_opacity_slot(&SceneCounts::default(), slot, 0.75));
        assert_eq!(scene.overlay_textures[0].opacity, 0.75);
        assert_eq!(
            scene
                .overlay_texture_slot(&SceneCounts::default(), slot)
                .map(|texture| texture.opacity),
            Some(0.75)
        );

        let replacement = texture(1.0);
        let replacement_id = replacement.texture.id();
        assert!(scene.write_overlay_texture_slot(&SceneCounts::default(), slot, replacement));
        assert_eq!(scene.overlay_textures[0].texture.id(), replacement_id);
        match &scene.overlay_commands[0] {
            RenderCommand::Texture(command) => {
                assert_eq!(command.texture.id(), replacement_id);
                assert_eq!(command.opacity, 1.0);
            }
            _ => panic!("expected overlay texture command"),
        }
    }
}
