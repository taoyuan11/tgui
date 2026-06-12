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
    pub(crate) fn delta_since(&self, base: &ScenePrimitives) -> ScenePrimitives {
        let mut delta = ScenePrimitives::default();
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
        delta
            .commands
            .extend(self.commands.iter().skip(base.commands.len()).cloned());
        delta.overlay_commands.extend(
            self.overlay_commands
                .iter()
                .skip(base.overlay_commands.len())
                .cloned(),
        );
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

    /// 各渲染流当前的命令数量快照。Phase 1 splice 快路径用它在
    /// 「祖先链向上 `extend` 拼接」的纯连接模型下，给每个子树在根扁平场景里
    /// 定位出稳定的命令区间起点。
    #[cfg(feature = "fine-grained-splice")]
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
            commands: self.commands.len(),
            overlay_shapes: self.overlay_shapes.len(),
            overlay_textures: self.overlay_textures.len(),
            overlay_meshes: self.overlay_meshes.len(),
            overlay_texts: self.overlay_texts.len(),
            overlay_commands: self.overlay_commands.len(),
        }
    }

    /// 把 `chunk` 的各主渲染流原地覆盖到 `self` 从 `offset` 起、长度为 `chunk` 流长度的
    /// 区间上。调用方必须保证 `offset + chunk.len == old_chunk.len`（数量一致，纯属性变化），
    /// 否则区间越界返回 `false`、`self` 不被修改，调用方回退到 recompose。
    ///
    /// 仅覆盖主渲染流（overlay_* 由 finalize 阶段独立维护，splice 快路径只在子树
    /// 不含 overlay 内容时启用，见 `ComputedScene::is_simple_for_splice`）。
    #[cfg(feature = "fine-grained-splice")]
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
        overwrite(
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
            && overwrite(&mut self.commands, offset.commands, &chunk.commands)
    }
}

/// 每个渲染流的命令数量。既用作 splice 的区间起点偏移（沿 root→target 路径累加），
/// 也用作 splice 资格判定的「数量一致性」对比。
#[cfg(feature = "fine-grained-splice")]
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
    pub commands: usize,
    pub overlay_shapes: usize,
    pub overlay_textures: usize,
    pub overlay_meshes: usize,
    pub overlay_texts: usize,
    pub overlay_commands: usize,
}

#[cfg(feature = "fine-grained-splice")]
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
        self.commands += other.commands;
        self.overlay_shapes += other.overlay_shapes;
        self.overlay_textures += other.overlay_textures;
        self.overlay_meshes += other.overlay_meshes;
        self.overlay_texts += other.overlay_texts;
        self.overlay_commands += other.overlay_commands;
    }

    /// 该段是否完全没有 overlay 流内容。splice 快路径要求子树不产生 overlay/portal，
    /// 否则 `finalize_portals` / `finalize_overlay_layers` 会改变流偏移，原地覆盖不成立。
    pub(crate) fn has_no_overlay(&self) -> bool {
        self.overlay_shapes == 0
            && self.overlay_textures == 0
            && self.overlay_meshes == 0
            && self.overlay_texts == 0
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
