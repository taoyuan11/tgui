//! Retained, backend-independent rendering pipeline.
//!
//! Paint recording, scene collection, compilation, caching, and the headless
//! backend are always available. Only the concrete GPU executor is gated by
//! the `render` feature, keeping core and snapshot tests GPU-free.

mod cache;
mod compiler;
mod paint;
mod scene;

#[cfg(feature = "render")]
pub mod wgpu;

pub use cache::{CacheEntryKind, RenderCache, RenderCacheStats};
pub use compiler::{
    Batch, BatchBoundaryReason, BatchKind, BufferRange, CompileContext, CompiledScene,
    CompiledSceneSnapshot, HeadlessRenderer, OffscreenCost, PipelineKey, PrimitiveKind,
    QuadInstance, RenderCompiler, RenderPass, RendererCapabilities, TextureBinding, UploadKind,
    UploadRequest,
};
pub use paint::{
    BackdropFilter, BlendMode, Brush, Canvas, FillRule, GradientStop, ImageSampling, LayerSpec,
    LinearGradient, Paint, PaintCommand, PaintSnapshot, Path, PathSegment, Shadow, StrokeStyle,
    TextRun,
};
pub use scene::{
    ChunkInvalidationReason, ChunkPrerequisites, ChunkRevisionTuple, RenderNode,
    RenderNodeDescriptor, RenderTree, RenderTreeReport, SceneChunk, SceneSnapshot,
};
