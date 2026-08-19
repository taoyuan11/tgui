use super::cache::RenderCache;
use super::paint::{BlendMode, Brush, Canvas, Paint, PaintCommand, Path, PathSegment, TextRun};
use super::scene::RenderTree;
use crate::core::{
    Color, DpiScale, Error, Point, Rect, ResourceId, Result, SceneRevision, Transform2D,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RendererCapabilities {
    pub generation: u64,
    pub supports_paths: bool,
    pub supports_native_surface: bool,
    pub supports_backdrop: bool,
    pub max_texture_dimension_2d: u32,
}

impl Default for RendererCapabilities {
    fn default() -> Self {
        Self {
            generation: 1,
            supports_paths: true,
            supports_native_surface: false,
            supports_backdrop: false,
            max_texture_dimension_2d: 16_384,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompileContext {
    pub capabilities: RendererCapabilities,
    pub dpi_scale: DpiScale,
    pub theme_revision: u64,
    pub font_revision: u64,
    pub image_revision: u64,
    pub glyph_revision: u64,
    pub resource_revision: u64,
    pub scene_revision: SceneRevision,
    pub transient_budget_bytes: u64,
}

impl CompileContext {
    pub fn new(capabilities: RendererCapabilities, dpi_scale: DpiScale) -> Self {
        Self {
            capabilities,
            dpi_scale,
            theme_revision: 0,
            font_revision: 0,
            image_revision: 0,
            glyph_revision: 0,
            resource_revision: 0,
            scene_revision: SceneRevision::ZERO,
            transient_budget_bytes: 128 * 1024 * 1024,
        }
    }

    pub fn with_scene_revision(mut self, revision: SceneRevision) -> Self {
        self.scene_revision = revision;
        self
    }
    pub fn with_transient_budget(mut self, bytes: u64) -> Self {
        self.transient_budget_bytes = bytes;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimitiveKind {
    Quad,
    Path,
    Text,
    Image,
    Glyph,
    NativeSurface,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    pub primitive: PrimitiveKind,
    pub blend: BlendMode,
    pub clip_depth: u32,
    pub layer_depth: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchBoundaryReason {
    Clip,
    Transform,
    Opacity,
    Layer,
    NativeSurface,
    Pipeline,
    Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferRange {
    pub start: usize,
    pub count: usize,
}

impl BufferRange {
    pub const fn new(start: usize, count: usize) -> Self {
        Self { start, count }
    }
    pub const fn end(self) -> usize {
        self.start + self.count
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BatchKind {
    Quad {
        instances: BufferRange,
    },
    Path {
        vertices: BufferRange,
        indices: BufferRange,
    },
    Text {
        bindings: BufferRange,
    },
    Image {
        bindings: BufferRange,
    },
    Glyph {
        bindings: BufferRange,
    },
    NativeSurface {
        bindings: BufferRange,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Batch {
    pub kind: BatchKind,
    pub pipeline: PipelineKey,
    pub clip: Option<Rect>,
    pub opacity: f32,
    pub source_commands: BufferRange,
    pub boundary_reason: Option<BatchBoundaryReason>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadInstance {
    pub rect: [f32; 4],
    pub radii: [f32; 4],
    pub color: [f32; 4],
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureBinding {
    pub resource: ResourceId,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadKind {
    TextRun,
    Image,
    GlyphAtlas,
    Vertex,
    Index,
    Instance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UploadRequest {
    pub kind: UploadKind,
    pub resource: Option<ResourceId>,
    pub bytes: u64,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OffscreenCost {
    pub width: u32,
    pub height: u32,
    pub passes: u32,
    pub transient_vram_bytes: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderPass {
    pub index: usize,
    pub offscreen: bool,
    pub bounds: Option<Rect>,
    pub batches: Vec<Batch>,
    pub cost: OffscreenCost,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledScene {
    pub scene_revision: SceneRevision,
    pub passes: Vec<RenderPass>,
    pub batches: Vec<Batch>,
    pub quad_instances: Vec<QuadInstance>,
    pub path_vertices: Vec<PathVertex>,
    pub path_indices: Vec<u32>,
    pub texture_bindings: Vec<TextureBinding>,
    pub uploads: Vec<UploadRequest>,
    pub paint_command_count: usize,
    pub fingerprint: u64,
    pub offscreen_cost: OffscreenCost,
}

impl CompiledScene {
    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }
    pub fn quad_instance_count(&self) -> usize {
        self.quad_instances.len()
    }
    pub fn upload_bytes(&self) -> u64 {
        self.uploads.iter().map(|upload| upload.bytes).sum()
    }
    pub fn snapshot(&self) -> CompiledSceneSnapshot {
        CompiledSceneSnapshot {
            scene_revision: self.scene_revision,
            paint_commands: self.paint_command_count,
            passes: self.pass_count(),
            batches: self.batch_count(),
            quad_instances: self.quad_instance_count(),
            uploads: self.uploads.len(),
            upload_bytes: self.upload_bytes(),
            fingerprint: self.fingerprint,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompiledSceneSnapshot {
    pub scene_revision: SceneRevision,
    pub paint_commands: usize,
    pub passes: usize,
    pub batches: usize,
    pub quad_instances: usize,
    pub uploads: usize,
    pub upload_bytes: u64,
    pub fingerprint: u64,
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct CompileKey {
    command_fingerprint: u64,
    scene_revision: u64,
    capability_generation: u64,
    capability_flags: u8,
    dpi_bits: u64,
    theme_revision: u64,
    font_revision: u64,
    image_revision: u64,
    glyph_revision: u64,
    resource_revision: u64,
}

pub struct RenderCompiler {
    cache: RenderCache<CompileKey, Arc<CompiledScene>>,
    committed: Option<Arc<CompiledScene>>,
    cache_limit_bytes: u64,
    rejected: u64,
}

impl RenderCompiler {
    pub fn new(cache_limit_bytes: u64) -> Self {
        Self {
            cache: RenderCache::new(cache_limit_bytes),
            committed: None,
            cache_limit_bytes,
            rejected: 0,
        }
    }
    pub fn committed(&self) -> Option<Arc<CompiledScene>> {
        self.committed.clone()
    }
    pub(crate) fn restore_committed(&mut self, scene: Option<Arc<CompiledScene>>) {
        self.committed = scene;
    }
    pub fn rejected_compiles(&self) -> u64 {
        self.rejected
    }
    pub fn cache_stats(&self) -> super::RenderCacheStats {
        self.cache.stats()
    }

    pub fn compile(
        &mut self,
        commands: &[PaintCommand],
        context: &CompileContext,
    ) -> Result<Arc<CompiledScene>> {
        let command_fingerprint = commands_fingerprint(commands);
        let key = CompileKey {
            command_fingerprint,
            scene_revision: context.scene_revision.get(),
            capability_generation: context.capabilities.generation,
            capability_flags: u8::from(context.capabilities.supports_paths)
                | (u8::from(context.capabilities.supports_native_surface) << 1)
                | (u8::from(context.capabilities.supports_backdrop) << 2),
            dpi_bits: context.dpi_scale.get().to_bits(),
            theme_revision: context.theme_revision,
            font_revision: context.font_revision,
            image_revision: context.image_revision,
            glyph_revision: context.glyph_revision,
            resource_revision: context.resource_revision,
        };
        if let Some(cached) = self.cache.get(&key).cloned() {
            self.committed = Some(cached.clone());
            return Ok(cached);
        }
        let compiled = match compile_commands(commands, context) {
            Ok(scene) => Arc::new(scene),
            Err(error) => {
                self.rejected = self.rejected.saturating_add(1);
                return Err(error);
            }
        };
        let bytes = estimate_scene_bytes(&compiled);
        if self.cache.insert(key, compiled.clone(), bytes) {
            self.committed = Some(compiled.clone());
        }
        Ok(compiled)
    }

    pub fn compile_tree(
        &mut self,
        tree: &RenderTree,
        context: &CompileContext,
    ) -> Result<Arc<CompiledScene>> {
        let previous = self.committed.clone();
        let mut chunks = Vec::with_capacity(tree.chunks().len());
        for chunk in tree.chunks() {
            let mut chunk_context = context.clone();
            chunk_context.scene_revision = chunk.revisions.scene;
            match self.compile(&chunk.commands, &chunk_context) {
                Ok(compiled) => chunks.push(compiled),
                Err(error) => {
                    self.committed = previous;
                    return Err(error);
                }
            }
        }
        let combined = Arc::new(combine_scenes(
            &chunks,
            context.scene_revision,
            tree.commands().len(),
        ));
        self.committed = Some(combined.clone());
        Ok(combined)
    }

    pub fn compile_retaining(
        &mut self,
        commands: &[PaintCommand],
        context: &CompileContext,
    ) -> Result<Arc<CompiledScene>> {
        self.compile(commands, context)
    }

    pub fn cache_limit_bytes(&self) -> u64 {
        self.cache_limit_bytes
    }
}

impl Default for RenderCompiler {
    fn default() -> Self {
        Self::new(32 * 1024 * 1024)
    }
}

#[derive(Clone, Debug, Default)]
pub struct HeadlessRenderer;

impl HeadlessRenderer {
    pub fn compile(
        commands: &[PaintCommand],
        context: &CompileContext,
    ) -> Result<CompiledSceneSnapshot> {
        let mut compiler = RenderCompiler::default();
        Ok(compiler.compile(commands, context)?.snapshot())
    }

    pub fn render(canvas: &Canvas, context: &CompileContext) -> Result<CompiledSceneSnapshot> {
        Self::compile(canvas.commands(), context)
    }
}

fn compile_commands(commands: &[PaintCommand], context: &CompileContext) -> Result<CompiledScene> {
    super::paint::validate_commands(commands)?;
    let mut output = CompiledScene {
        scene_revision: context.scene_revision,
        passes: vec![RenderPass {
            index: 0,
            offscreen: false,
            bounds: None,
            batches: Vec::new(),
            cost: OffscreenCost::default(),
        }],
        batches: Vec::new(),
        quad_instances: Vec::new(),
        path_vertices: Vec::new(),
        path_indices: Vec::new(),
        texture_bindings: Vec::new(),
        uploads: Vec::new(),
        paint_command_count: commands.len(),
        fingerprint: 0,
        offscreen_cost: OffscreenCost::default(),
    };
    let mut state = CompileState::default();
    for (index, command) in commands.iter().enumerate() {
        compile_command(&mut output, &mut state, command, index, context)?;
    }
    flush_batch(&mut output, &mut state);
    output.batches = output
        .passes
        .iter()
        .flat_map(|pass| pass.batches.iter().cloned())
        .collect();
    output.offscreen_cost =
        output
            .passes
            .iter()
            .fold(OffscreenCost::default(), |mut cost, pass| {
                cost.width = cost.width.max(pass.cost.width);
                cost.height = cost.height.max(pass.cost.height);
                cost.passes += pass.cost.passes;
                cost.transient_vram_bytes = cost
                    .transient_vram_bytes
                    .saturating_add(pass.cost.transient_vram_bytes);
                cost
            });
    output.fingerprint = compiled_fingerprint(&output);
    Ok(output)
}

#[derive(Default)]
struct CompileState {
    clips: Vec<Option<Rect>>,
    transforms: Vec<Transform2D>,
    layers: Vec<(f32, usize)>,
    current_clip: Option<Rect>,
    current_transform: Transform2D,
    current_opacity: f32,
    current_pass: usize,
    pass_stack: Vec<usize>,
    transient_bytes: u64,
    current_batch: Option<BatchBuilder>,
}

struct BatchBuilder {
    kind: PrimitiveKind,
    blend: BlendMode,
    start_command: usize,
    clip: Option<Rect>,
    opacity: f32,
    reason: Option<BatchBoundaryReason>,
    instance_start: usize,
    path_vertex_start: usize,
    path_index_start: usize,
    binding_start: usize,
    pass: usize,
}

fn compile_command(
    output: &mut CompiledScene,
    state: &mut CompileState,
    command: &PaintCommand,
    index: usize,
    context: &CompileContext,
) -> Result<()> {
    match command {
        PaintCommand::Clear(_) | PaintCommand::Marker(_) => flush_batch(output, state),
        PaintCommand::FillRect { rect, color } => add_quad(
            output,
            state,
            QuadCommand {
                rect: *rect,
                radii: [0.0; 4],
                color: ColorPaint::Solid(*color),
                blend: BlendMode::SourceOver,
                opacity: 1.0,
            },
            index,
        ),
        PaintCommand::DrawRect { rect, paint } => add_quad(
            output,
            state,
            QuadCommand {
                rect: *rect,
                radii: [0.0; 4],
                color: paint_color(paint),
                blend: paint.blend_mode,
                opacity: paint.opacity,
            },
            index,
        ),
        PaintCommand::DrawRoundedRect { rect, radii, paint } => add_quad(
            output,
            state,
            QuadCommand {
                rect: *rect,
                radii: [
                    radii.top_left,
                    radii.top_right,
                    radii.bottom_right,
                    radii.bottom_left,
                ],
                color: paint_color(paint),
                blend: paint.blend_mode,
                opacity: paint.opacity,
            },
            index,
        ),
        PaintCommand::FillPath { path, paint } => {
            add_path(output, state, path, paint, index, context)?
        }
        PaintCommand::StrokePath { path, paint, .. } => {
            add_path(output, state, path, paint, index, context)?
        }
        PaintCommand::PushClip(clip) => {
            flush_with_reason(output, state, BatchBoundaryReason::Clip);
            state.clips.push(state.current_clip);
            state.current_clip = state
                .current_clip
                .and_then(|old| old.intersection(clip.bounds()))
                .or(Some(clip.bounds()));
        }
        PaintCommand::PopClip => {
            flush_with_reason(output, state, BatchBoundaryReason::Clip);
            state.current_clip = state.clips.pop().flatten();
        }
        PaintCommand::PushTransform(transform) => {
            flush_with_reason(output, state, BatchBoundaryReason::Transform);
            state.transforms.push(state.current_transform);
            state.current_transform = state.current_transform.then(*transform);
        }
        PaintCommand::PopTransform => {
            flush_with_reason(output, state, BatchBoundaryReason::Transform);
            state.current_transform = state.transforms.pop().unwrap_or(Transform2D::IDENTITY);
        }
        PaintCommand::BeginLayer(layer) => {
            if let Some(backdrop) = layer.backdrop {
                if !context.capabilities.supports_backdrop {
                    return Err(Error::compile(
                        "render_compiler",
                        "backdrop filter is not supported by renderer capabilities",
                    ));
                }
                let _ = backdrop;
            }
            flush_with_reason(output, state, BatchBoundaryReason::Layer);
            let width = context
                .dpi_scale
                .logical_to_physical(layer.bounds.size.width)
                .map_err(Error::from)?;
            let height = context
                .dpi_scale
                .logical_to_physical(layer.bounds.size.height)
                .map_err(Error::from)?;
            let bytes = u64::from(width)
                .saturating_mul(u64::from(height))
                .saturating_mul(4);
            let projected = state.transient_bytes.saturating_add(bytes);
            if projected > context.transient_budget_bytes {
                return Err(Error::compile(
                    "render_compiler",
                    format!("transient layer budget exceeded: requested {bytes} bytes"),
                ));
            }
            let pass = output.passes.len();
            output.passes.push(RenderPass {
                index: pass,
                offscreen: true,
                bounds: Some(layer.bounds),
                batches: Vec::new(),
                cost: OffscreenCost {
                    width,
                    height,
                    passes: 1,
                    transient_vram_bytes: bytes,
                },
            });
            state.layers.push((state.current_opacity, pass));
            state.pass_stack.push(state.current_pass);
            state.current_pass = pass;
            state.transient_bytes = projected;
            state.current_opacity *= layer.opacity;
        }
        PaintCommand::EndLayer => {
            flush_with_reason(output, state, BatchBoundaryReason::Layer);
            let (opacity, _) = state
                .layers
                .pop()
                .ok_or_else(|| Error::compile("render_compiler", "layer stack underflow"))?;
            state.current_opacity = opacity;
            state.current_pass = state.pass_stack.pop().unwrap_or(0);
        }
        PaintCommand::DrawTextRun(run) => add_text(output, state, run, index, context),
        PaintCommand::DrawImage {
            rect,
            image,
            opacity,
            ..
        } => add_binding(
            output,
            state,
            PrimitiveKind::Image,
            *rect,
            ResourceId::from_parts(image.slot(), image.generation()),
            *opacity,
            index,
            context,
        ),
        PaintCommand::DrawGlyphAtlas { rect, page, .. } => add_binding(
            output,
            state,
            PrimitiveKind::Glyph,
            *rect,
            ResourceId::from_parts(page.slot(), page.generation()),
            1.0,
            index,
            context,
        ),
        PaintCommand::NativeSurface { rect, surface, .. } => {
            if !context.capabilities.supports_native_surface {
                return Err(Error::compile(
                    "render_compiler",
                    "NativeSurface requires an explicit renderer capability",
                ));
            }
            flush_with_reason(output, state, BatchBoundaryReason::NativeSurface);
            add_binding(
                output,
                state,
                PrimitiveKind::NativeSurface,
                *rect,
                *surface,
                1.0,
                index,
                context,
            );
            if let Some(batch) = state.current_batch.as_mut() {
                batch.reason = Some(BatchBoundaryReason::NativeSurface);
            }
            flush_batch(output, state);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ColorPaint {
    Solid(Color),
    Gradient(Color),
}

#[derive(Clone, Copy)]
struct QuadCommand {
    rect: Rect,
    radii: [f32; 4],
    color: ColorPaint,
    blend: BlendMode,
    opacity: f32,
}

fn paint_color(paint: &Paint) -> ColorPaint {
    match &paint.brush {
        Brush::Solid(color) => ColorPaint::Solid(*color),
        Brush::LinearGradient(gradient) => ColorPaint::Gradient(
            gradient
                .stops
                .first()
                .map_or(Color::WHITE, |stop| stop.color),
        ),
    }
}

fn add_quad(
    output: &mut CompiledScene,
    state: &mut CompileState,
    quad: QuadCommand,
    command: usize,
) {
    let rect = transformed_rect(quad.rect, state.current_transform);
    let instance = QuadInstance {
        rect: [
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
        ],
        radii: quad.radii,
        color: color_rgba(quad.color),
        opacity: state.current_opacity * quad.opacity,
    };
    let can_extend = state.current_batch.as_ref().is_some_and(|batch| {
        batch.kind == PrimitiveKind::Quad
            && batch.blend == quad.blend
            && batch.clip == state.current_clip
            && batch.pass == state.current_pass
    });
    let index = output.quad_instances.len();
    output.quad_instances.push(instance);
    if !can_extend {
        flush_with_reason(output, state, BatchBoundaryReason::Pipeline);
        state.current_batch = Some(BatchBuilder {
            kind: PrimitiveKind::Quad,
            blend: quad.blend,
            start_command: command,
            clip: state.current_clip,
            opacity: state.current_opacity,
            reason: None,
            instance_start: index,
            path_vertex_start: 0,
            path_index_start: 0,
            binding_start: 0,
            pass: state.current_pass,
        });
    }
}

fn add_path(
    output: &mut CompiledScene,
    state: &mut CompileState,
    path: &Path,
    paint: &Paint,
    command: usize,
    context: &CompileContext,
) -> Result<()> {
    if !context.capabilities.supports_paths {
        return Err(Error::compile(
            "render_compiler",
            "path primitive is not supported by renderer capabilities",
        ));
    }
    flush_with_reason(output, state, BatchBoundaryReason::Pipeline);
    let start_vertex = output.path_vertices.len();
    let start_index = output.path_indices.len();
    let points = flatten_path(path, state.current_transform);
    let color = color_rgba(paint_color(paint));
    for point in &points {
        output.path_vertices.push(PathVertex {
            position: [point.x, point.y],
            color,
            opacity: state.current_opacity * paint.opacity,
        });
    }
    for index in 1..points.len().saturating_sub(1) {
        output.path_indices.extend([
            start_vertex as u32,
            (start_vertex + index) as u32,
            (start_vertex + index + 1) as u32,
        ]);
    }
    state.current_batch = Some(BatchBuilder {
        kind: PrimitiveKind::Path,
        blend: paint.blend_mode,
        start_command: command,
        clip: state.current_clip,
        opacity: state.current_opacity,
        reason: None,
        instance_start: 0,
        path_vertex_start: start_vertex,
        path_index_start: start_index,
        binding_start: 0,
        pass: state.current_pass,
    });
    Ok(())
}

fn add_text(
    output: &mut CompiledScene,
    state: &mut CompileState,
    run: &TextRun,
    command: usize,
    context: &CompileContext,
) {
    add_binding(
        output,
        state,
        PrimitiveKind::Text,
        run.bounds,
        run.layout,
        1.0,
        command,
        context,
    );
    output.uploads.push(UploadRequest {
        kind: UploadKind::TextRun,
        resource: Some(run.layout),
        bytes: u64::from(run.glyph_count).saturating_mul(16),
        revision: context.font_revision,
    });
}

#[allow(clippy::too_many_arguments)]
fn add_binding(
    output: &mut CompiledScene,
    state: &mut CompileState,
    kind: PrimitiveKind,
    rect: Rect,
    resource: ResourceId,
    opacity: f32,
    command: usize,
    context: &CompileContext,
) {
    flush_with_reason(output, state, BatchBoundaryReason::Pipeline);
    let binding = output.texture_bindings.len();
    output.texture_bindings.push(TextureBinding {
        resource,
        generation: resource.generation(),
    });
    output.uploads.push(UploadRequest {
        kind: match kind {
            PrimitiveKind::Image => UploadKind::Image,
            PrimitiveKind::Glyph => UploadKind::GlyphAtlas,
            _ => UploadKind::Vertex,
        },
        resource: Some(resource),
        bytes: u64::from(rect.size.width.max(0.0) as u32)
            .saturating_mul(u64::from(rect.size.height.max(0.0) as u32))
            .saturating_mul(4),
        revision: context.resource_revision,
    });
    state.current_batch = Some(BatchBuilder {
        kind,
        blend: BlendMode::SourceOver,
        start_command: command,
        clip: state.current_clip,
        opacity: state.current_opacity * opacity,
        reason: None,
        instance_start: 0,
        path_vertex_start: 0,
        path_index_start: 0,
        binding_start: binding,
        pass: state.current_pass,
    });
}

fn flush_with_reason(
    output: &mut CompiledScene,
    state: &mut CompileState,
    reason: BatchBoundaryReason,
) {
    if state.current_batch.is_some() {
        if let Some(batch) = state.current_batch.as_mut() {
            batch.reason = Some(reason);
        }
        flush_batch(output, state);
    }
}

fn flush_batch(output: &mut CompiledScene, state: &mut CompileState) {
    let Some(builder) = state.current_batch.take() else {
        return;
    };
    let batch = Batch {
        kind: match builder.kind {
            PrimitiveKind::Quad => BatchKind::Quad {
                instances: BufferRange::new(
                    builder.instance_start,
                    output.quad_instances.len() - builder.instance_start,
                ),
            },
            PrimitiveKind::Path => BatchKind::Path {
                vertices: BufferRange::new(
                    builder.path_vertex_start,
                    output.path_vertices.len() - builder.path_vertex_start,
                ),
                indices: BufferRange::new(
                    builder.path_index_start,
                    output.path_indices.len() - builder.path_index_start,
                ),
            },
            PrimitiveKind::Text => BatchKind::Text {
                bindings: BufferRange::new(builder.binding_start, 1),
            },
            PrimitiveKind::Image => BatchKind::Image {
                bindings: BufferRange::new(builder.binding_start, 1),
            },
            PrimitiveKind::Glyph => BatchKind::Glyph {
                bindings: BufferRange::new(builder.binding_start, 1),
            },
            PrimitiveKind::NativeSurface => BatchKind::NativeSurface {
                bindings: BufferRange::new(builder.binding_start, 1),
            },
        },
        pipeline: PipelineKey {
            primitive: builder.kind,
            blend: builder.blend,
            clip_depth: state.clips.len() as u32,
            layer_depth: state.layers.len() as u32,
        },
        clip: builder.clip,
        opacity: builder.opacity,
        source_commands: BufferRange::new(builder.start_command, 1),
        boundary_reason: builder.reason,
    };
    if let Some(pass) = output.passes.get_mut(builder.pass) {
        pass.batches.push(batch);
    }
}

fn flatten_path(path: &Path, transform: Transform2D) -> Vec<Point> {
    let mut points = Vec::new();
    for segment in path.segments() {
        match *segment {
            PathSegment::MoveTo(point) | PathSegment::LineTo(point) => {
                points.push(transform.transform_point(point))
            }
            PathSegment::QuadraticTo { to, .. } | PathSegment::CubicTo { to, .. } => {
                points.push(transform.transform_point(to))
            }
            PathSegment::Close => {}
        }
    }
    points
}

fn transformed_rect(rect: Rect, transform: Transform2D) -> Rect {
    let points = [
        Point::new(rect.min_x(), rect.min_y()),
        Point::new(rect.max_x(), rect.min_y()),
        Point::new(rect.max_x(), rect.max_y()),
        Point::new(rect.min_x(), rect.max_y()),
    ]
    .map(|point| transform.transform_point(point));
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    Rect::from_xywh(
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    )
}

fn color_rgba(color: ColorPaint) -> [f32; 4] {
    let color = match color {
        ColorPaint::Solid(color) | ColorPaint::Gradient(color) => color,
    };
    [
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        f32::from(color.alpha) / 255.0,
    ]
}
fn commands_fingerprint(commands: &[PaintCommand]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for command in commands {
        for byte in command.stable_debug().bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}
fn compiled_fingerprint(scene: &CompiledScene) -> u64 {
    let mut hash = commands_fingerprint(&[]);
    for batch in &scene.batches {
        for byte in format!("{batch:?}").bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}
fn estimate_scene_bytes(scene: &CompiledScene) -> u64 {
    (scene.quad_instances.len() * std::mem::size_of::<QuadInstance>()
        + scene.path_vertices.len() * std::mem::size_of::<PathVertex>()
        + scene.path_indices.len() * std::mem::size_of::<u32>()) as u64
}

fn combine_scenes(
    scenes: &[Arc<CompiledScene>],
    scene_revision: SceneRevision,
    paint_command_count: usize,
) -> CompiledScene {
    let mut combined = CompiledScene {
        scene_revision,
        passes: vec![RenderPass {
            index: 0,
            offscreen: false,
            bounds: None,
            batches: Vec::new(),
            cost: OffscreenCost::default(),
        }],
        batches: Vec::new(),
        quad_instances: Vec::new(),
        path_vertices: Vec::new(),
        path_indices: Vec::new(),
        texture_bindings: Vec::new(),
        uploads: Vec::new(),
        paint_command_count,
        fingerprint: 0,
        offscreen_cost: OffscreenCost::default(),
    };
    let mut command_offset = 0;
    for scene in scenes {
        let instance_offset = combined.quad_instances.len();
        let vertex_offset = combined.path_vertices.len();
        let index_offset = combined.path_indices.len();
        let binding_offset = combined.texture_bindings.len();
        combined
            .quad_instances
            .extend_from_slice(&scene.quad_instances);
        combined
            .path_vertices
            .extend_from_slice(&scene.path_vertices);
        combined.path_indices.extend(
            scene
                .path_indices
                .iter()
                .map(|index| index.saturating_add(vertex_offset as u32)),
        );
        combined
            .texture_bindings
            .extend_from_slice(&scene.texture_bindings);
        combined.uploads.extend_from_slice(&scene.uploads);
        for pass in &scene.passes {
            let mut pass = pass.clone();
            for batch in &mut pass.batches {
                offset_batch(
                    batch,
                    command_offset,
                    instance_offset,
                    vertex_offset,
                    index_offset,
                    binding_offset,
                );
            }
            if pass.offscreen {
                pass.index = combined.passes.len();
                combined.passes.push(pass);
            } else {
                combined.passes[0].batches.extend(pass.batches);
            }
        }
        command_offset += scene.paint_command_count;
    }
    combined.batches = combined
        .passes
        .iter()
        .flat_map(|pass| pass.batches.iter().cloned())
        .collect();
    combined.offscreen_cost =
        combined
            .passes
            .iter()
            .fold(OffscreenCost::default(), |mut total, pass| {
                total.width = total.width.max(pass.cost.width);
                total.height = total.height.max(pass.cost.height);
                total.passes = total.passes.saturating_add(pass.cost.passes);
                total.transient_vram_bytes = total
                    .transient_vram_bytes
                    .saturating_add(pass.cost.transient_vram_bytes);
                total
            });
    combined.fingerprint = compiled_fingerprint(&combined);
    combined
}

fn offset_batch(
    batch: &mut Batch,
    command_offset: usize,
    instance_offset: usize,
    vertex_offset: usize,
    index_offset: usize,
    binding_offset: usize,
) {
    batch.source_commands.start += command_offset;
    match &mut batch.kind {
        BatchKind::Quad { instances } => instances.start += instance_offset,
        BatchKind::Path { vertices, indices } => {
            vertices.start += vertex_offset;
            indices.start += index_offset;
        }
        BatchKind::Text { bindings }
        | BatchKind::Image { bindings }
        | BatchKind::Glyph { bindings }
        | BatchKind::NativeSurface { bindings } => bindings.start += binding_offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, ElementId, LayoutRevision, Rect, ResourceRevision};
    use crate::render::{ChunkPrerequisites, ChunkRevisionTuple, RenderNodeDescriptor, RenderTree};
    use std::collections::BTreeSet;
    #[test]
    fn five_adjacent_rects_share_one_quad_batch() {
        let commands = (0..5)
            .map(|index| PaintCommand::FillRect {
                rect: Rect::from_xywh(index as f32, 0.0, 1.0, 1.0),
                color: Color::WHITE,
            })
            .collect::<Vec<_>>();
        let context = CompileContext::new(RendererCapabilities::default(), DpiScale::ONE);
        let mut compiler = RenderCompiler::default();
        let scene = compiler.compile(&commands, &context).unwrap();
        assert_eq!(scene.batch_count(), 1);
        assert_eq!(scene.quad_instance_count(), 5);
    }
    #[test]
    fn invalid_native_surface_does_not_replace_committed_scene() {
        let context = CompileContext::new(RendererCapabilities::default(), DpiScale::ONE);
        let mut compiler = RenderCompiler::default();
        let good = [PaintCommand::FillRect {
            rect: Rect::from_xywh(0.0, 0.0, 1.0, 1.0),
            color: Color::WHITE,
        }];
        compiler.compile(&good, &context).unwrap();
        let bad = [PaintCommand::NativeSurface {
            rect: Rect::from_xywh(0.0, 0.0, 1.0, 1.0),
            surface: ResourceId::from_parts(1, 1),
            opaque: true,
        }];
        assert!(compiler.compile(&bad, &context).is_err());
        assert!(compiler.committed().is_some());
    }

    #[test]
    fn compile_tree_reuses_unchanged_chunk_results() {
        let root_id = ElementId::from_parts(0, 1);
        let child_id = ElementId::from_parts(1, 1);
        let command = |color| PaintCommand::FillRect {
            rect: Rect::from_xywh(0.0, 0.0, 4.0, 4.0),
            color,
        };
        let mut root = RenderNodeDescriptor::new(
            root_id,
            Rect::from_xywh(0.0, 0.0, 4.0, 4.0),
            [command(Color::WHITE)],
        )
        .unwrap();
        root.boundary = true;
        let mut child = RenderNodeDescriptor::new(
            child_id,
            Rect::from_xywh(0.0, 0.0, 4.0, 4.0),
            [command(Color::BLACK)],
        )
        .unwrap();
        child.parent = Some(root_id);
        child.boundary = true;
        let mut tree = RenderTree::new();
        let revisions = ChunkRevisionTuple {
            layout: LayoutRevision::new(1),
            scene: SceneRevision::new(1),
            resource: ResourceRevision::ZERO,
        };
        tree.collect(
            &[root.clone(), child.clone()],
            revisions,
            ChunkPrerequisites::default(),
            &BTreeSet::new(),
            false,
        )
        .unwrap();
        let mut compiler = RenderCompiler::default();
        let first_context = CompileContext::new(RendererCapabilities::default(), DpiScale::ONE)
            .with_scene_revision(SceneRevision::new(1));
        compiler.compile_tree(&tree, &first_context).unwrap();

        child.commands = Arc::from([command(Color::WHITE)]);
        let mut dirty = BTreeSet::new();
        dirty.insert(child_id);
        tree.collect(
            &[root, child],
            ChunkRevisionTuple {
                scene: SceneRevision::new(2),
                ..revisions
            },
            ChunkPrerequisites::default(),
            &dirty,
            false,
        )
        .unwrap();
        let second_context = CompileContext::new(RendererCapabilities::default(), DpiScale::ONE)
            .with_scene_revision(SceneRevision::new(2));
        let second = compiler.compile_tree(&tree, &second_context).unwrap();

        assert_eq!(compiler.cache_stats().hits, 1);
        assert_eq!(second.pass_count(), 1);
        assert_eq!(second.quad_instance_count(), 2);
    }
}
