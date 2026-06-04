use super::*;
#[cfg(feature = "video")]
use crate::video::VideoController;
use smallvec::SmallVec;
use std::sync::Arc;

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

#[derive(Clone)]
pub struct CanvasTextSpanPrimitive {
    pub content: Arc<str>,
    pub font_family: Option<Arc<str>>,
    pub color: Color,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
}

#[derive(Clone)]
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

#[derive(Clone)]
pub struct TexturePrimitive {
    pub texture: Arc<TextureFrame>,
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
    pub overlay_shapes: SmallVec<[RenderPrimitive; 1]>,
    pub overlay_textures: SmallVec<[TexturePrimitive; 1]>,
    #[allow(dead_code)]
    pub overlay_meshes: SmallVec<[MeshPrimitive; 1]>,
    #[allow(dead_code)]
    pub overlay_texts: SmallVec<[TextPrimitive; 1]>,
    pub(crate) commands: SmallVec<[RenderCommand; 1]>,
    pub(crate) overlay_commands: SmallVec<[RenderCommand; 1]>,
}

impl ScenePrimitives {
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
            RenderCommand::Mesh(primitive) => self.push_mesh(primitive),
        }
    }

    pub(crate) fn push_backdrop_blur(&mut self, primitive: BackdropBlurPrimitive) {
        if clipped_out(primitive.rect, primitive.clip_rect) {
            return;
        }
        self.backdrop_blurs.push(primitive);
        self.commands.push(RenderCommand::BackdropBlur(primitive));
    }

    pub(crate) fn push_brush(&mut self, primitive: BrushPrimitive) {
        if clipped_out(primitive.rect, primitive.clip_rect) {
            return;
        }
        self.brushes.push(primitive.clone());
        self.commands.push(RenderCommand::Brush(primitive));
    }

    pub(crate) fn push_canvas_composite(&mut self, primitive: CanvasCompositePrimitive) {
        if clipped_out(primitive.bounds, primitive.clip_rect) {
            return;
        }
        self.canvas_composites.push(primitive.clone());
        self.commands
            .push(RenderCommand::CanvasComposite(Box::new(primitive)));
    }

    pub(crate) fn push_shape(&mut self, primitive: RenderPrimitive) {
        if clipped_out(primitive.rect, primitive.clip_rect) {
            return;
        }
        self.shapes.push(primitive);
        self.commands.push(RenderCommand::Shape(primitive));
    }

    pub(crate) fn push_mesh(&mut self, primitive: MeshPrimitive) {
        self.meshes.push(primitive.clone());
        self.commands.push(RenderCommand::Mesh(primitive));
    }

    pub(crate) fn push_texture(&mut self, primitive: TexturePrimitive) {
        if clipped_out(primitive.frame, primitive.clip_rect) {
            return;
        }
        self.textures.push(primitive.clone());
        self.commands.push(RenderCommand::Texture(primitive));
    }

    #[cfg(feature = "video")]
    pub(crate) fn push_video_texture(&mut self, primitive: VideoTexturePrimitive) {
        if clipped_out(primitive.frame, primitive.clip_rect) {
            return;
        }
        self.video_textures.push(primitive.clone());
        self.commands.push(RenderCommand::VideoTexture(primitive));
    }

    pub(crate) fn push_text(&mut self, primitive: TextPrimitive) {
        if clipped_out(primitive.frame, primitive.clip_rect) {
            return;
        }
        self.texts.push(primitive.clone());
        self.commands.push(RenderCommand::Text(Box::new(primitive)));
    }

    pub(crate) fn push_overlay_shape(&mut self, primitive: RenderPrimitive) {
        self.overlay_shapes.push(primitive);
        self.overlay_commands.push(RenderCommand::Shape(primitive));
    }

    #[allow(dead_code)]
    pub(crate) fn push_overlay_texture(&mut self, primitive: TexturePrimitive) {
        self.overlay_textures.push(primitive.clone());
        self.overlay_commands
            .push(RenderCommand::Texture(primitive));
    }

    #[allow(dead_code)]
    pub(crate) fn push_overlay_mesh(&mut self, primitive: MeshPrimitive) {
        self.overlay_meshes.push(primitive.clone());
        self.overlay_commands.push(RenderCommand::Mesh(primitive));
    }

    #[allow(dead_code)]
    pub(crate) fn push_overlay_text(&mut self, primitive: TextPrimitive) {
        self.overlay_texts.push(primitive.clone());
        self.overlay_commands
            .push(RenderCommand::Text(Box::new(primitive)));
    }

    pub(crate) fn extend(&mut self, other: &ScenePrimitives) {
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
        self.overlay_shapes
            .extend(other.overlay_shapes.iter().copied());
        self.overlay_textures
            .extend(other.overlay_textures.iter().cloned());
        self.overlay_meshes
            .extend(other.overlay_meshes.iter().cloned());
        self.overlay_texts
            .extend(other.overlay_texts.iter().cloned());
        self.commands.extend(other.commands.iter().cloned());
        self.overlay_commands
            .extend(other.overlay_commands.iter().cloned());
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
}
