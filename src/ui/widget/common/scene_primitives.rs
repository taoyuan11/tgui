use super::*;

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
    pub content: String,
    pub font_family: Option<String>,
    pub color: Color,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
}

#[derive(Clone)]
pub struct TextPrimitive {
    pub content: String,
    pub rich_spans: Option<Arc<[CanvasTextSpanPrimitive]>>,
    pub frame: Rect,
    pub quad: Option<[Point; 4]>,
    pub color: Color,
    pub force_color: bool,
    pub font_family: Option<String>,
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
    CanvasComposite(CanvasCompositePrimitive),
    Shape(RenderPrimitive),
    Texture(TexturePrimitive),
    Text(TextPrimitive),
    Mesh(MeshPrimitive),
}

#[derive(Clone, Default)]
pub struct ScenePrimitives {
    pub backdrop_blurs: Vec<BackdropBlurPrimitive>,
    pub brushes: Vec<BrushPrimitive>,
    pub canvas_composites: Vec<CanvasCompositePrimitive>,
    pub shapes: Vec<RenderPrimitive>,
    pub meshes: Vec<MeshPrimitive>,
    pub textures: Vec<TexturePrimitive>,
    pub texts: Vec<TextPrimitive>,
    pub overlay_shapes: Vec<RenderPrimitive>,
    pub overlay_textures: Vec<TexturePrimitive>,
    #[allow(dead_code)]
    pub overlay_meshes: Vec<MeshPrimitive>,
    #[allow(dead_code)]
    pub overlay_texts: Vec<TextPrimitive>,
    pub(crate) commands: Vec<RenderCommand>,
    pub(crate) overlay_commands: Vec<RenderCommand>,
}

impl ScenePrimitives {
    pub(crate) fn push_render_command(&mut self, command: RenderCommand) {
        match command {
            RenderCommand::BackdropBlur(primitive) => self.push_backdrop_blur(primitive),
            RenderCommand::Brush(primitive) => self.push_brush(primitive),
            RenderCommand::CanvasComposite(primitive) => self.push_canvas_composite(primitive),
            RenderCommand::Shape(primitive) => self.push_shape(primitive),
            RenderCommand::Texture(primitive) => self.push_texture(primitive),
            RenderCommand::Text(primitive) => self.push_text(primitive),
            RenderCommand::Mesh(primitive) => self.push_mesh(primitive),
        }
    }

    pub(crate) fn push_backdrop_blur(&mut self, primitive: BackdropBlurPrimitive) {
        self.backdrop_blurs.push(primitive);
        self.commands.push(RenderCommand::BackdropBlur(primitive));
    }

    pub(crate) fn push_brush(&mut self, primitive: BrushPrimitive) {
        self.brushes.push(primitive.clone());
        self.commands.push(RenderCommand::Brush(primitive));
    }

    pub(crate) fn push_canvas_composite(&mut self, primitive: CanvasCompositePrimitive) {
        self.canvas_composites.push(primitive.clone());
        self.commands
            .push(RenderCommand::CanvasComposite(primitive));
    }

    pub(crate) fn push_shape(&mut self, primitive: RenderPrimitive) {
        self.shapes.push(primitive);
        self.commands.push(RenderCommand::Shape(primitive));
    }

    pub(crate) fn push_mesh(&mut self, primitive: MeshPrimitive) {
        self.meshes.push(primitive.clone());
        self.commands.push(RenderCommand::Mesh(primitive));
    }

    pub(crate) fn push_texture(&mut self, primitive: TexturePrimitive) {
        self.textures.push(primitive.clone());
        self.commands.push(RenderCommand::Texture(primitive));
    }

    pub(crate) fn push_text(&mut self, primitive: TextPrimitive) {
        self.texts.push(primitive.clone());
        self.commands.push(RenderCommand::Text(primitive));
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
        self.overlay_commands.push(RenderCommand::Text(primitive));
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
