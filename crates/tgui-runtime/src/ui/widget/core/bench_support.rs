use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::mem::{align_of, size_of};
use std::time::{Duration, Instant};

use crate::animation::AnimationEngine;
use crate::foundation::binding::{DependencyPhase, InvalidationSignal, PropertySlot};
use crate::foundation::color::Color;
use crate::media::MediaManager;
use crate::text::font::{FontCatalog, FontManager};
use crate::ui::layout::{Length, Value};
use crate::ui::theme::Theme;
use crate::ui::unit::{Dp, UnitContext};
use crate::ui::widget::{
    BackdropBlurPrimitive, BrushPrimitive, ComputedScene, Point, ReactiveScenePropertyValue, Rect,
    TextPrimitive, VisualContextSnapshot, WidgetId, WidgetStateMap,
};
use smallvec::SmallVec;

use super::{CollectedSceneCache, ResolvedSceneLayout, SceneChunkParts, WidgetTree};
use crate::runtime::reactive_slots::{
    build_reactive_slot_binding_for_scene, write_reactive_slot_patch_to_scene, ReactiveSlotBinding,
};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct GpuBenchmarkAdapterInfo {
    pub name: String,
    pub backend: String,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuTextCacheStats {
    pub cache_entries: usize,
    pub atlas_pages: usize,
    pub dedicated_textures: usize,
    pub unique_bind_groups: usize,
    pub r8_atlas_pages: usize,
    pub rgba_atlas_pages: usize,
    pub r8_allocations: usize,
    pub rgba_allocations: usize,
    pub atlas_live_allocations: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuTextCacheActivityStats {
    pub hits: usize,
    pub misses: usize,
    pub atlas_releases: usize,
    pub retained_prepare_cache_clears: usize,
    pub whole_pages_released: usize,
    pub whole_page_atlas_releases: usize,
    pub individual_atlas_releases: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuTextAtlasUploadStats {
    pub write_calls: usize,
    pub uploaded_bytes: usize,
    pub shadow_bytes: usize,
    pub shadow_budget_bytes: usize,
    pub r8_uploaded_bytes: usize,
    pub rgba_uploaded_bytes: usize,
    pub r8_shadow_bytes: usize,
    pub rgba_shadow_bytes: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuTextBlendStats {
    pub transparent_source_pixels: usize,
    pub direct_copy_pixels: usize,
    pub general_blend_pixels: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuGlyphRasterCacheStats {
    pub image_entries: usize,
    pub image_bytes: usize,
    pub outline_entries: usize,
    /// New cosmic-text raster-cache keys inserted since statistics were reset.
    pub image_insertions: usize,
    /// Whole-cache resets performed by the legacy one-frame control path.
    pub frame_resets: usize,
    pub budget_evictions: usize,
    pub font_system_resets: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuDrawStats {
    pub rect_commands: usize,
    pub rect_draw_calls: usize,
    pub brush_commands: usize,
    pub brush_draw_calls: usize,
    pub mesh_commands: usize,
    pub mesh_draw_calls: usize,
    /// Prepared sprite commands that reached a visible draw. Text and texture primitives share
    /// the sprite pipeline; a text-dense atlas workload is overwhelmingly text here.
    pub sprite_commands: usize,
    /// Actual `RenderPass::draw` calls encoded for those commands after safe contiguous batching.
    pub sprite_draw_calls: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuPrepareStats {
    pub total_commands: usize,
    pub rebuilt_commands: usize,
    pub reused_commands: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuCacheLivenessStats {
    pub scans: usize,
    pub paint_only_skips: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuCleanFrameCacheStats {
    pub hits: usize,
    pub misses: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuTextureSceneStats {
    pub texture_commands: usize,
    pub unique_texture_ids: usize,
    pub unique_clip_rects: usize,
}

/// Retained texture identity and cache state exposed only to benchmark/equivalence probes.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct GpuTextureRetainedState {
    pub texture_ids: Vec<u64>,
    pub texture_revisions: Vec<u64>,
    pub frames: Vec<Rect>,
    pub opacities: Vec<f32>,
    pub media_key_fingerprints: Vec<Option<u64>>,
    pub media_layout_fingerprints: Vec<Option<u64>>,
    pub mask_tints: Vec<Option<Color>>,
    pub prepare_cache_serial: u64,
    pub cache_liveness_dirty: bool,
    pub dirty_draw_range_count: usize,
}

/// 返回 `widget/core` benchmark 的默认视口尺寸。
#[allow(dead_code)]
pub fn default_bench_viewport() -> Rect {
    Rect::new(0.0, 0.0, 1280.0, 720.0)
}

/// 用于 benchmark 的默认运行环境。
#[allow(dead_code)]
pub struct WidgetBenchmarkContext {
    font_manager: FontManager,
    theme: Theme,
    media: MediaManager,
    animations: AnimationEngine,
    style_sheet: crate::ui::widget::StyleSheet,
    units: UnitContext,
    viewport: Rect,
    cached_layout: Option<ResolvedSceneLayout<()>>,
    cached_scene: Option<ComputedScene<()>>,
    cached_scene_chunks: HashMap<WidgetId, ComputedScene<()>>,
    cached_chunk_parts: HashMap<WidgetId, SceneChunkParts<()>>,
    cached_visual_contexts: HashMap<WidgetId, VisualContextSnapshot>,
    scroll_offsets: HashMap<WidgetId, Point>,
    last_tree_ptr: Option<usize>,
    gpu_renderer: Option<crate::rendering::renderer::HeadlessBenchRenderer>,
    last_animation_frame_activity: AnimationFrameActivityStats,
    animation_reactive_slot_bindings: HashMap<(WidgetId, PropertySlot), ReactiveSlotBinding>,
    force_full_layout_animation_rebuild: bool,
    force_full_scene_animation_recollect: bool,
    force_individual_reactive_resolve: bool,
    force_legacy_texture_mask_tint_reactive_resolve: bool,
    force_legacy_widget_shadow_opacity: bool,
    force_legacy_canvas_shadow_opacity: bool,
    force_legacy_text_color_reactive_resolve: bool,
    force_legacy_background_reactive_resolve: bool,
    force_legacy_background_brush_reactive_resolve: bool,
    force_legacy_background_blur_reactive_resolve: bool,
    force_legacy_offset_reactive_resolve: bool,
    force_legacy_scale_reactive_resolve: bool,
    force_legacy_border_color_reactive_resolve: bool,
    force_legacy_border_radius_reactive_resolve: bool,
    force_legacy_border_width_reactive_resolve: bool,
    force_legacy_text_opacity_reactive_resolve: bool,
    force_legacy_container_opacity_reactive_resolve: bool,
    force_legacy_progress_value_reactive_resolve: bool,
}

/// benchmark 的轻量统计摘要，避免直接暴露内部布局/scene 类型。
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetBenchmarkStats {
    pub dependency_count: usize,
    pub has_global_dependency: bool,
    pub shape_count: usize,
    pub mesh_count: usize,
    pub mesh_vertex_count: usize,
    pub text_count: usize,
    pub texture_count: usize,
    pub overlay_shape_count: usize,
    pub hit_region_count: usize,
    pub scroll_region_count: usize,
}

/// Benchmark-only value snapshot for one determinate ProgressBar retained resolver target.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct ProgressValueResolveSnapshot {
    pub track_rect: Rect,
    pub fill_rect: Rect,
    pub track_color: Color,
    pub fill_color: Color,
    pub label: Option<ProgressValueLabelSnapshot>,
}

/// Optional implicit percentage-label payload paired with [`ProgressValueResolveSnapshot`].
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct ProgressValueLabelSnapshot {
    pub frame: Rect,
    pub content: String,
    pub font_family: Option<String>,
}

/// Benchmark-only value snapshot for one retained SliderValue resolver target.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct SliderValueResolveSnapshot {
    pub value: f32,
    pub track_rect: Rect,
    pub active_rect: Rect,
    pub thumb_rect: Rect,
    pub track_color: Color,
    pub active_track_color: Color,
    pub thumb_color: Color,
    pub thumb_border: Option<(Color, f32)>,
    pub label: Option<ProgressValueLabelSnapshot>,
}

/// Benchmark-only snapshot of the retained primitives moved by one Container Offset target.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerOffsetResolveSnapshot {
    pub background: Option<(Rect, Color)>,
    pub border: Option<(Rect, f32, Color)>,
    pub backdrop_blur: Option<BackdropBlurPrimitive>,
    pub brush: Option<BrushPrimitive>,
    pub has_texture: bool,
    pub container_occluder: Option<(WidgetId, Rect, Option<Rect>)>,
}

/// Benchmark-only snapshot of the retained primitives resized by one Container Scale target.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerScaleResolveSnapshot {
    pub background: Option<(Rect, Color, f32)>,
    pub border: Option<(Rect, f32, Color, f32)>,
    pub backdrop_blur: Option<BackdropBlurPrimitive>,
    pub brush: Option<BrushPrimitive>,
    pub has_texture: bool,
    pub container_occluder: Option<(WidgetId, Rect, Option<Rect>)>,
}

/// Describes the retained scroll-region lookup used by event-handling benchmarks.
///
/// Every container contributes a [`ScrollRegion`], while only containers with real overflow are
/// wheel/touch candidates. Keeping both counts visible prevents a benchmark from accidentally
/// treating cache maintenance or fixture construction as lookup work.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollRegionLookupStats {
    pub region_count: usize,
    pub uses_index: bool,
    pub scrollable_candidate_count: usize,
    pub scrollbar_candidate_count: usize,
}

/// One prepared scroll-target lookup, including how many retained candidates were examined.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollTargetLookupStats {
    pub found_target: bool,
    pub candidate_visits: usize,
}

/// Instrumentation for one real animation-cache refresh performed by the benchmark context.
/// This prevents an end-to-end benchmark from silently measuring a stable frame or a fallback
/// rebuild after the animation has settled.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimationFrameActivityStats {
    pub refresh_changed: bool,
    pub layout_widget_count: usize,
    pub scene_widget_count: usize,
    pub layout_root_count: usize,
    pub scene_root_count: usize,
    pub layout_patch_succeeded: bool,
    pub scene_patch_succeeded: bool,
    pub reactive_slot_write_succeeded: bool,
    pub reactive_slot_resolve_duration: Duration,
    pub full_layout_rebuild: bool,
    pub full_scene_recollect: bool,
    pub fell_back: bool,
}

/// Counts which intrinsic text route a real layout/scene pass exercised.
/// `precise_layout_builds` is also an allocation proxy: every build creates the
/// line table, caret boundary vectors, grapheme geometry, and its owning `Arc`.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextMeasureActivityStats {
    pub measure_only_calls: u64,
    pub measure_only_cache_misses: u64,
    pub precise_measure_calls: u64,
    pub precise_layout_builds: u64,
}

/// Size and alignment of one type participating in widget resolution.
///
/// This is benchmark-only telemetry: it keeps large recursive values visible when compiler or
/// dependency changes alter the amount of memory moved by a full layout rebuild.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetTypeSizeEntry {
    pub name: &'static str,
    pub size: usize,
    pub align: usize,
}

/// Return type-layout telemetry for the hot values and representative styles used by widget
/// resolution. The function intentionally lives behind `bench-support`; none of these internal
/// layouts are part of the stable public API.
#[allow(dead_code)]
pub fn widget_type_size_telemetry() -> Vec<WidgetTypeSizeEntry> {
    macro_rules! entry {
        ($ty:ty) => {
            WidgetTypeSizeEntry {
                name: stringify!($ty),
                size: size_of::<$ty>(),
                align: align_of::<$ty>(),
            }
        };
    }

    vec![
        entry!(super::Element<()>),
        entry!(super::ResolvedElement<()>),
        entry!(super::WidgetKind<()>),
        entry!(super::ResolvedWidgetKind<()>),
        entry!(crate::ui::layout::LayoutStyle),
        entry!(super::VisualStyle),
        entry!(crate::ui::widget::ContainerLayout),
        entry!(crate::ui::widget::common::ChildSource<()>),
        entry!(crate::ui::layout::Value<bool>),
        entry!(crate::ui::layout::Value<usize>),
        entry!(Vec<super::Element<()>>),
        entry!(Option<Box<super::Element<()>>>),
        entry!(crate::ui::widget::text::Text),
        entry!(crate::ui::widget::style::WidgetSurfaceStyle),
        entry!(crate::ui::widget::style::ContainerStyle),
        entry!(crate::ui::widget::style::TextWidgetStyle),
        entry!(crate::ui::widget::style::ButtonStyle),
        entry!(crate::ui::widget::style::CheckboxStyle),
        entry!(crate::ui::widget::style::SelectStyle),
        entry!(crate::ui::widget::style::InputStyle),
        entry!(crate::ui::widget::style::ModalStyle),
        entry!(crate::ui::widget::style::DrawerStyle),
        entry!(crate::ui::widget::DataGridStyle),
    ]
}

impl WidgetBenchmarkContext {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            font_manager: FontManager::new(&FontCatalog::default()),
            theme: Theme::default(),
            media: MediaManager::new(InvalidationSignal::new()),
            animations: AnimationEngine::default(),
            style_sheet: crate::ui::widget::StyleSheet::default(),
            units: UnitContext::default(),
            viewport: default_bench_viewport(),
            cached_layout: None,
            cached_scene: None,
            cached_scene_chunks: HashMap::new(),
            cached_chunk_parts: HashMap::new(),
            cached_visual_contexts: HashMap::new(),
            scroll_offsets: HashMap::new(),
            last_tree_ptr: None,
            gpu_renderer: None,
            last_animation_frame_activity: AnimationFrameActivityStats::default(),
            animation_reactive_slot_bindings: HashMap::new(),
            force_full_layout_animation_rebuild: false,
            force_full_scene_animation_recollect: false,
            force_individual_reactive_resolve: false,
            force_legacy_texture_mask_tint_reactive_resolve: false,
            force_legacy_widget_shadow_opacity: false,
            force_legacy_canvas_shadow_opacity: false,
            force_legacy_text_color_reactive_resolve: false,
            force_legacy_background_reactive_resolve: false,
            force_legacy_background_brush_reactive_resolve: false,
            force_legacy_background_blur_reactive_resolve: false,
            force_legacy_offset_reactive_resolve: false,
            force_legacy_scale_reactive_resolve: false,
            force_legacy_border_color_reactive_resolve: false,
            force_legacy_border_radius_reactive_resolve: false,
            force_legacy_border_width_reactive_resolve: false,
            force_legacy_text_opacity_reactive_resolve: false,
            force_legacy_container_opacity_reactive_resolve: false,
            force_legacy_progress_value_reactive_resolve: false,
        }
    }

    #[allow(dead_code)]
    pub fn with_viewport(mut self, viewport: Rect) -> Self {
        self.viewport = viewport;
        self
    }

    #[allow(dead_code)]
    pub fn set_benchmark_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.cached_layout = None;
        self.clear_scene_cache();
    }

    #[allow(dead_code)]
    pub fn invalidate_all(&mut self) {
        self.cached_layout = None;
        self.clear_scene_cache();
    }

    #[allow(dead_code)]
    pub fn text_measure_activity(&self) -> TextMeasureActivityStats {
        let (
            measure_only_calls,
            measure_only_cache_misses,
            precise_measure_calls,
            precise_layout_builds,
        ) = self.font_manager.text_measure_activity();
        TextMeasureActivityStats {
            measure_only_calls,
            measure_only_cache_misses,
            precise_measure_calls,
            precise_layout_builds,
        }
    }

    #[allow(dead_code)]
    pub fn reset_text_measure_activity(&self) {
        self.font_manager.reset_text_measure_activity();
    }

    #[allow(dead_code)]
    pub fn clear_text_measure_caches(&self) {
        self.font_manager.clear_text_measure_caches_for_benchmark();
    }

    #[allow(dead_code)]
    pub fn force_precise_text_measurement(&self, force: bool) {
        self.font_manager
            .force_precise_measurement_for_benchmark(force);
    }

    /// Lazily creates the real tgui renderer against an offscreen GPU texture.
    /// Returns an explicit error when the host has no headless-compatible adapter.
    #[allow(dead_code)]
    pub fn initialize_headless_gpu(&mut self) -> Result<GpuBenchmarkAdapterInfo, String> {
        if self.gpu_renderer.is_none() {
            let size = crate::platform::dpi::PhysicalSize::new(
                self.viewport.width.max(Dp::new(1.0)).get().ceil() as u32,
                self.viewport.height.max(Dp::new(1.0)).get().ceil() as u32,
            );
            self.gpu_renderer = Some(
                crate::rendering::renderer::HeadlessBenchRenderer::new(size)
                    .map_err(|error| error.to_string())?,
            );
        }
        let renderer = self.gpu_renderer.as_ref().expect("renderer initialized");
        Ok(GpuBenchmarkAdapterInfo {
            name: renderer.adapter_name.clone(),
            backend: renderer.backend.clone(),
        })
    }

    /// Runs the same retained-scene prepare, vertex upload, render-pass encoding,
    /// queue submission, and GPU completion path used by the window renderer.
    #[allow(dead_code)]
    pub fn render_cached_scene_to_headless_gpu(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
    ) -> Result<WidgetBenchmarkStats, String> {
        self.sync_cache(tree, now, true);
        if self.gpu_renderer.is_none() {
            self.initialize_headless_gpu()?;
        }
        let computed = self
            .cached_scene
            .as_mut()
            .expect("benchmark scene cache should exist");
        self.gpu_renderer
            .as_mut()
            .expect("renderer initialized")
            .render_and_wait(
                &mut computed.scene,
                &self.font_manager,
                &computed.scroll_regions,
                &computed.transform_records,
            )
            .map_err(|error| error.to_string())?;
        Ok(WidgetBenchmarkStats {
            dependency_count: computed.dependencies.dependency_count(),
            has_global_dependency: computed.dependencies.has_global_dependency(),
            shape_count: computed.scene.shapes.len(),
            mesh_count: computed.scene.meshes.len(),
            mesh_vertex_count: computed
                .scene
                .meshes
                .iter()
                .map(|mesh| mesh.vertices.len())
                .sum(),
            text_count: computed.scene.texts.len(),
            texture_count: computed.scene.textures.len(),
            overlay_shape_count: computed.scene.overlay_shapes.len(),
            hit_region_count: computed.hit_regions.len() + computed.overlay_hit_regions.len(),
            scroll_region_count: computed.scroll_regions.len(),
        })
    }

    #[allow(dead_code)]
    pub fn headless_text_gpu_cache_stats(&self) -> Option<GpuTextCacheStats> {
        let (
            cache_entries,
            atlas_pages,
            dedicated_textures,
            unique_bind_groups,
            r8_atlas_pages,
            rgba_atlas_pages,
            r8_allocations,
            rgba_allocations,
            atlas_live_allocations,
        ) = self.gpu_renderer.as_ref()?.text_gpu_cache_stats();
        Some(GpuTextCacheStats {
            cache_entries,
            atlas_pages,
            dedicated_textures,
            unique_bind_groups,
            r8_atlas_pages,
            rgba_atlas_pages,
            r8_allocations,
            rgba_allocations,
            atlas_live_allocations,
        })
    }

    #[allow(dead_code)]
    pub fn headless_glyph_raster_cache_stats(&self) -> Option<GpuGlyphRasterCacheStats> {
        let stats = self.gpu_renderer.as_ref()?.text_raster_cache_stats();
        Some(GpuGlyphRasterCacheStats {
            image_entries: stats.image_entries,
            image_bytes: stats.image_bytes,
            outline_entries: stats.outline_entries,
            image_insertions: stats.image_insertions,
            frame_resets: stats.frame_resets,
            budget_evictions: stats.budget_evictions,
            font_system_resets: stats.font_system_resets,
        })
    }

    #[allow(dead_code)]
    pub fn headless_text_gpu_cache_activity_stats(&self) -> Option<GpuTextCacheActivityStats> {
        let (
            hits,
            misses,
            atlas_releases,
            retained_prepare_cache_clears,
            whole_pages_released,
            whole_page_atlas_releases,
            individual_atlas_releases,
        ) = self.gpu_renderer.as_ref()?.text_cache_activity_stats();
        Some(GpuTextCacheActivityStats {
            hits,
            misses,
            atlas_releases,
            retained_prepare_cache_clears,
            whole_pages_released,
            whole_page_atlas_releases,
            individual_atlas_releases,
        })
    }

    #[allow(dead_code)]
    pub fn headless_text_atlas_upload_stats(&self) -> Option<GpuTextAtlasUploadStats> {
        let (
            write_calls,
            uploaded_bytes,
            shadow_bytes,
            shadow_budget_bytes,
            r8_uploaded_bytes,
            rgba_uploaded_bytes,
            r8_shadow_bytes,
            rgba_shadow_bytes,
        ) = self.gpu_renderer.as_ref()?.text_atlas_upload_stats();
        Some(GpuTextAtlasUploadStats {
            write_calls,
            uploaded_bytes,
            shadow_bytes,
            shadow_budget_bytes,
            r8_uploaded_bytes,
            rgba_uploaded_bytes,
            r8_shadow_bytes,
            rgba_shadow_bytes,
        })
    }

    #[allow(dead_code)]
    pub fn headless_text_blend_stats(&self) -> Option<GpuTextBlendStats> {
        let stats = self.gpu_renderer.as_ref()?.text_blend_stats();
        Some(GpuTextBlendStats {
            transparent_source_pixels: stats.transparent_source_pixels,
            direct_copy_pixels: stats.direct_copy_pixels,
            general_blend_pixels: stats.general_blend_pixels,
        })
    }

    #[allow(dead_code)]
    pub fn headless_gpu_draw_stats(&self) -> Option<GpuDrawStats> {
        let (
            rect_commands,
            rect_draw_calls,
            brush_commands,
            brush_draw_calls,
            mesh_commands,
            mesh_draw_calls,
            sprite_commands,
            sprite_draw_calls,
        ) = self.gpu_renderer.as_ref()?.scene_draw_stats();
        Some(GpuDrawStats {
            rect_commands,
            rect_draw_calls,
            brush_commands,
            brush_draw_calls,
            mesh_commands,
            mesh_draw_calls,
            sprite_commands,
            sprite_draw_calls,
        })
    }

    #[allow(dead_code)]
    pub fn animation_frame_activity(&self) -> AnimationFrameActivityStats {
        self.last_animation_frame_activity
    }

    #[allow(dead_code)]
    pub fn headless_gpu_prepare_stats(&self) -> Option<GpuPrepareStats> {
        let (total_commands, rebuilt_commands, reused_commands) =
            self.gpu_renderer.as_ref()?.prepare_reuse_stats();
        Some(GpuPrepareStats {
            total_commands,
            rebuilt_commands,
            reused_commands,
        })
    }

    #[allow(dead_code)]
    pub fn headless_gpu_cache_liveness_stats(&self) -> Option<GpuCacheLivenessStats> {
        let (scans, paint_only_skips) = self.gpu_renderer.as_ref()?.cache_liveness_stats();
        Some(GpuCacheLivenessStats {
            scans,
            paint_only_skips,
        })
    }

    #[allow(dead_code)]
    pub fn headless_gpu_clean_frame_cache_stats(&self) -> Option<GpuCleanFrameCacheStats> {
        let (hits, misses) = self
            .gpu_renderer
            .as_ref()?
            .clean_prepared_frame_cache_stats();
        Some(GpuCleanFrameCacheStats { hits, misses })
    }

    #[allow(dead_code)]
    pub fn has_active_property_animations(&self) -> bool {
        self.animations.has_active_animations()
    }

    #[allow(dead_code)]
    pub fn set_force_full_layout_animation_rebuild(&mut self, enabled: bool) {
        self.force_full_layout_animation_rebuild = enabled;
    }

    #[allow(dead_code)]
    pub fn cached_layout_widget_count(&self) -> usize {
        self.cached_layout
            .as_ref()
            .map(ResolvedSceneLayout::widget_count)
            .unwrap_or(0)
    }

    /// Isomorphic A/B control: retain layout, but rebuild the complete scene for every scene-only
    /// animation refresh instead of using retained property slots or subtree patching.
    #[allow(dead_code)]
    pub fn set_force_full_scene_animation_recollect(&mut self, enabled: bool) {
        self.force_full_scene_animation_recollect = enabled;
    }

    /// Benchmark-only A/B control for the former one-context-per-property resolver.
    #[allow(dead_code)]
    pub fn set_force_individual_reactive_resolve(&mut self, enabled: bool) {
        self.force_individual_reactive_resolve = enabled;
    }

    /// Benchmark-only A/B control for texture-mask tint property resolution.
    #[allow(dead_code)]
    pub fn set_force_legacy_texture_mask_tint_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_texture_mask_tint_reactive_resolve = enabled;
    }

    /// Benchmark-only A/B control for widget shadows that baked visual opacity into RGBA pixels.
    #[allow(dead_code)]
    pub fn set_force_legacy_widget_shadow_opacity(&mut self, enabled: bool) {
        self.force_legacy_widget_shadow_opacity = enabled;
    }

    /// Benchmark-only A/B control for canvas shadows that baked visual opacity into RGBA pixels.
    #[allow(dead_code)]
    pub fn set_force_legacy_canvas_shadow_opacity(&mut self, enabled: bool) {
        self.force_legacy_canvas_shadow_opacity = enabled;
    }

    /// Benchmark-only A/B control for TextColor property resolution.
    #[allow(dead_code)]
    pub fn set_force_legacy_text_color_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_text_color_reactive_resolve = enabled;
    }

    /// Benchmark-only A/B control for plain-container Background property resolution.
    #[allow(dead_code)]
    pub fn set_force_legacy_background_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_background_reactive_resolve = enabled;
    }

    /// Benchmark-only A/B control for plain-container BackgroundBrush property resolution.
    #[allow(dead_code)]
    pub fn set_force_legacy_background_brush_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_background_brush_reactive_resolve = enabled;
    }

    /// Benchmark-only A/B control for plain-container BackgroundBlur property resolution.
    #[allow(dead_code)]
    pub fn set_force_legacy_background_blur_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_background_blur_reactive_resolve = enabled;
    }

    /// Benchmark-only A/B control for strict plain-container Offset property resolution.
    #[allow(dead_code)]
    pub fn set_force_legacy_offset_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_offset_reactive_resolve = enabled;
    }

    /// Benchmark-only A/B control for strict plain-container Scale property resolution.
    #[allow(dead_code)]
    pub fn set_force_legacy_scale_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_scale_reactive_resolve = enabled;
    }

    /// Benchmark-only A/B control for plain-container BorderColor property resolution.
    #[allow(dead_code)]
    pub fn set_force_legacy_border_color_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_border_color_reactive_resolve = enabled;
    }

    /// Benchmark-only A/B control for plain-container BorderRadius property resolution.
    #[allow(dead_code)]
    pub fn set_force_legacy_border_radius_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_border_radius_reactive_resolve = enabled;
    }

    #[allow(dead_code)]
    pub fn set_force_legacy_border_width_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_border_width_reactive_resolve = enabled;
    }

    #[allow(dead_code)]
    pub fn set_force_legacy_text_opacity_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_text_opacity_reactive_resolve = enabled;
    }

    #[allow(dead_code)]
    pub fn set_force_legacy_container_opacity_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_container_opacity_reactive_resolve = enabled;
    }

    /// Benchmark-only A/B control for ProgressValue property resolution.
    #[allow(dead_code)]
    pub fn set_force_legacy_progress_value_reactive_resolve(&mut self, enabled: bool) {
        self.force_legacy_progress_value_reactive_resolve = enabled;
    }

    #[allow(dead_code)]
    pub fn resolve_all_plain_container_backgrounds_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<(Rect, Color)>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(
                        resolved.kind,
                        super::ResolvedWidgetKind::Container { .. }
                            | super::ResolvedWidgetKind::Virtual { .. }
                    ) && resolved.background.is_some()
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::Background))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_background_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::ShapeFillColor { rect, color, .. }) => {
                Some((rect, color))
            }
            _ => None,
        })
        .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_plain_container_background_blurs_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<(BackdropBlurPrimitive, Option<bool>)>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::Container { .. })
                        && matches!(&resolved.visual.background_blur, Value::Signal(_))
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::BackgroundBlur))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_background_blur_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::BackdropBlur {
                primitive,
                container_occluder,
            }) => Some((primitive, container_occluder)),
            _ => None,
        })
        .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_plain_container_background_brushes_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<BrushPrimitive>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::Container { .. })
                        && matches!(&resolved.visual.background_brush, Some(Value::Signal(_)))
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::BackgroundBrush))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_background_brush_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::Brush(primitive)) => Some(primitive),
            _ => None,
        })
        .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_plain_container_offsets_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<ContainerOffsetResolveSnapshot>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::Container { .. })
                        && matches!(&resolved.visual.offset, Value::Signal(_))
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::Offset))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_offset_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::Offset {
                background,
                border,
                backdrop_blur,
                brush,
                texture,
                container_occluder,
            }) => Some(ContainerOffsetResolveSnapshot {
                background,
                border,
                backdrop_blur,
                brush,
                has_texture: texture.is_some(),
                container_occluder,
            }),
            _ => None,
        })
        .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_plain_container_scales_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<ContainerScaleResolveSnapshot>> {
        self.resolve_all_plain_container_scales_with_reduced_motion_for_benchmark(
            tree, now, legacy, false,
        )
    }

    #[allow(dead_code)]
    pub fn resolve_all_plain_container_scales_with_reduced_motion_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
        reduced_motion: bool,
    ) -> Vec<Option<ContainerScaleResolveSnapshot>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::Container { .. })
                        && matches!(&resolved.visual.scale, Value::Signal(_))
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::Scale))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_scale_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                reduced_motion,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::Scale {
                background,
                border,
                backdrop_blur,
                brush,
                texture,
                container_occluder,
            }) => Some(ContainerScaleResolveSnapshot {
                background,
                border,
                backdrop_blur,
                brush,
                has_texture: texture.is_some(),
                container_occluder,
            }),
            _ => None,
        })
        .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_plain_container_border_colors_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<(Rect, f32, Color)>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::Container { .. })
                        && resolved.visual.border_color.is_some()
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::BorderColor))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_border_color_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::ShapeStrokeColor {
                rect,
                stroke_width,
                color,
            }) => Some((rect, stroke_width, color)),
            _ => None,
        })
        .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_plain_container_border_radii_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<(Option<(Rect, f32)>, Option<(Rect, f32)>)>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::Container { .. })
                        && resolved.visual.border_radius.is_some()
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::BorderRadius))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_border_radius_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::BorderRadius { background, border }) => Some((
                background.map(|(rect, _, radius)| (rect, radius)),
                border.map(|(rect, _, _, radius)| (rect, radius)),
            )),
            _ => None,
        })
        .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_plain_container_border_widths_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<(Option<(Rect, f32)>, Option<(Rect, f32)>)>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::Container { .. })
                        && resolved.visual.border_width.is_some()
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::BorderWidth))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_border_width_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::BorderWidth {
                background, border, ..
            }) => Some((
                background.map(|(rect, _, radius)| (rect, radius)),
                border.map(|(rect, _, width)| (rect, width)),
            )),
            _ => None,
        })
        .collect()
    }

    /// Full-layout/full-scene control for the 1k BorderColor CPU equivalence benchmark.
    #[allow(dead_code)]
    pub fn full_recollect_plain_container_border_colors_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
    ) -> Vec<Option<(Rect, f32, Color)>> {
        self.invalidate_all();
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::Container { .. })
                        && resolved.visual.border_color.is_some()
                })
            })
            .collect();
        targets.sort_unstable_by_key(|widget_id| widget_id.raw());
        targets
            .into_iter()
            .map(|widget_id| {
                self.cached_scene_chunks
                    .get(&widget_id)?
                    .scene
                    .shapes
                    .iter()
                    .find(|shape| shape.stroke_width > 0.0)
                    .map(|shape| (shape.rect, shape.stroke_width, shape.color))
            })
            .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_progress_values_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<ProgressValueResolveSnapshot>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::ProgressBar { .. })
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::ProgressValue))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_progress_value_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::ProgressFill {
                track_rect,
                fill_rect,
                track_color,
                fill_color,
                label,
            }) => Some(ProgressValueResolveSnapshot {
                track_rect,
                fill_rect,
                track_color,
                fill_color,
                label: label.map(|label| ProgressValueLabelSnapshot {
                    frame: label.frame,
                    content: label.content.to_string(),
                    font_family: label.font_family.map(|family| family.to_string()),
                }),
            }),
            _ => None,
        })
        .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_slider_values_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<SliderValueResolveSnapshot>> {
        self.resolve_all_slider_values_for_benchmark_with_states(
            tree,
            now,
            legacy,
            &WidgetStateMap::default(),
        )
    }

    fn resolve_all_slider_values_for_benchmark_with_states(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
        widget_states: &WidgetStateMap,
    ) -> Vec<Option<SliderValueResolveSnapshot>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(
                        &resolved.kind,
                        super::ResolvedWidgetKind::Slider {
                            value: Value::Signal(_),
                            ..
                        }
                    )
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::SliderValue))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_slider_value_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::SliderValue {
                value,
                track_rect,
                active_rect,
                thumb_rect,
                track_color,
                active_track_color,
                thumb_color,
                thumb_border,
                label,
                ..
            }) => Some(SliderValueResolveSnapshot {
                value,
                track_rect,
                active_rect,
                thumb_rect,
                track_color,
                active_track_color,
                thumb_color,
                thumb_border,
                label: label.map(|label| ProgressValueLabelSnapshot {
                    frame: label.frame,
                    content: label.content.to_string(),
                    font_family: label.font_family.map(|family| family.to_string()),
                }),
            }),
            _ => None,
        })
        .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_text_colors_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<Color>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::Text { .. })
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::TextColor))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_text_color_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::TextColor { color }) => Some(color),
            _ => None,
        })
        .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_plain_text_opacities_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<Color>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::Text { .. })
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::Opacity))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_text_opacity_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::Opacity { text, .. }) => text,
            _ => None,
        })
        .collect()
    }

    #[allow(dead_code)]
    pub fn resolve_all_plain_container_opacities_for_benchmark(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Vec<Option<(Color, Option<Color>, Option<bool>)>> {
        self.sync_cache(tree, now, true);
        let Some(layout) = self.cached_layout.as_ref() else {
            return Vec::new();
        };
        let mut targets: Vec<_> = layout
            .paths
            .keys()
            .copied()
            .filter(|widget_id| {
                layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                    matches!(resolved.kind, super::ResolvedWidgetKind::Container { .. })
                        && matches!(resolved.visual.opacity, Value::Signal(_))
                })
            })
            .map(|widget_id| (widget_id, PropertySlot::Opacity))
            .collect();
        targets.sort_unstable_by_key(|(widget_id, _)| widget_id.raw());
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_container_opacity_reactive_resolve(legacy, || {
            layout.resolve_reactive_scene_property_values(
                &targets,
                &self.cached_visual_contexts,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &empty_widget_states,
                &empty_select_states,
                &self.scroll_offsets,
                &empty_virtual_states,
                self.viewport,
                now,
                &self.style_sheet,
            )
        })
        .into_iter()
        .map(|value| match value {
            Some(ReactiveScenePropertyValue::Opacity {
                background: Some((_, background)),
                border,
                container_occluder,
                ..
            }) => Some((
                background,
                border.map(|(_, _, color)| color),
                container_occluder,
            )),
            _ => None,
        })
        .collect()
    }

    #[cfg(test)]
    fn resolve_first_image_texture_mask_tint_for_test(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Option<ReactiveScenePropertyValue> {
        self.sync_cache(tree, now, true);
        let layout = self.cached_layout.as_ref()?;
        let widget_id = layout.paths.keys().copied().find(|widget_id| {
            layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                matches!(resolved.kind, super::ResolvedWidgetKind::Image { .. })
            })
        })?;
        let visual_contexts = &self.cached_visual_contexts;
        let font_manager = &self.font_manager;
        let theme = &self.theme;
        let media = &self.media;
        let animations = &mut self.animations;
        let scroll_offsets = &self.scroll_offsets;
        let style_sheet = &self.style_sheet;
        let viewport = self.viewport;
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_texture_mask_tint_reactive_resolve(legacy, || {
            layout
                .resolve_reactive_scene_property_values(
                    &[(widget_id, PropertySlot::TextureMaskTint)],
                    visual_contexts,
                    font_manager,
                    theme,
                    media,
                    animations,
                    false,
                    None,
                    None,
                    &empty_widget_states,
                    &empty_select_states,
                    scroll_offsets,
                    &empty_virtual_states,
                    viewport,
                    now,
                    style_sheet,
                )
                .into_iter()
                .next()
                .flatten()
        })
    }

    #[cfg(test)]
    fn resolve_first_text_content_for_test(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Option<ReactiveScenePropertyValue> {
        self.sync_cache(tree, now, true);
        let layout = self.cached_layout.as_ref()?;
        let widget_id = layout.paths.keys().copied().find(|widget_id| {
            layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                matches!(
                    resolved.kind,
                    super::ResolvedWidgetKind::Text { .. }
                        | super::ResolvedWidgetKind::TextEditor { .. }
                )
            })
        })?;
        let visual_contexts = &self.cached_visual_contexts;
        let font_manager = &self.font_manager;
        let theme = &self.theme;
        let media = &self.media;
        let animations = &mut self.animations;
        let scroll_offsets = &self.scroll_offsets;
        let style_sheet = &self.style_sheet;
        let viewport = self.viewport;
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_text_content_reactive_resolve(legacy, || {
            layout
                .resolve_reactive_scene_property_values(
                    &[(widget_id, PropertySlot::TextContent)],
                    visual_contexts,
                    font_manager,
                    theme,
                    media,
                    animations,
                    false,
                    None,
                    None,
                    &empty_widget_states,
                    &empty_select_states,
                    scroll_offsets,
                    &empty_virtual_states,
                    viewport,
                    now,
                    style_sheet,
                )
                .into_iter()
                .next()
                .flatten()
        })
    }

    #[cfg(test)]
    fn resolve_first_text_color_for_test(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Option<ReactiveScenePropertyValue> {
        self.sync_cache(tree, now, true);
        let layout = self.cached_layout.as_ref()?;
        let widget_id = layout.paths.keys().copied().find(|widget_id| {
            layout.resolved_widget(*widget_id).is_some_and(|resolved| {
                matches!(
                    resolved.kind,
                    super::ResolvedWidgetKind::Text { .. }
                        | super::ResolvedWidgetKind::TextEditor { .. }
                )
            })
        })?;
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_text_color_reactive_resolve(legacy, || {
            layout
                .resolve_reactive_scene_property_values(
                    &[(widget_id, PropertySlot::TextColor)],
                    &self.cached_visual_contexts,
                    &self.font_manager,
                    &self.theme,
                    &self.media,
                    &mut self.animations,
                    false,
                    None,
                    None,
                    &empty_widget_states,
                    &empty_select_states,
                    &self.scroll_offsets,
                    &empty_virtual_states,
                    self.viewport,
                    now,
                    &self.style_sheet,
                )
                .into_iter()
                .next()
                .flatten()
        })
    }

    #[cfg(test)]
    fn resolve_first_background_for_test(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
        legacy: bool,
    ) -> Option<ReactiveScenePropertyValue> {
        self.sync_cache(tree, now, true);
        let layout = self.cached_layout.as_ref()?;
        let widget_id = layout.paths.keys().copied().find(|widget_id| {
            layout
                .resolved_widget(*widget_id)
                .is_some_and(|resolved| resolved.background.is_some())
        })?;
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_background_reactive_resolve(legacy, || {
            layout
                .resolve_reactive_scene_property_values(
                    &[(widget_id, PropertySlot::Background)],
                    &self.cached_visual_contexts,
                    &self.font_manager,
                    &self.theme,
                    &self.media,
                    &mut self.animations,
                    false,
                    None,
                    None,
                    &empty_widget_states,
                    &empty_select_states,
                    &self.scroll_offsets,
                    &empty_virtual_states,
                    self.viewport,
                    now,
                    &self.style_sheet,
                )
                .into_iter()
                .next()
                .flatten()
        })
    }

    #[allow(dead_code)]
    pub fn headless_output_rgba(&self) -> Result<Vec<u8>, String> {
        self.gpu_renderer
            .as_ref()
            .ok_or_else(|| "headless GPU renderer is not initialized".to_string())?
            .read_output_rgba()
            .map_err(|error| error.to_string())
    }

    #[allow(dead_code)]
    pub fn cached_texture_scene_stats(&self) -> Option<GpuTextureSceneStats> {
        let textures = &self.cached_scene.as_ref()?.scene.textures;
        let unique_texture_ids = textures
            .iter()
            .map(|texture| texture.texture.id())
            .collect::<HashSet<_>>()
            .len();
        let mut clips = Vec::new();
        for texture in textures {
            if !clips.contains(&texture.clip_rect) {
                clips.push(texture.clip_rect);
            }
        }
        Some(GpuTextureSceneStats {
            texture_commands: textures.len(),
            unique_texture_ids,
            unique_clip_rects: clips.len(),
        })
    }

    #[allow(dead_code)]
    pub fn cached_texture_retained_state(&self) -> Option<GpuTextureRetainedState> {
        let scene = &self.cached_scene.as_ref()?.scene;
        let mut texture_ids = Vec::with_capacity(scene.textures.len());
        let mut texture_revisions = Vec::with_capacity(scene.textures.len());
        let mut frames = Vec::with_capacity(scene.textures.len());
        let mut opacities = Vec::with_capacity(scene.textures.len());
        let mut media_key_fingerprints = Vec::with_capacity(scene.textures.len());
        let mut media_layout_fingerprints = Vec::with_capacity(scene.textures.len());
        let mut mask_tints = Vec::with_capacity(scene.textures.len());
        for texture in &scene.textures {
            texture_ids.push(texture.texture.id());
            texture_revisions.push(texture.texture.revision());
            frames.push(texture.frame);
            opacities.push(texture.opacity);
            media_key_fingerprints.push(texture.media_key.as_ref().map(|key| {
                let mut hasher = DefaultHasher::new();
                key.hash(&mut hasher);
                hasher.finish()
            }));
            media_layout_fingerprints.push(texture.media_layout.map(|layout| {
                let mut hasher = DefaultHasher::new();
                for value in [
                    layout.content_frame.x,
                    layout.content_frame.y,
                    layout.content_frame.width,
                    layout.content_frame.height,
                ] {
                    value.get().to_bits().hash(&mut hasher);
                }
                layout.fit.hash(&mut hasher);
                layout.scale_factor.to_bits().hash(&mut hasher);
                hasher.finish()
            }));
            mask_tints.push(texture.mask_tint);
        }
        Some(GpuTextureRetainedState {
            texture_ids,
            texture_revisions,
            frames,
            opacities,
            media_key_fingerprints,
            media_layout_fingerprints,
            mask_tints,
            prepare_cache_serial: scene.prepare_cache_serial(),
            cache_liveness_dirty: scene.cache_liveness_dirty(),
            dirty_draw_range_count: scene.dirty_draw_ranges().len(),
        })
    }

    #[allow(dead_code)]
    pub fn cached_text_primitives(&self) -> Option<Vec<TextPrimitive>> {
        Some(
            self.cached_scene
                .as_ref()?
                .scene
                .texts
                .iter()
                .cloned()
                .collect(),
        )
    }

    /// Selects the former one-command-per-draw path for an isomorphic benchmark control.
    /// This is intentionally available only through `bench-support`.
    #[allow(dead_code)]
    pub fn set_headless_sprite_draw_batching(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_sprite_draw_batching(enabled);
        true
    }

    /// Selects the former one-command-per-draw Rect/Brush path for an isomorphic benchmark
    /// control. Mesh remains unbatched because its clip bind group is command-local.
    #[allow(dead_code)]
    pub fn set_headless_primitive_draw_batching(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_primitive_draw_batching(enabled);
        true
    }

    #[allow(dead_code)]
    pub fn set_headless_transparent_shape_skip(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_transparent_shape_skip(enabled);
        true
    }

    #[allow(dead_code)]
    pub fn reset_headless_gpu_cache_liveness_stats(&mut self) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.reset_cache_liveness_stats();
        true
    }

    /// Restores the former policy where every dirty draw range forced full text/texture liveness
    /// scans, for an isomorphic benchmark control.
    #[allow(dead_code)]
    pub fn set_headless_gpu_cache_liveness_legacy_dirty_gate(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_cache_liveness_legacy_dirty_draw_gate(enabled);
        true
    }

    /// Selects whole-page stale atlas release or the former per-allocation coalescing control.
    #[allow(dead_code)]
    pub fn set_headless_text_atlas_whole_page_stale_release(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_text_atlas_whole_page_stale_release(enabled);
        true
    }

    #[allow(dead_code)]
    pub fn headless_gpu_liveness_duration(&self) -> Option<Duration> {
        Some(self.gpu_renderer.as_ref()?.last_render_profile().liveness)
    }

    #[allow(dead_code)]
    pub fn force_headless_gpu_cache_liveness_refresh(&mut self) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.force_cache_liveness_refresh();
        true
    }

    #[allow(dead_code)]
    pub fn set_headless_gpu_clean_frame_cache(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_clean_prepared_frame_cache(enabled);
        true
    }

    #[allow(dead_code)]
    pub fn clear_headless_text_gpu_cache(&mut self) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.clear_text_gpu_cache();
        true
    }

    /// Selects immediate per-text atlas writes as an isomorphic benchmark control.
    #[allow(dead_code)]
    pub fn set_headless_text_atlas_deferred_upload(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_text_atlas_deferred_upload(enabled);
        true
    }

    /// Selects the former RGBA paragraph-mask atlas as an isomorphic A/B control.
    #[allow(dead_code)]
    pub fn set_headless_text_r8_atlas(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_text_r8_atlas(enabled);
        true
    }

    #[allow(dead_code)]
    pub fn reset_headless_text_atlas_upload_stats(&mut self) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.reset_text_atlas_upload_stats();
        true
    }

    #[allow(dead_code)]
    pub fn reset_headless_text_blend_stats(&mut self) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.reset_text_blend_stats();
        true
    }

    /// Selects the former all-float per-pixel source-over loop as an isomorphic A/B control.
    #[allow(dead_code)]
    pub fn set_headless_text_blend_fast_path(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_text_blend_fast_path(enabled);
        true
    }

    /// Selects the former per-frame Swash cache reset as an isomorphic A/B control.
    #[allow(dead_code)]
    pub fn set_headless_glyph_raster_cache_retention(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_text_raster_cache_retention(enabled);
        true
    }

    #[allow(dead_code)]
    pub fn reset_headless_glyph_raster_cache_stats(&mut self) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.reset_text_raster_cache_stats();
        true
    }

    #[allow(dead_code)]
    pub fn reset_headless_text_gpu_cache_activity_stats(&mut self) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.reset_text_cache_activity_stats();
        true
    }

    /// Selects the former RGBA cache key, where top-level alpha forced a whole-text reraster.
    #[allow(dead_code)]
    pub fn set_headless_text_alpha_cache_normalization(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_text_alpha_cache_normalization(enabled);
        true
    }

    /// Selects the baked-RGBA paragraph texture as an isomorphic A/B control for mask text.
    #[allow(dead_code)]
    pub fn set_headless_text_mask_tint(&mut self, enabled: bool) -> bool {
        let Some(renderer) = self.gpu_renderer.as_mut() else {
            return false;
        };
        renderer.set_text_mask_tint(enabled);
        true
    }

    /// Benchmark-only control path: discard the retained per-container content-bounds cache
    /// while preserving the solved Taffy layout. This provides an apples-to-apples baseline
    /// for scene recollection without rebuilding layout.
    #[allow(dead_code)]
    pub fn clear_cached_content_bounds(&mut self) {
        if let Some(layout) = self.cached_layout.as_mut() {
            layout.layout_root.clear_cached_child_content_bounds();
        }
    }

    /// Keep the solved layout and content-bounds cache, but pin child collection to the
    /// conservative full-scan path for A/B benchmarks.
    #[allow(dead_code)]
    pub fn disable_cached_child_culling(&mut self) {
        if let Some(layout) = self.cached_layout.as_mut() {
            layout.layout_root.disable_cached_child_culling();
        }
    }

    #[allow(dead_code)]
    pub fn set_scroll_offset(&mut self, widget_id: WidgetId, offset: Point) {
        self.scroll_offsets.insert(widget_id, offset);
        self.clear_scene_cache();
    }

    #[allow(dead_code)]
    pub fn set_first_scroll_offset(
        &mut self,
        tree: &WidgetTree<()>,
        offset: Point,
        now: Instant,
    ) -> bool {
        self.sync_cache(tree, now, true);
        let Some(id) = self
            .cached_scene
            .as_ref()
            .and_then(|scene| scene.scroll_regions.first())
            .map(|region| region.id)
        else {
            return false;
        };
        self.set_scroll_offset(id, offset);
        true
    }

    /// 模拟滚动 / 动画帧的热路径：保留已缓存的布局，仅强制重新 collect 整个场景。
    /// 运行时在 `scroll_epoch` / `animation_epoch` 变化但 `layout_cache` 仍有效时
    /// 走这条路径（见 `scene_runtime::computed_scene`）。
    #[allow(dead_code)]
    pub fn recollect_scene_only(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
    ) -> WidgetBenchmarkStats {
        if self.cached_layout.is_none() {
            self.rebuild_layout(tree, now);
        }
        self.clear_scene_cache();
        self.rebuild_scene(tree, now);
        let computed = self
            .cached_scene
            .as_ref()
            .expect("benchmark scene cache should exist");
        WidgetBenchmarkStats {
            dependency_count: computed.dependencies.dependency_count(),
            has_global_dependency: computed.dependencies.has_global_dependency(),
            shape_count: computed.scene.shapes.len(),
            mesh_count: computed.scene.meshes.len(),
            mesh_vertex_count: computed
                .scene
                .meshes
                .iter()
                .map(|mesh| mesh.vertices.len())
                .sum(),
            text_count: computed.scene.texts.len(),
            texture_count: computed.scene.textures.len(),
            overlay_shape_count: computed.scene.overlay_shapes.len(),
            hit_region_count: computed.hit_regions.len() + computed.overlay_hit_regions.len(),
            scroll_region_count: computed.scroll_regions.len(),
        }
    }

    /// A/B baseline for collection snapshot costs. The scene/layout/cache work is identical,
    /// but child/overlay boundaries use the former full `ComputedScene::clone` snapshots.
    #[allow(dead_code)]
    pub fn recollect_scene_only_with_legacy_snapshots(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
    ) -> WidgetBenchmarkStats {
        super::resolved::with_legacy_scene_snapshots(|| self.recollect_scene_only(tree, now))
    }

    /// 单属性更新基准的入口：模拟「改一个深层叶子的视觉属性」走的子树 patch ——
    /// 重收集该叶子自己的 chunk，再沿祖先链 `recompose_scene_chunk` 向上合成到根
    /// （即运行时 `scene_subtree_patch` 的核心成本）。返回是否成功 patch（失败表示
    /// 走了回退，bench 不应把这种样本计入）。预期在 `single_property_update` bench 中与
    /// `recollect_scene_only`（整树重收集，对照上界）对比。
    #[allow(dead_code)]
    pub fn patch_single_deep_leaf_scene(&mut self, tree: &WidgetTree<()>, now: Instant) -> bool {
        if self.cached_scene.is_none() {
            self.sync_cache(tree, now, true);
        }
        let Some(layout) = self.cached_layout.as_ref() else {
            return false;
        };
        let Some(leaf) = Self::deepest_leaf_id(layout) else {
            return false;
        };
        self.patch_scene_roots(&[leaf], false, now)
    }

    /// Clone-based A/B control for `patch_single_deep_leaf_scene`.
    ///
    /// This runs identical subtree collection and ancestor traversal, changing only the ancestor
    /// materialization strategy. It intentionally remains benchmark-only so production callers
    /// cannot accidentally select the slower path.
    #[cfg(feature = "bench-support")]
    #[allow(dead_code)]
    pub fn patch_single_deep_leaf_scene_legacy_recompose(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
    ) -> bool {
        if self.cached_scene.is_none() {
            self.sync_cache(tree, now, true);
        }
        let Some(layout) = self.cached_layout.as_ref() else {
            return false;
        };
        let Some(leaf) = Self::deepest_leaf_id(layout) else {
            return false;
        };
        self.patch_scene_roots_with_strategy(&[leaf], false, true, now)
    }

    /// Patch several deepest leaves in one transaction, exercising shared and disjoint ancestor
    /// recomposition for the multi-root Criterion control.
    #[cfg(feature = "bench-support")]
    #[allow(dead_code)]
    pub fn patch_multiple_deep_leaf_scenes(
        &mut self,
        tree: &WidgetTree<()>,
        root_count: usize,
        now: Instant,
    ) -> bool {
        self.patch_multiple_deep_leaf_scenes_with_strategy(tree, root_count, now, false)
    }

    /// Clone-based A/B control for `patch_multiple_deep_leaf_scenes`.
    #[cfg(feature = "bench-support")]
    #[allow(dead_code)]
    pub fn patch_multiple_deep_leaf_scenes_legacy_recompose(
        &mut self,
        tree: &WidgetTree<()>,
        root_count: usize,
        now: Instant,
    ) -> bool {
        self.patch_multiple_deep_leaf_scenes_with_strategy(tree, root_count, now, true)
    }

    #[cfg(feature = "bench-support")]
    fn patch_multiple_deep_leaf_scenes_with_strategy(
        &mut self,
        tree: &WidgetTree<()>,
        root_count: usize,
        now: Instant,
        legacy_recompose: bool,
    ) -> bool {
        if self.cached_scene.is_none() {
            self.sync_cache(tree, now, true);
        }
        let Some(layout) = self.cached_layout.as_ref() else {
            return false;
        };
        let roots = Self::deepest_leaf_ids(layout, root_count);
        !roots.is_empty()
            && self.patch_scene_roots_with_strategy(&roots, false, legacy_recompose, now)
    }

    /// 找到布局里深度最大的叶子 widget id（无子节点）。用于把单属性更新的成本
    /// 集中在「最深 → 根」的最长祖先链上，复现 roadmap 描述的超线性 patch 场景。
    #[allow(dead_code)]
    fn deepest_leaf_id(layout: &ResolvedSceneLayout<()>) -> Option<WidgetId> {
        Self::deepest_leaf_ids(layout, 1).into_iter().next()
    }

    fn deepest_leaf_ids(layout: &ResolvedSceneLayout<()>, count: usize) -> Vec<WidgetId> {
        let ids = layout.all_widget_ids().collect::<Vec<_>>();
        let mut non_leaf_ids = HashSet::with_capacity(ids.len());
        for id in &ids {
            if let Some(parent) = layout.parent_of(*id) {
                non_leaf_ids.insert(parent);
            }
        }

        let mut leaves = ids
            .into_iter()
            .filter(|id| !non_leaf_ids.contains(id))
            .collect::<Vec<_>>();
        leaves.sort_by_key(|id| std::cmp::Reverse(layout.depth_of(*id)));
        leaves.truncate(count);
        leaves
    }

    /// Benchmark helper for "one local layout root changed" scenarios. It patches the
    /// parent of the deepest leaf, then updates the scene cache for that same root.
    #[allow(dead_code)]
    pub fn patch_parent_of_deepest_leaf_layout_and_scene(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
    ) -> bool {
        if self.cached_scene.is_none() {
            self.sync_cache(tree, now, true);
        }
        let Some(layout) = self.cached_layout.as_ref() else {
            return false;
        };
        let Some(leaf) = Self::deepest_leaf_id(layout) else {
            return false;
        };
        let root = layout.parent_of(leaf).unwrap_or(leaf);

        if !self.patch_layout_roots(&[root], now) {
            return false;
        }
        if !self.patch_scene_roots(&[root], false, now) {
            self.clear_scene_cache();
            return false;
        }
        true
    }

    #[allow(dead_code)]
    pub fn run_layout(&mut self, tree: &WidgetTree<()>, now: Instant) -> WidgetBenchmarkStats {
        self.sync_cache(tree, now, false);
        let layout = self
            .cached_layout
            .as_ref()
            .expect("benchmark layout cache should exist");

        WidgetBenchmarkStats {
            dependency_count: layout.dependencies().dependency_count(),
            has_global_dependency: layout.dependencies().has_global_dependency(),
            ..WidgetBenchmarkStats::default()
        }
    }

    /// A/B control for uniform VirtualList window planning. This rebuilds the
    /// same layout while selecting the former O(total_items) stripe-offset path.
    #[allow(dead_code)]
    pub fn run_layout_with_legacy_virtual_window_plan(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
    ) -> WidgetBenchmarkStats {
        self.invalidate_all();
        crate::ui::widget::r#virtual::legacy_uniform_window_plan::with_enabled(|| {
            self.run_layout(tree, now)
        })
    }

    /// A/B control for Tree's retained flattened-row snapshot. This rebuilds the
    /// same layout while selecting the former per-query full flattening path.
    #[allow(dead_code)]
    pub fn run_layout_with_legacy_tree_row_source(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
    ) -> WidgetBenchmarkStats {
        self.invalidate_all();
        crate::ui::widget::legacy_tree_row_source::with_enabled(|| self.run_layout(tree, now))
    }

    #[allow(dead_code)]
    pub fn run_layout_and_scene(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
    ) -> WidgetBenchmarkStats {
        self.sync_cache(tree, now, true);
        let computed = self
            .cached_scene
            .as_ref()
            .expect("benchmark scene cache should exist");

        WidgetBenchmarkStats {
            dependency_count: computed.dependencies.dependency_count(),
            has_global_dependency: computed.dependencies.has_global_dependency(),
            shape_count: computed.scene.shapes.len(),
            mesh_count: computed.scene.meshes.len(),
            mesh_vertex_count: computed
                .scene
                .meshes
                .iter()
                .map(|mesh| mesh.vertices.len())
                .sum(),
            text_count: computed.scene.texts.len(),
            texture_count: computed.scene.textures.len(),
            overlay_shape_count: computed.scene.overlay_shapes.len(),
            hit_region_count: computed.hit_regions.len() + computed.overlay_hit_regions.len(),
            scroll_region_count: computed.scroll_regions.len(),
        }
    }

    /// 在已缓存的真实 scene 上执行命中路径扫描，供 Criterion 测 runtime hit-region
    /// 热路径，而不把内部 `HitInteraction` 类型暴露到公共 bench API。
    #[allow(dead_code)]
    pub fn cached_hit_path_len(
        &mut self,
        tree: &WidgetTree<()>,
        point: Point,
        now: Instant,
    ) -> usize {
        self.sync_cache(tree, now, true);
        let Some(computed) = self.cached_scene.as_ref() else {
            return 0;
        };
        WidgetTree::hit_path_from_computed(computed, point).len()
    }

    /// Full-scan control for measuring the spatial hit-candidate index against the exact legacy
    /// traversal on the same cached scene.
    #[allow(dead_code)]
    pub fn cached_hit_path_len_full_scan(
        &mut self,
        tree: &WidgetTree<()>,
        point: Point,
        now: Instant,
    ) -> usize {
        self.sync_cache(tree, now, true);
        let Some(computed) = self.cached_scene.as_ref() else {
            return 0;
        };
        WidgetTree::hit_path_from_computed_full_scan(computed, point).len()
    }

    /// Number of retained visual-transform records in the cached production scene. Event-path
    /// benchmarks use this to prove that their transform fallback/control fixtures exercise the
    /// intended runtime branch instead of accidentally measuring the ordinary spatial index.
    #[allow(dead_code)]
    pub fn cached_transform_record_count(&mut self, tree: &WidgetTree<()>, now: Instant) -> usize {
        self.sync_cache(tree, now, true);
        self.cached_scene
            .as_ref()
            .map_or(0, |computed| computed.transform_records.len())
    }

    fn prepared_scroll_target_with_strategy(
        &self,
        point: Point,
        delta: Point,
        use_index: bool,
    ) -> (Option<WidgetId>, usize) {
        let mut candidate_visits = 0;
        let Some(computed) = self.cached_scene.as_ref() else {
            return (None, candidate_visits);
        };
        let regions = computed.scroll_regions.as_slice();
        let indexed = use_index
            .then(|| computed.scroll_region_lookup_index())
            .flatten();

        let mut visit = |index: usize| {
            candidate_visits += 1;
            let region = regions.get(index).copied()?;
            if region.visible_frame.is_empty() || !region.visible_frame.contains(point) {
                return None;
            }
            let max = region.max_offset();
            let current = self
                .scroll_offsets
                .get(&region.id)
                .copied()
                .unwrap_or(region.scroll_offset);
            let mut next = current;
            if region.can_scroll_x() {
                next.x = (next.x - delta.x).clamp(Dp::ZERO, max.x);
            }
            if region.can_scroll_y() {
                next.y = (next.y - delta.y).clamp(Dp::ZERO, max.y);
            }
            ((next.x - current.x).abs() > 0.01 || (next.y - current.y).abs() > 0.01)
                .then_some(region.id)
        };

        if let Some(index) = indexed {
            for region_index in index.scrollable_indices().iter().rev().copied() {
                if let Some(id) = visit(region_index) {
                    return (Some(id), candidate_visits);
                }
            }
        } else {
            for region_index in (0..regions.len()).rev() {
                if let Some(id) = visit(region_index) {
                    return (Some(id), candidate_visits);
                }
            }
        }
        (None, candidate_visits)
    }

    /// Inspect the already-collected lookup index without synchronizing layout, scene, or
    /// animation caches.
    #[allow(dead_code)]
    pub fn prepared_scroll_region_lookup_stats(&self) -> ScrollRegionLookupStats {
        let Some(computed) = self.cached_scene.as_ref() else {
            return ScrollRegionLookupStats::default();
        };
        let Some(index) = computed.scroll_region_lookup_index() else {
            return ScrollRegionLookupStats {
                region_count: computed.scroll_regions.len(),
                uses_index: false,
                scrollable_candidate_count: computed
                    .scroll_regions
                    .iter()
                    .copied()
                    .filter(|region| region.can_scroll_x() || region.can_scroll_y())
                    .count(),
                scrollbar_candidate_count: computed
                    .scroll_regions
                    .iter()
                    .filter(|region| {
                        region.horizontal_thumb.is_some() || region.vertical_thumb.is_some()
                    })
                    .count(),
            };
        };
        ScrollRegionLookupStats {
            region_count: computed.scroll_regions.len(),
            uses_index: true,
            scrollable_candidate_count: index.scrollable_indices().len(),
            scrollbar_candidate_count: index.scrollbar_indices().len(),
        }
    }

    /// Query a retained scene directly. This matches the production event path, where frame/cache
    /// synchronization has already completed before input targeting runs.
    #[allow(dead_code)]
    pub fn prepared_scroll_target(&self, point: Point, delta: Point) -> Option<WidgetId> {
        self.prepared_scroll_target_with_strategy(point, delta, true)
            .0
    }

    /// Full-scan control for [`Self::prepared_scroll_target`].
    #[allow(dead_code)]
    pub fn prepared_scroll_target_full_scan(&self, point: Point, delta: Point) -> Option<WidgetId> {
        self.prepared_scroll_target_with_strategy(point, delta, false)
            .0
    }

    /// Return candidate visits for a single retained lookup, outside Criterion's timed loop.
    #[allow(dead_code)]
    pub fn prepared_scroll_target_stats(
        &self,
        point: Point,
        delta: Point,
        use_index: bool,
    ) -> ScrollTargetLookupStats {
        let (target, candidate_visits) =
            self.prepared_scroll_target_with_strategy(point, delta, use_index);
        ScrollTargetLookupStats {
            found_target: target.is_some(),
            candidate_visits,
        }
    }

    fn cached_scroll_target_with_strategy(
        &mut self,
        tree: &WidgetTree<()>,
        point: Point,
        delta: Point,
        now: Instant,
        use_index: bool,
    ) -> Option<WidgetId> {
        self.sync_cache(tree, now, true);
        self.prepared_scroll_target_with_strategy(point, delta, use_index)
            .0
    }

    #[allow(dead_code)]
    pub fn cached_scroll_target(
        &mut self,
        tree: &WidgetTree<()>,
        point: Point,
        delta: Point,
        now: Instant,
    ) -> Option<WidgetId> {
        self.cached_scroll_target_with_strategy(tree, point, delta, now, true)
    }

    #[allow(dead_code)]
    pub fn cached_scroll_target_full_scan(
        &mut self,
        tree: &WidgetTree<()>,
        point: Point,
        delta: Point,
        now: Instant,
    ) -> Option<WidgetId> {
        self.cached_scroll_target_with_strategy(tree, point, delta, now, false)
    }

    fn sync_cache(&mut self, tree: &WidgetTree<()>, now: Instant, need_scene: bool) {
        let legacy_widget = self.force_legacy_widget_shadow_opacity;
        let legacy_canvas = self.force_legacy_canvas_shadow_opacity;
        super::with_legacy_widget_shadow_opacity(legacy_widget, || {
            crate::ui::widget::canvas::with_legacy_canvas_shadow_opacity(legacy_canvas, || {
                self.sync_cache_inner(tree, now, need_scene)
            })
        });
    }

    fn sync_cache_inner(&mut self, tree: &WidgetTree<()>, now: Instant, need_scene: bool) {
        let tree_ptr = tree as *const WidgetTree<()> as usize;
        if self.last_tree_ptr != Some(tree_ptr) || self.cached_layout.is_none() {
            self.rebuild_layout(tree, now);
            self.last_tree_ptr = Some(tree_ptr);
        } else {
            self.refresh_animation_caches(now, need_scene);
            if self.cached_layout.is_none() {
                self.rebuild_layout(tree, now);
            }
        }

        if need_scene && self.cached_scene.is_none() {
            self.rebuild_scene(tree, now);
        }
    }

    fn rebuild_layout(&mut self, tree: &WidgetTree<()>, now: Instant) {
        let layout = tree.build_scene_layout_at(
            &self.font_manager,
            &self.theme,
            &self.media,
            &mut self.animations,
            self.units,
            &HashMap::new(),
            &HashMap::new(),
            self.viewport,
            now,
        );
        self.cached_layout = Some(layout);
        self.clear_scene_cache();
    }

    fn rebuild_scene(&mut self, tree: &WidgetTree<()>, now: Instant) {
        let layout = self
            .cached_layout
            .as_ref()
            .expect("benchmark layout cache should exist");
        let collected = tree.collect_scene_cache_from_layout_with_focus_value_at(
            &self.font_manager,
            layout,
            &self.theme,
            &self.media,
            &mut self.animations,
            false,
            None,
            None,
            &Default::default(),
            &Default::default(),
            &self.scroll_offsets,
            self.viewport,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            now,
            &Default::default(),
            None,
            None,
            &self.style_sheet,
        );
        self.store_scene_cache(collected, now);
    }

    fn clear_scene_cache(&mut self) {
        self.cached_scene = None;
        self.cached_scene_chunks.clear();
        self.cached_chunk_parts.clear();
        self.cached_visual_contexts.clear();
        self.animation_reactive_slot_bindings.clear();
    }

    fn refresh_animation_caches(&mut self, now: Instant, need_scene: bool) {
        self.last_animation_frame_activity = AnimationFrameActivityStats::default();
        let refresh = self.animations.refresh(now);
        if !refresh.changed {
            return;
        }
        self.last_animation_frame_activity.refresh_changed = true;
        self.last_animation_frame_activity.layout_widget_count = refresh.layout_widget_ids.len();
        self.last_animation_frame_activity.scene_widget_count = refresh.scene_widget_ids.len();

        if self.force_full_layout_animation_rebuild && !refresh.layout_widget_ids.is_empty() {
            self.last_animation_frame_activity.layout_root_count = refresh.layout_widget_ids.len();
            self.last_animation_frame_activity.full_layout_rebuild = true;
            self.cached_layout = None;
            self.clear_scene_cache();
            return;
        }

        if self.force_full_scene_animation_recollect
            && refresh.layout_widget_ids.is_empty()
            && !refresh.scene_widget_ids.is_empty()
        {
            self.last_animation_frame_activity.scene_root_count = refresh.scene_widget_ids.len();
            self.last_animation_frame_activity.full_scene_recollect = true;
            self.clear_scene_cache();
            return;
        }

        if !refresh.layout_property_targets.is_empty()
            && refresh.scene_widget_ids.is_empty()
            && !refresh.has_unscoped_layout_changes
        {
            if let Some(scene_root_count) =
                self.try_update_animation_layout_slots(&refresh.layout_property_targets, now)
            {
                self.last_animation_frame_activity.layout_root_count = scene_root_count;
                self.last_animation_frame_activity.scene_root_count = scene_root_count;
                self.last_animation_frame_activity.layout_patch_succeeded = true;
                self.last_animation_frame_activity.scene_patch_succeeded = true;
                return;
            }
        }

        if refresh.layout_widget_ids.is_empty()
            && !refresh.scene_property_targets.is_empty()
            && !refresh.has_unscoped_scene_changes
            && self.try_patch_animation_reactive_slots(&refresh.scene_property_targets, now)
        {
            self.last_animation_frame_activity.scene_root_count = refresh.scene_widget_ids.len();
            self.last_animation_frame_activity.scene_patch_succeeded = true;
            self.last_animation_frame_activity
                .reactive_slot_write_succeeded = true;
            return;
        }

        let mut layout_roots = SmallVec::<[WidgetId; 16]>::new();
        let mut scene_roots = SmallVec::<[WidgetId; 16]>::new();
        if let Some(layout) = self.cached_layout.as_ref() {
            if !refresh.layout_widget_ids.is_empty() {
                layout_roots = layout.highest_roots_from_sorted_raw_ids(&refresh.layout_widget_ids);
            }
            if !refresh.scene_widget_ids.is_empty() || !refresh.layout_widget_ids.is_empty() {
                let mut affected_ids = SmallVec::<[u64; 16]>::with_capacity(
                    refresh.scene_widget_ids.len() + refresh.layout_widget_ids.len(),
                );
                affected_ids.extend_from_slice(&refresh.scene_widget_ids);
                affected_ids.extend_from_slice(&refresh.layout_widget_ids);
                affected_ids.sort_unstable();
                affected_ids.dedup();
                scene_roots = layout.highest_roots_from_sorted_raw_ids(&affected_ids);
            }
        }

        self.last_animation_frame_activity.layout_root_count = layout_roots.len();
        self.last_animation_frame_activity.scene_root_count = scene_roots.len();

        if !layout_roots.is_empty() {
            if !self.patch_layout_roots(&layout_roots, now) {
                self.last_animation_frame_activity.fell_back = true;
                self.cached_layout = None;
                self.clear_scene_cache();
                return;
            }
            self.last_animation_frame_activity.layout_patch_succeeded = true;
        }

        if !need_scene {
            if !layout_roots.is_empty() {
                self.clear_scene_cache();
            }
            return;
        }

        if !scene_roots.is_empty() {
            let resolve_roots = layout_roots.is_empty()
                && (refresh.scene_property_targets.is_empty()
                    || refresh.has_unscoped_scene_changes);
            if self.patch_scene_roots(&scene_roots, resolve_roots, now) {
                self.last_animation_frame_activity.scene_patch_succeeded = true;
            } else {
                self.last_animation_frame_activity.fell_back = true;
                self.clear_scene_cache();
            }
        }
    }

    fn patch_layout_roots(&mut self, roots: &[WidgetId], now: Instant) -> bool {
        let Some(layout) = self.cached_layout.as_mut() else {
            return false;
        };
        layout
            .patch_layout_roots(
                roots,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                self.viewport,
                now,
            )
            .is_ok()
    }

    fn patch_scene_roots(&mut self, roots: &[WidgetId], resolve_roots: bool, now: Instant) -> bool {
        self.patch_scene_roots_with_strategy(roots, resolve_roots, false, now)
    }

    fn patch_scene_roots_with_strategy(
        &mut self,
        roots: &[WidgetId],
        resolve_roots: bool,
        legacy_recompose: bool,
        now: Instant,
    ) -> bool {
        let Some(mut dependencies) = self
            .cached_scene
            .as_ref()
            .map(|computed| computed.dependencies.clone())
        else {
            return false;
        };
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();

        struct ScenePatch {
            old_ids: Vec<WidgetId>,
            cache: CollectedSceneCache<()>,
        }

        let (patches, ancestors, root_id) = {
            let Some(layout) = self.cached_layout.as_mut() else {
                return false;
            };
            if resolve_roots && !layout.patch_resolved_roots(roots, &self.theme) {
                return false;
            }

            let mut patches = Vec::with_capacity(roots.len());
            for root in roots {
                let old_ids = layout.subtree_widget_ids(*root);
                let Some(visual_context) = self.cached_visual_contexts.get(root).copied() else {
                    return false;
                };
                let Some(cache) = layout.collect_scene_cache_for_widget_with_focus_value(
                    *root,
                    &self.font_manager,
                    &self.theme,
                    &self.media,
                    &mut self.animations,
                    false,
                    visual_context,
                    None,
                    None,
                    &empty_widget_states,
                    &empty_select_states,
                    &self.scroll_offsets,
                    &empty_virtual_states,
                    self.viewport,
                    now,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    false,
                    None,
                    None,
                    &self.style_sheet,
                ) else {
                    return false;
                };
                patches.push(ScenePatch { old_ids, cache });
            }

            let mut ancestors = SmallVec::<[WidgetId; 32]>::new();
            for root in roots {
                let mut parent = layout.parent_of(*root);
                while let Some(current) = parent {
                    if !ancestors.contains(&current) {
                        ancestors.push(current);
                    }
                    parent = layout.parent_of(current);
                }
            }
            ancestors.sort_by_key(|widget_id| std::cmp::Reverse(layout.depth_of(*widget_id)));
            (patches, ancestors, layout.root_id())
        };

        if self.cached_scene.is_none() {
            return false;
        }

        let scene_owner_ids = patches
            .iter()
            .flat_map(|patch| patch.old_ids.iter())
            .map(|widget_id| widget_id.raw())
            .collect::<HashSet<_>>();
        dependencies.remove_widget_phase_owners(&scene_owner_ids, DependencyPhase::Scene);

        for patch in patches {
            dependencies.merge_from(&patch.cache.dependencies);
            let new_ids = patch.cache.chunks.keys().copied().collect::<HashSet<_>>();
            for old_id in &patch.old_ids {
                if !new_ids.contains(old_id) {
                    self.cached_scene_chunks.remove(old_id);
                    self.cached_chunk_parts.remove(old_id);
                    self.cached_visual_contexts.remove(old_id);
                }
            }
            self.cached_scene_chunks.extend(patch.cache.chunks);
            self.cached_chunk_parts.extend(patch.cache.chunk_parts);
            self.cached_visual_contexts
                .extend(patch.cache.visual_contexts);
        }

        let Some(layout) = self.cached_layout.as_ref() else {
            return false;
        };
        for ancestor in ancestors {
            let result = if legacy_recompose {
                layout.recompose_scene_chunk_legacy(
                    ancestor,
                    &self.cached_chunk_parts,
                    &mut self.cached_scene_chunks,
                )
            } else {
                layout.recompose_scene_chunk(
                    ancestor,
                    &self.cached_chunk_parts,
                    &mut self.cached_scene_chunks,
                )
            };
            if result.is_none() {
                return false;
            }
        }

        let Some(mut root_chunk) = self.cached_scene_chunks.get(&root_id).cloned() else {
            return false;
        };
        root_chunk.dependencies = dependencies;
        // This benchmark helper recomposes ancestors instead of using the production runtime's
        // direct-splice planner. Give the new root a fresh serial so an end-to-end GPU benchmark
        // cannot reuse stale prepared draws after layout geometry changed.
        root_chunk.assign_new_prepare_cache_serial();
        self.cached_scene = Some(root_chunk);
        self.rebuild_animation_reactive_slot_bindings(now);
        true
    }

    fn resolve_animation_reactive_slot_values_batch(
        &mut self,
        targets: &[(WidgetId, PropertySlot)],
        now: Instant,
    ) -> Option<Vec<Option<ReactiveScenePropertyValue>>> {
        let layout = self.cached_layout.as_ref()?;
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_virtual_states = HashMap::new();
        super::resolved::with_legacy_texture_mask_tint_reactive_resolve(
            self.force_legacy_texture_mask_tint_reactive_resolve,
            || {
                super::resolved::with_legacy_text_color_reactive_resolve(
                    self.force_legacy_text_color_reactive_resolve,
                    || {
                        super::resolved::with_legacy_text_opacity_reactive_resolve(
                            self.force_legacy_text_opacity_reactive_resolve,
                            || {
                                super::resolved::with_legacy_container_opacity_reactive_resolve(
                                    self.force_legacy_container_opacity_reactive_resolve,
                                    || {
                                        super::resolved::with_legacy_background_reactive_resolve(
                                            self.force_legacy_background_reactive_resolve,
                                            || {
                                                super::resolved::with_legacy_background_brush_reactive_resolve(
                                                    self.force_legacy_background_brush_reactive_resolve,
                                                    || {
                                                        super::resolved::with_legacy_background_blur_reactive_resolve(
                                                            self.force_legacy_background_blur_reactive_resolve,
                                                            || {
                                                                super::resolved::with_legacy_border_color_reactive_resolve(
                                                                    self.force_legacy_border_color_reactive_resolve,
                                                                    || {
                                                                        super::resolved::with_legacy_border_radius_reactive_resolve(
                                                                            self.force_legacy_border_radius_reactive_resolve,
                                                                            || {
                                                                                super::resolved::with_legacy_border_width_reactive_resolve(
                                                                                    self.force_legacy_border_width_reactive_resolve,
                                                                                    || {
                                                                                        super::resolved::with_legacy_progress_value_reactive_resolve(
                                                                                            self.force_legacy_progress_value_reactive_resolve,
                                                                                            || {
                                                                                                super::resolved::with_legacy_scale_reactive_resolve(
                                                                                                    self.force_legacy_scale_reactive_resolve,
                                                                                                    || {
                                                                                                        super::resolved::with_legacy_offset_reactive_resolve(
                                                                                                            self.force_legacy_offset_reactive_resolve,
                                                                                                            || {
                                                                                                                Some(layout.resolve_reactive_scene_property_values(
                                                                                                                    targets,
                                                                                                                    &self.cached_visual_contexts,
                                                                                                                    &self.font_manager,
                                                                                                                    &self.theme,
                                                                                                                    &self.media,
                                                                                                                    &mut self.animations,
                                                                                                                    false,
                                                                                                                    None,
                                                                                                                    None,
                                                                                                                    &empty_widget_states,
                                                                                                                    &empty_select_states,
                                                                                                                    &self.scroll_offsets,
                                                                                                                    &empty_virtual_states,
                                                                                                                    self.viewport,
                                                                                                                    now,
                                                                                                                    &self.style_sheet,
                                                                                                                ))
                                                                                                            },
                                                                                                        )
                                                                                                    },
                                                                                                )
                                                                                            },
                                                                                        )
                                                                                    },
                                                                                )
                                                                            },
                                                                        )
                                                                    },
                                                                )
                                                            },
                                                        )
                                                    },
                                                )
                                            },
                                        )
                                    },
                                )
                            },
                        )
                    },
                )
            },
        )
    }

    fn resolve_animation_reactive_slot_values(
        &mut self,
        targets: &[(WidgetId, PropertySlot)],
        now: Instant,
    ) -> Option<Vec<Option<ReactiveScenePropertyValue>>> {
        if !self.force_individual_reactive_resolve {
            return self.resolve_animation_reactive_slot_values_batch(targets, now);
        }

        let mut values = Vec::with_capacity(targets.len());
        for target in targets {
            let mut resolved = self
                .resolve_animation_reactive_slot_values_batch(std::slice::from_ref(target), now)?;
            values.push(resolved.pop().unwrap_or(None));
        }
        Some(values)
    }

    fn rebuild_animation_reactive_slot_bindings(&mut self, now: Instant) {
        let Some(computed) = self.cached_scene.as_ref() else {
            self.animation_reactive_slot_bindings.clear();
            return;
        };
        let owners = computed.dependencies.property_owners();
        let mut targets = owners
            .into_iter()
            .filter_map(|owner| {
                (owner.phase == DependencyPhase::Scene)
                    .then_some((WidgetId::from_raw(owner.widget_id), owner.property?))
            })
            .collect::<Vec<_>>();
        targets.sort_by_key(|(widget_id, property)| (widget_id.raw(), *property as u8));
        targets.dedup();

        let Some(values) = self.resolve_animation_reactive_slot_values(&targets, now) else {
            self.animation_reactive_slot_bindings.clear();
            return;
        };
        let mut bindings = HashMap::with_capacity(targets.len());
        for ((widget_id, property), value) in targets.into_iter().zip(values) {
            let Some(value) = value else {
                continue;
            };
            let Some(layout) = self.cached_layout.as_ref() else {
                break;
            };
            let Some(computed) = self.cached_scene.as_ref() else {
                break;
            };
            if let Some(binding) = build_reactive_slot_binding_for_scene(
                widget_id,
                value,
                layout,
                computed,
                &self.cached_scene_chunks,
                &self.cached_chunk_parts,
            ) {
                bindings.insert((widget_id, property), binding);
            }
        }
        self.animation_reactive_slot_bindings = bindings;
    }

    fn try_patch_animation_reactive_slots(
        &mut self,
        targets: &[(WidgetId, PropertySlot)],
        now: Instant,
    ) -> bool {
        let resolve_started = Instant::now();
        let Some(values) = self.resolve_animation_reactive_slot_values(targets, now) else {
            return false;
        };
        self.last_animation_frame_activity
            .reactive_slot_resolve_duration = resolve_started.elapsed();
        let mut plans = Vec::with_capacity(targets.len());
        for ((widget_id, property), value) in targets.iter().copied().zip(values) {
            let Some(binding) = self
                .animation_reactive_slot_bindings
                .get(&(widget_id, property))
                .cloned()
            else {
                return false;
            };
            let Some(value) = value else {
                return false;
            };
            let Some(patch) = binding.patch_for(value) else {
                return false;
            };
            plans.push((binding, patch));
        }

        let Some(computed) = self.cached_scene.as_mut() else {
            return false;
        };
        for (binding, patch) in plans {
            if !write_reactive_slot_patch_to_scene(
                computed,
                &mut self.cached_scene_chunks,
                &mut self.cached_chunk_parts,
                &binding,
                &patch,
            ) {
                return false;
            }
        }
        true
    }

    fn try_update_animation_layout_slots(
        &mut self,
        targets: &[(WidgetId, PropertySlot)],
        now: Instant,
    ) -> Option<usize> {
        let mut widget_ids = targets
            .iter()
            .map(|(widget_id, _)| *widget_id)
            .collect::<Vec<_>>();
        widget_ids.sort_by_key(|widget_id| widget_id.raw());
        widget_ids.dedup();
        if widget_ids.is_empty() {
            return None;
        }

        let mut scene_root_ids = {
            let layout = self.cached_layout.as_ref()?;
            targets
                .iter()
                .map(|(widget_id, property)| {
                    self.animation_layout_slot_scene_root(layout, *widget_id, *property)
                        .raw()
                })
                .collect::<Vec<_>>()
        };
        scene_root_ids.sort_unstable();
        scene_root_ids.dedup();
        let scene_roots = self
            .cached_layout
            .as_ref()?
            .highest_roots_from_sorted_raw_ids(&scene_root_ids);
        if scene_roots.is_empty() {
            return None;
        }
        if self
            .cached_layout
            .as_ref()
            .is_some_and(|layout| scene_roots.iter().any(|root| *root == layout.root_id()))
        {
            return None;
        }

        self.cached_layout
            .as_mut()?
            .update_layout_style_slots(
                &widget_ids,
                &self.font_manager,
                &self.theme,
                &self.media,
                &mut self.animations,
                self.viewport,
                now,
            )
            .ok()?;
        self.patch_scene_roots(&scene_roots, false, now)
            .then_some(scene_roots.len())
    }

    fn animation_layout_slot_scene_root(
        &self,
        layout: &ResolvedSceneLayout<()>,
        widget_id: WidgetId,
        property: PropertySlot,
    ) -> WidgetId {
        let mut current = match property {
            PropertySlot::Padding => widget_id,
            _ => layout.parent_of(widget_id).unwrap_or(widget_id),
        };
        loop {
            if current == layout.root_id()
                || layout.resolved_widget(current).is_some_and(|resolved| {
                    animation_layout_length_is_definite_px(resolved.layout.width.as_ref())
                        && animation_layout_length_is_definite_px(resolved.layout.height.as_ref())
                })
            {
                return current;
            }
            let Some(parent) = layout.parent_of(current) else {
                return current;
            };
            current = parent;
        }
    }

    fn store_scene_cache(&mut self, collected: CollectedSceneCache<()>, now: Instant) {
        self.cached_scene = Some(collected.computed);
        self.cached_scene_chunks = collected.chunks;
        self.cached_chunk_parts = collected.chunk_parts;
        self.cached_visual_contexts = collected.visual_contexts;
        self.rebuild_animation_reactive_slot_bindings(now);
    }
}

fn animation_layout_length_is_definite_px(value: Option<&Value<Length>>) -> bool {
    matches!(value.map(Value::resolve_untracked), Some(Length::Px(_)))
}

impl Default for WidgetBenchmarkContext {
    fn default() -> Self {
        Self::new()
    }
}

/// CPU-only A/B fixture for the stateful color of Select's built-in SVG chevron.
///
/// The legacy default baked a new animated tint into the SVG source on intermediate pressed
/// frames. The constant control keeps the chevron neutral while leaving the Select surface and
/// border stateful, so both fixtures exercise identical layout, text and hit-region work.
#[allow(dead_code)]
pub struct SelectArrowBenchmarkContext {
    tree: WidgetTree<()>,
    select_ids: Vec<WidgetId>,
    layout: ResolvedSceneLayout<()>,
    font_manager: FontManager,
    theme: Theme,
    media: MediaManager,
    animations: AnimationEngine,
    widget_states: WidgetStateMap,
    viewport: Rect,
    now: Instant,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectArrowBenchmarkStats {
    pub frames: usize,
    pub texture_commands: usize,
    pub unique_texture_ids: usize,
    pub cached_image_sources: usize,
}

#[allow(dead_code)]
impl SelectArrowBenchmarkContext {
    pub fn new(selects: usize, legacy_stateful_arrow: bool) -> Self {
        use crate::foundation::binding::InvalidationSignal;
        use crate::ui::layout::{Axis, Value};
        use crate::ui::theme::StateValue;
        use crate::ui::unit::dp;
        use crate::ui::widget::{Element, Flex, Select, SelectOption};

        let selects = selects.max(1);
        let theme = Theme::default();
        let muted = theme.colors.on_surface_muted;
        let pressed = theme.colors.on_surface;
        let disabled = theme.colors.on_disabled;
        let mut column = Flex::new(Axis::Vertical).width(dp(200.0));
        let mut select_ids = Vec::with_capacity(selects);
        for index in 0..selects {
            let select: Element<()> = Select::new(
                vec![SelectOption::new(index, format!("Option {index}"))],
                Some(index),
            )
            .style(move |style, _| {
                style.arrow = StateValue::interactive(
                    Value::Static(muted),
                    Value::Static(muted),
                    Value::Static(if legacy_stateful_arrow {
                        pressed
                    } else {
                        muted
                    }),
                    Value::Static(disabled),
                );
            })
            .size(dp(200.0), dp(40.0))
            .into();
            select_ids.push(select.id);
            column = column.child(select);
        }
        let tree = WidgetTree::new(column);
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = MediaManager::new(InvalidationSignal::new());
        let mut animations = AnimationEngine::default();
        let viewport = Rect::new(0.0, 0.0, 220.0, selects as f32 * 40.0);
        let now = Instant::now();
        let layout = tree.build_scene_layout_at(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            viewport,
            now,
        );
        let mut context = Self {
            tree,
            select_ids,
            layout,
            font_manager,
            theme,
            media,
            animations,
            widget_states: WidgetStateMap::default(),
            viewport,
            now,
        };
        let _ = context.collect_at(now);
        context.now += Duration::from_millis(1);
        context
    }

    fn collect_at(&mut self, now: Instant) -> CollectedSceneCache<()> {
        self.tree
            .collect_scene_cache_from_layout_with_focus_value_and_reduced_motion_at(
                &self.font_manager,
                &self.layout,
                &self.theme,
                &self.media,
                &mut self.animations,
                false,
                None,
                None,
                &self.widget_states,
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
                self.viewport,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                false,
                now,
                &HashMap::new(),
                &HashMap::new(),
                None,
                None,
                false,
                &crate::ui::widget::StyleSheet::default(),
            )
    }

    pub fn run_pressed_animation(&mut self) -> SelectArrowBenchmarkStats {
        for id in &self.select_ids {
            self.widget_states.set(
                *id,
                crate::ui::theme::WidgetState {
                    pressed: true,
                    ..Default::default()
                },
            );
        }
        let mut texture_commands = 0_usize;
        let mut texture_ids = HashSet::new();
        const FRAME_STEP: Duration = Duration::from_millis(15);
        const FRAMES: usize = 9;
        for frame in 0..FRAMES {
            let now = self.now + FRAME_STEP * frame as u32;
            let collected = self.collect_at(now);
            texture_commands += collected.computed.scene.textures.len();
            texture_ids.extend(
                collected
                    .computed
                    .scene
                    .textures
                    .iter()
                    .map(|texture| texture.texture.id()),
            );
        }
        self.now += FRAME_STEP * FRAMES as u32;
        SelectArrowBenchmarkStats {
            frames: FRAMES,
            texture_commands,
            unique_texture_ids: texture_ids.len(),
            cached_image_sources: self.media.cached_image_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::binding::{TextController, ViewModelContext};
    use crate::foundation::color::Color;
    #[cfg(feature = "collect-profile")]
    use crate::ui::layout::Insets;
    use crate::ui::layout::{Axis, Length, Overflow, Wrap};
    use crate::ui::unit::dp;
    use crate::ui::widget::{
        BackgroundBrush, BackgroundGradientStop, BackgroundLinearGradient,
        BackgroundRadialGradient, Button, Element, Flex, Icon, Input, Point, Popover, Portal,
        ProgressBar, Rect, ScrollView, Slider, Stack, Text,
    };

    #[test]
    fn select_arrow_benchmark_exposes_stateful_svg_texture_churn() {
        let mut legacy = SelectArrowBenchmarkContext::new(1, true);
        let mut constant = SelectArrowBenchmarkContext::new(1, false);
        let legacy = legacy.run_pressed_animation();
        let constant = constant.run_pressed_animation();

        assert!(legacy.unique_texture_ids > constant.unique_texture_ids);
        assert!(legacy.cached_image_sources > constant.cached_image_sources);
        assert_eq!(constant.unique_texture_ids, 1);
        assert_eq!(legacy.texture_commands, constant.texture_commands);
    }

    const MONOCHROME_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path fill="#000" d="M0 0h16v16H0z"/></svg>"##;

    #[test]
    fn monochrome_svg_gpu_mask_tint_changes_rgb_without_affecting_regular_svg() {
        let tint = Color::hexa(0xE11D48FF);
        let tinted_tree: WidgetTree<()> = WidgetTree::new(
            Icon::monochrome_svg(MONOCHROME_ICON_SVG)
                .size(dp(20.0))
                .style(move |style, _| style.color = tint.into()),
        );
        let regular_tree: WidgetTree<()> =
            WidgetTree::new(Icon::svg(MONOCHROME_ICON_SVG).size(dp(20.0)));
        let viewport = Rect::new(0.0, 0.0, 32.0, 32.0);
        let mut tinted = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut regular = WidgetBenchmarkContext::default().with_viewport(viewport);
        if tinted.initialize_headless_gpu().is_err() || regular.initialize_headless_gpu().is_err() {
            eprintln!("skipping monochrome SVG GPU readback test: no headless adapter");
            return;
        }
        tinted
            .render_cached_scene_to_headless_gpu(&tinted_tree, Instant::now())
            .expect("render tinted SVG icon");
        regular
            .render_cached_scene_to_headless_gpu(&regular_tree, Instant::now())
            .expect("render regular SVG icon");

        let tinted_pixels = tinted.headless_output_rgba().expect("tinted readback");
        let regular_pixels = regular.headless_output_rgba().expect("regular readback");
        let center = (10 * 32 + 10) * 4;
        let tinted_rgb = &tinted_pixels[center..center + 3];
        let regular_rgb = &regular_pixels[center..center + 3];
        assert!(tinted_rgb[0] > tinted_rgb[1] && tinted_rgb[0] > tinted_rgb[2]);
        assert!(regular_rgb.iter().all(|channel| *channel <= 2));
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn animated_monochrome_svg_tint_patch_matches_full_recollect() {
        let view_model = ViewModelContext::for_benchmarks();
        let color = view_model.state(Color::rgba(14, 165, 233, 224));
        let signal = color
            .signal()
            .animated(crate::animation::Transition::linear(Duration::from_millis(
                480,
            )));
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(96.0), dp(20.0))
            .gap(dp(4.0));
        for _ in 0..4 {
            let signal = signal.clone();
            row = row.child(
                Icon::monochrome_svg(MONOCHROME_ICON_SVG)
                    .size(dp(20.0))
                    .style(move |style, _| style.color = signal.clone().into()),
            );
        }
        let tree = WidgetTree::new(row);
        let viewport = Rect::new(0.0, 0.0, 100.0, 24.0);
        let start = Instant::now();
        let mut retained = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        retained.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);
        if retained.initialize_headless_gpu().is_err() || full.initialize_headless_gpu().is_err() {
            eprintln!("skipping animated SVG tint GPU equivalence test: no headless adapter");
            return;
        }
        retained
            .render_cached_scene_to_headless_gpu(&tree, start)
            .expect("warm retained SVG tint");
        full.render_cached_scene_to_headless_gpu(&tree, start)
            .expect("warm full-recollect SVG tint");

        color.set(Color::rgba(244, 63, 94, 176));
        let animation_start = start + Duration::from_millis(1);
        retained.invalidate_all();
        full.invalidate_all();
        retained.run_layout_and_scene(&tree, animation_start);
        full.run_layout_and_scene(&tree, animation_start);
        assert!(retained.has_active_property_animations());
        assert!(full.has_active_property_animations());
        full.set_force_full_scene_animation_recollect(true);
        let retained_before = retained
            .cached_texture_retained_state()
            .expect("retained SVG tint baseline");
        let full_before = full
            .cached_texture_retained_state()
            .expect("full SVG tint baseline");

        let frame = animation_start + Duration::from_millis(120);
        retained
            .render_cached_scene_to_headless_gpu(&tree, frame)
            .expect("render retained SVG tint animation");
        full.render_cached_scene_to_headless_gpu(&tree, frame)
            .expect("render full-recollect SVG tint animation");
        let retained_activity = retained.animation_frame_activity();
        assert_eq!(retained_activity.scene_widget_count, 4);
        assert!(retained_activity.scene_patch_succeeded);
        assert!(retained_activity.reactive_slot_write_succeeded);
        assert!(!retained_activity.full_scene_recollect);
        let full_activity = full.animation_frame_activity();
        assert_eq!(full_activity.scene_widget_count, 4);
        assert!(full_activity.full_scene_recollect);

        let retained_after = retained
            .cached_texture_retained_state()
            .expect("retained SVG tint result");
        let full_after = full
            .cached_texture_retained_state()
            .expect("full SVG tint result");
        assert_eq!(retained_after.texture_ids, retained_before.texture_ids);
        assert_eq!(
            retained_after.texture_revisions,
            retained_before.texture_revisions
        );
        assert_eq!(retained_after.frames, retained_before.frames);
        assert_eq!(
            retained_after.media_key_fingerprints,
            retained_before.media_key_fingerprints
        );
        assert_eq!(
            retained_after.media_layout_fingerprints,
            retained_before.media_layout_fingerprints
        );
        assert_eq!(
            retained_after.prepare_cache_serial,
            retained_before.prepare_cache_serial
        );
        assert!(!retained_after.cache_liveness_dirty);
        assert_ne!(retained_after.mask_tints, retained_before.mask_tints);
        assert_ne!(
            full_after.prepare_cache_serial,
            full_before.prepare_cache_serial
        );
        assert_eq!(retained_after.frames, full_after.frames);
        assert_eq!(retained_after.mask_tints, full_after.mask_tints);
        assert_eq!(
            retained
                .headless_output_rgba()
                .expect("retained SVG pixels"),
            full.headless_output_rgba().expect("full SVG pixels")
        );
    }

    #[test]
    fn texture_mask_tint_direct_resolve_matches_legacy_and_missing_resolver_falls_back() {
        let tint = Color::rgba(14, 165, 233, 224);
        let tinted_tree: WidgetTree<()> = WidgetTree::new(
            Icon::monochrome_svg(MONOCHROME_ICON_SVG)
                .size(dp(20.0))
                .style(move |style, _| style.color = tint.into()),
        );
        let regular_tree: WidgetTree<()> =
            WidgetTree::new(Icon::svg(MONOCHROME_ICON_SVG).size(dp(20.0)));
        let viewport = Rect::new(0.0, 0.0, 32.0, 32.0);
        let now = Instant::now();

        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_value =
            direct.resolve_first_image_texture_mask_tint_for_test(&tinted_tree, now, false);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value =
            legacy.resolve_first_image_texture_mask_tint_for_test(&tinted_tree, now, true);
        assert_eq!(direct_value, legacy_value);
        assert_eq!(
            direct_value,
            Some(ReactiveScenePropertyValue::TextureMaskTint { color: tint })
        );

        let mut direct_regular = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_missing = direct_regular.resolve_first_image_texture_mask_tint_for_test(
            &regular_tree,
            now,
            false,
        );
        let mut legacy_regular = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_missing =
            legacy_regular.resolve_first_image_texture_mask_tint_for_test(&regular_tree, now, true);
        assert_eq!(direct_missing, legacy_missing);
        assert_eq!(direct_missing, None);
    }

    #[test]
    fn text_color_direct_resolve_matches_legacy_with_runtime_surface_opacity() {
        let color = Color::rgba(14, 165, 233, 224);
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::<()>::new(Axis::Vertical).opacity(0.75).child(
                Text::new("Text color")
                    .size(dp(180.0), dp(20.0))
                    .style(move |style, _| {
                        style.color = color.into();
                        style.surface.opacity = 0.5.into();
                    }),
            ),
        );
        let viewport = Rect::new(0.0, 0.0, 220.0, 80.0);
        let now = Instant::now();

        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_value = direct.resolve_first_text_color_for_test(&tree, now, false);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value = legacy.resolve_first_text_color_for_test(&tree, now, true);
        assert_eq!(direct_value, legacy_value);
        assert!(matches!(
            direct_value,
            Some(ReactiveScenePropertyValue::TextColor { color: resolved })
                if resolved == color.with_alpha_factor(0.75 * 0.5)
        ));

        let input: WidgetTree<()> = WidgetTree::new(
            Input::new(TextController::new_legacy("TextColor fallback")).size(dp(180.0), dp(32.0)),
        );
        let mut direct_input = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_input_value = direct_input.resolve_first_text_color_for_test(&input, now, false);
        let mut legacy_input = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_input_value = legacy_input.resolve_first_text_color_for_test(&input, now, true);
        assert_eq!(direct_input_value, legacy_input_value);
        assert_eq!(direct_input_value, None);
    }

    #[test]
    fn plain_text_opacity_direct_resolve_matches_legacy_and_decorated_text_falls_back() {
        let view_model = ViewModelContext::for_benchmarks();
        let opacity = view_model.state(0.5_f32);
        let signal = opacity.signal();
        let color = Color::rgba(14, 165, 233, 224);
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::<()>::new(Axis::Vertical).opacity(0.75).child(
                Text::new("Text opacity")
                    .size(dp(180.0), dp(20.0))
                    .style(move |style, _| {
                        style.color = color.into();
                        style.surface.opacity = signal.clone().into();
                    }),
            ),
        );
        let viewport = Rect::new(0.0, 0.0, 220.0, 80.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, now);
        super::super::resolved::text_opacity_direct_probe::reset();
        let direct_value = direct.resolve_all_plain_text_opacities_for_benchmark(&tree, now, false);
        assert_eq!(
            super::super::resolved::text_opacity_direct_probe::attempts(),
            1
        );
        assert_eq!(super::super::resolved::text_opacity_direct_probe::hits(), 1);
        assert_eq!(
            super::super::resolved::text_opacity_direct_probe::prepared_fallbacks(),
            0
        );
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value = legacy.resolve_all_plain_text_opacities_for_benchmark(&tree, now, true);
        assert_eq!(direct_value, legacy_value);
        assert_eq!(
            direct_value,
            vec![Some(color.with_alpha_factor(0.75 * 0.5))]
        );

        let decorated_opacity = view_model.state(0.5_f32);
        let decorated_signal = decorated_opacity.signal();
        let decorated: WidgetTree<()> =
            WidgetTree::new(Text::new("Decorated").size(dp(180.0), dp(20.0)).style(
                move |style, _| {
                    style.color = color.into();
                    style.surface.opacity = decorated_signal.clone().into();
                    style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                },
            ));
        let mut direct_decorated = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct_decorated.set_force_legacy_text_opacity_reactive_resolve(true);
        direct_decorated.run_layout_and_scene(&decorated, now);
        direct_decorated.set_force_legacy_text_opacity_reactive_resolve(false);
        super::super::resolved::text_opacity_direct_probe::reset();
        let direct_fallback =
            direct_decorated.resolve_all_plain_text_opacities_for_benchmark(&decorated, now, false);
        assert_eq!(
            super::super::resolved::text_opacity_direct_probe::attempts(),
            1
        );
        assert_eq!(super::super::resolved::text_opacity_direct_probe::hits(), 0);
        assert_eq!(
            super::super::resolved::text_opacity_direct_probe::prepared_fallbacks(),
            1
        );
        let mut legacy_decorated = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_fallback =
            legacy_decorated.resolve_all_plain_text_opacities_for_benchmark(&decorated, now, true);
        assert_eq!(direct_fallback, legacy_fallback);
        assert_eq!(direct_fallback, vec![None]);
    }

    #[test]
    fn animated_plain_text_opacity_multiframe_direct_matches_legacy_and_full_recollect() {
        const TRANSITION: Duration = Duration::from_millis(480);

        let view_model = ViewModelContext::for_benchmarks();
        let opacity = view_model.state(0.8_f32);
        let signal = opacity
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let color = Color::rgba(56, 189, 248, 224);
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(196.0), dp(20.0))
            .gap(dp(1.0))
            .opacity(0.75);
        for index in 0..4 {
            let signal = signal.clone();
            row = row.child(
                Text::new(format!("T{index}"))
                    .size(dp(48.0), dp(20.0))
                    .style(move |style, _| {
                        style.color = color.into();
                        style.surface.opacity = signal.clone().into();
                    }),
            );
        }
        let tree = WidgetTree::new(row);
        let viewport = Rect::new(0.0, 0.0, 196.0, 20.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_text_opacity_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        full.set_force_legacy_text_opacity_reactive_resolve(true);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);
        assert_plain_container_animation_scene_equivalent(&direct, &legacy, &full, Duration::ZERO);

        opacity.set(0.2);
        let animation_start = start + Duration::from_millis(1);
        super::super::resolved::text_opacity_direct_probe::reset();
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);

        for (frame_index, (frame_offset, expected_opacity)) in [
            (Duration::from_millis(120), 0.65_f32),
            (Duration::from_millis(240), 0.5_f32),
            (Duration::from_millis(440), 0.25_f32),
            (TRANSITION, 0.2_f32),
        ]
        .into_iter()
        .enumerate()
        {
            let frame = animation_start + frame_offset;
            let direct_stats = direct.run_layout_and_scene(&tree, frame);
            let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
            let full_stats = full.run_layout_and_scene(&tree, frame);
            assert_eq!(
                direct_stats, legacy_stats,
                "stats differ at {frame_offset:?}"
            );
            assert_eq!(
                direct_stats, full_stats,
                "full stats differ at {frame_offset:?}"
            );

            let expected_attempts = (frame_index + 2) * 4;
            assert_eq!(
                super::super::resolved::text_opacity_direct_probe::attempts(),
                expected_attempts
            );
            assert_eq!(
                super::super::resolved::text_opacity_direct_probe::hits(),
                expected_attempts
            );
            assert_eq!(
                super::super::resolved::text_opacity_direct_probe::prepared_fallbacks(),
                0
            );

            let activity = direct.animation_frame_activity();
            assert!(activity.refresh_changed);
            assert_eq!(activity.scene_widget_count, 4);
            assert!(activity.reactive_slot_write_succeeded);
            assert!(activity.scene_patch_succeeded);
            assert!(!activity.fell_back);
            assert!(full.animation_frame_activity().full_scene_recollect);
            assert_plain_container_animation_scene_equivalent(
                &direct,
                &legacy,
                &full,
                frame_offset,
            );

            let texts = &direct
                .cached_scene
                .as_ref()
                .expect("direct scene")
                .scene
                .texts;
            assert_eq!(texts.len(), 4);
            for text in texts {
                assert_eq!(text.color, color.with_alpha_factor(0.75 * expected_opacity));
            }
        }
    }

    #[test]
    fn decorated_text_opacity_multiframe_sticky_fallback_matches_full_recollect() {
        const TRANSITION: Duration = Duration::from_millis(480);

        let view_model = ViewModelContext::for_benchmarks();
        let opacity = view_model.state(0.8_f32);
        let signal = opacity
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let shadow_signal = signal.clone();
        let brush_signal = signal.clone();
        let color = Color::rgba(56, 189, 248, 224);
        let brush_color = Color::rgba(99, 102, 241, 160);
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::new(Axis::Horizontal)
                .size(dp(113.0), dp(24.0))
                .gap(dp(1.0))
                .opacity(0.75)
                .child(
                    Text::new("Shadow")
                        .size(dp(56.0), dp(24.0))
                        .style(move |style, _| {
                            style.color = color.into();
                            style.surface.opacity = shadow_signal.clone().into();
                            style.surface.shadow = Some(
                                crate::ui::theme::Shadow {
                                    offset_x: dp(0.0),
                                    offset_y: dp(3.0),
                                    blur: dp(10.0),
                                    spread: dp(0.0),
                                    color: Color::rgba(15, 23, 42, 96),
                                }
                                .into(),
                            );
                        }),
                )
                .child(
                    Text::new("Brush")
                        .size(dp(56.0), dp(24.0))
                        .style(move |style, _| {
                            style.color = color.into();
                            style.surface.opacity = brush_signal.clone().into();
                            style.surface.background_brush =
                                Some(crate::ui::widget::BackgroundBrush::Solid(brush_color).into());
                        }),
                ),
        );
        let viewport = Rect::new(0.0, 0.0, 113.0, 32.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_text_opacity_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        full.set_force_legacy_text_opacity_reactive_resolve(true);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);
        assert_plain_container_animation_scene_equivalent(&direct, &legacy, &full, Duration::ZERO);

        opacity.set(0.2);
        let animation_start = start + Duration::from_millis(1);
        super::super::resolved::text_opacity_direct_probe::reset();
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);

        for (frame_index, frame_offset) in [
            Duration::from_millis(120),
            Duration::from_millis(240),
            Duration::from_millis(440),
            TRANSITION,
        ]
        .into_iter()
        .enumerate()
        {
            let frame = animation_start + frame_offset;
            let direct_stats = direct.run_layout_and_scene(&tree, frame);
            let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
            let full_stats = full.run_layout_and_scene(&tree, frame);
            assert_eq!(
                direct_stats, legacy_stats,
                "stats differ at {frame_offset:?}"
            );
            assert_eq!(
                direct_stats, full_stats,
                "full stats differ at {frame_offset:?}"
            );

            assert_eq!(
                super::super::resolved::text_opacity_direct_probe::attempts(),
                2,
                "decorated candidates should be attempted only at transition start"
            );
            assert_eq!(
                super::super::resolved::text_opacity_direct_probe::prepared_fallbacks(),
                2
            );
            assert_eq!(super::super::resolved::text_opacity_direct_probe::hits(), 0);

            let activity = direct.animation_frame_activity();
            assert!(
                activity.refresh_changed,
                "frame {frame_index} did not refresh"
            );
            assert_eq!(activity.scene_widget_count, 2);
            assert!(!activity.reactive_slot_write_succeeded);
            assert!(activity.scene_patch_succeeded);
            assert!(!activity.fell_back);
            assert!(full.animation_frame_activity().full_scene_recollect);
            assert_plain_container_animation_scene_equivalent(
                &direct,
                &legacy,
                &full,
                frame_offset,
            );

            let scene = &direct.cached_scene.as_ref().expect("direct scene").scene;
            assert_eq!(scene.texts.len(), 2, "text topology changed");
            assert_eq!(scene.brushes.len(), 1, "brush topology changed");
            assert_eq!(scene.textures.len(), 1, "shadow topology changed");
        }
    }

    #[test]
    fn plain_container_opacity_direct_resolve_matches_legacy_and_dynamic_surface_falls_back() {
        let view_model = ViewModelContext::for_benchmarks();
        let opacity = view_model.state(0.5_f32);
        let opacity_signal = opacity.signal();
        let background = Color::rgba(15, 23, 42, 255);
        let border = Color::rgba(14, 165, 233, 224);
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::<()>::new(Axis::Vertical).opacity(0.75).child(
                Stack::new()
                    .size(dp(80.0), dp(40.0))
                    .style(move |style, _| {
                        style.surface.background = Some(background.into());
                        style.surface.border_width = Some(dp(2.0).into());
                        style.surface.border_color = Some(border.into());
                        style.surface.border_radius = Some(dp(10.0).into());
                        style.surface.opacity = opacity_signal.clone().into();
                    }),
            ),
        );
        let viewport = Rect::new(0.0, 0.0, 120.0, 80.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, now);
        super::super::resolved::container_opacity_direct_probe::reset();
        let direct_value =
            direct.resolve_all_plain_container_opacities_for_benchmark(&tree, now, false);
        assert_eq!(
            super::super::resolved::container_opacity_direct_probe::attempts(),
            1
        );
        assert_eq!(
            super::super::resolved::container_opacity_direct_probe::hits(),
            1
        );
        assert_eq!(
            super::super::resolved::container_opacity_direct_probe::prepared_fallbacks(),
            0
        );
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value =
            legacy.resolve_all_plain_container_opacities_for_benchmark(&tree, now, true);
        assert_eq!(direct_value, legacy_value);
        assert_eq!(
            direct_value,
            vec![Some((
                background.with_alpha_factor(0.75 * 0.5),
                Some(border.with_alpha_factor(0.75 * 0.5)),
                Some(true),
            ))]
        );

        // A reactive background is deliberately outside the compact opacity resolver. It must
        // still fall through to the complete visual resolver and produce the same retained value.
        let fallback_opacity = view_model.state(0.5_f32);
        let fallback_opacity_signal = fallback_opacity.signal();
        let dynamic_background = view_model.state(Color::rgba(244, 63, 94, 210));
        let dynamic_background_signal = dynamic_background.signal();
        let dynamic_tree: WidgetTree<()> = WidgetTree::new(
            Stack::new()
                .size(dp(80.0), dp(40.0))
                .style(move |style, _| {
                    style.surface.background = Some(dynamic_background_signal.clone().into());
                    style.surface.opacity = fallback_opacity_signal.clone().into();
                }),
        );
        let mut direct_dynamic = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct_dynamic.set_force_legacy_container_opacity_reactive_resolve(true);
        direct_dynamic.run_layout_and_scene(&dynamic_tree, now);
        direct_dynamic.set_force_legacy_container_opacity_reactive_resolve(false);
        super::super::resolved::container_opacity_direct_probe::reset();
        let direct_fallback = direct_dynamic.resolve_all_plain_container_opacities_for_benchmark(
            &dynamic_tree,
            now,
            false,
        );
        assert_eq!(
            super::super::resolved::container_opacity_direct_probe::attempts(),
            1
        );
        assert_eq!(
            super::super::resolved::container_opacity_direct_probe::hits(),
            0
        );
        assert_eq!(
            super::super::resolved::container_opacity_direct_probe::prepared_fallbacks(),
            1
        );
        let mut legacy_dynamic = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_fallback = legacy_dynamic.resolve_all_plain_container_opacities_for_benchmark(
            &dynamic_tree,
            now,
            true,
        );
        assert_eq!(direct_fallback, legacy_fallback);
        assert_eq!(
            direct_fallback,
            vec![Some((
                Color::rgba(244, 63, 94, 210).with_alpha_factor(0.5),
                None,
                Some(true),
            ))]
        );
    }

    #[test]
    fn plain_container_background_direct_resolve_matches_legacy_and_complex_surface_falls_back() {
        let view_model = ViewModelContext::for_benchmarks();
        let background = view_model.state(Color::rgba(14, 165, 233, 224));
        let signal = background.signal();
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::<()>::new(Axis::Vertical).opacity(0.75).child(
                Stack::new()
                    .size(dp(80.0), dp(40.0))
                    .style(move |style, _| {
                        style.surface.background = Some(signal.clone().into());
                        style.surface.border_width = Some(dp(2.0).into());
                    }),
            ),
        );
        let viewport = Rect::new(0.0, 0.0, 120.0, 80.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_value = direct.resolve_first_background_for_test(&tree, now, false);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value = legacy.resolve_first_background_for_test(&tree, now, true);
        assert_eq!(direct_value, legacy_value);
        assert!(matches!(
            direct_value,
            Some(ReactiveScenePropertyValue::ShapeFillColor {
                color,
                container_occluder: Some(true),
                ..
            }) if color == Color::rgba(14, 165, 233, 224).with_alpha_factor(0.75)
        ));

        let transparent = view_model.state(Color::TRANSPARENT);
        let transparent_signal = transparent.signal();
        let transparent_tree: WidgetTree<()> = WidgetTree::new(
            Stack::new()
                .size(dp(80.0), dp(40.0))
                .style(move |style, _| {
                    style.surface.background = Some(transparent_signal.clone().into());
                }),
        );
        let mut direct_transparent = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_transparent_value =
            direct_transparent.resolve_first_background_for_test(&transparent_tree, now, false);
        let mut legacy_transparent = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_transparent_value =
            legacy_transparent.resolve_first_background_for_test(&transparent_tree, now, true);
        assert_eq!(direct_transparent_value, legacy_transparent_value);
        assert!(matches!(
            direct_transparent_value,
            Some(ReactiveScenePropertyValue::ShapeFillColor {
                container_occluder: Some(false),
                color,
                ..
            }) if color == Color::TRANSPARENT
        ));

        let focusable_background = view_model.state(Color::rgba(14, 165, 233, 224));
        let focusable_signal = focusable_background.signal();
        let focusable_tree: WidgetTree<()> =
            WidgetTree::new(Stack::new().size(dp(80.0), dp(40.0)).focusable(true).style(
                move |style, _| {
                    style.surface.background = Some(focusable_signal.clone().into());
                },
            ));
        let mut direct_focusable = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_focusable_value =
            direct_focusable.resolve_first_background_for_test(&focusable_tree, now, false);
        let mut legacy_focusable = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_focusable_value =
            legacy_focusable.resolve_first_background_for_test(&focusable_tree, now, true);
        assert_eq!(direct_focusable_value, legacy_focusable_value);
        assert!(matches!(
            direct_focusable_value,
            Some(ReactiveScenePropertyValue::ShapeFillColor {
                container_occluder: Some(false),
                ..
            })
        ));

        let background = view_model.state(Color::rgba(244, 63, 94, 210));
        let signal = background.signal();
        let complex: WidgetTree<()> = WidgetTree::new(Stack::new().size(dp(80.0), dp(40.0)).style(
            move |style, _| {
                style.surface.background = Some(signal.clone().into());
                style.surface.shadow = Some(crate::ui::theme::Shadow::default().into());
            },
        ));
        let mut direct_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_complex_value =
            direct_complex.resolve_first_background_for_test(&complex, now, false);
        let mut legacy_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_complex_value =
            legacy_complex.resolve_first_background_for_test(&complex, now, true);
        assert_eq!(direct_complex_value, legacy_complex_value);
        assert!(matches!(
            direct_complex_value,
            Some(ReactiveScenePropertyValue::ShapeFillColor { .. })
        ));
    }

    #[test]
    fn plain_container_background_brush_direct_matches_legacy_full_and_complex_falls_back() {
        let variants = [
            BackgroundBrush::Solid(Color::rgba(14, 165, 233, 224)),
            BackgroundBrush::LinearGradient(BackgroundLinearGradient::new(
                Point::new(dp(0.0), dp(0.0)),
                Point::new(dp(80.0), dp(0.0)),
                vec![
                    BackgroundGradientStop::new(0.0, Color::rgba(56, 189, 248, 224)),
                    BackgroundGradientStop::new(1.0, Color::rgba(249, 115, 22, 192)),
                ],
            )),
            BackgroundBrush::RadialGradient(BackgroundRadialGradient::new(
                Point::new(dp(40.0), dp(20.0)),
                dp(40.0),
                vec![
                    BackgroundGradientStop::new(0.0, Color::rgba(244, 63, 94, 208)),
                    BackgroundGradientStop::new(1.0, Color::rgba(30, 64, 175, 128)),
                ],
            )),
        ];
        let view_model = ViewModelContext::for_benchmarks();
        let brush = view_model.state(variants[0].clone());
        let signal = brush.signal();
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(dp(120.0), dp(80.0))
                .overflow(Overflow::Hidden)
                .opacity(0.75)
                .style(|style, _| style.surface.border_radius = Some(dp(12.0).into()))
                .child(
                    Stack::new()
                        .size(dp(80.0), dp(40.0))
                        .style(move |style, _| {
                            style.surface.background = Some(Color::rgba(15, 23, 42, 160).into());
                            style.surface.background_brush = Some(signal.clone().into());
                            style.surface.border_width = Some(dp(2.0).into());
                            style.surface.border_color =
                                Some(Color::rgba(148, 163, 184, 176).into());
                            style.surface.border_radius = Some(dp(10.0).into());
                        }),
                ),
        );
        let viewport = Rect::new(0.0, 0.0, 140.0, 100.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);

        for (frame_index, expected) in variants.into_iter().enumerate() {
            let frame = start + Duration::from_millis(frame_index as u64);
            if frame_index > 0 {
                brush.set(expected.clone());
                full.invalidate_all();
                full.run_layout_and_scene(&tree, frame);
            }

            super::super::resolved::background_brush_direct_probe::reset();
            let direct_value = direct
                .resolve_all_plain_container_background_brushes_for_benchmark(&tree, frame, false);
            assert_eq!(
                super::super::resolved::background_brush_direct_probe::hits(),
                1,
                "variant {frame_index} did not use the strict direct resolver"
            );
            let legacy_value = legacy
                .resolve_all_plain_container_background_brushes_for_benchmark(&tree, frame, true);
            assert_eq!(
                direct_value, legacy_value,
                "direct and legacy values differ for variant {frame_index}"
            );
            let full_value = full
                .cached_scene
                .as_ref()
                .expect("full brush scene")
                .scene
                .brushes
                .iter()
                .cloned()
                .map(Some)
                .collect::<Vec<_>>();
            assert_eq!(
                direct_value, full_value,
                "direct and full-recollected values differ for variant {frame_index}"
            );
            assert!(matches!(
                direct_value.as_slice(),
                [Some(primitive)]
                    if primitive.rect == Rect::new(2.0, 2.0, 76.0, 36.0)
                        && (primitive.corner_radius - 8.0).abs() <= f32::EPSILON
                        && primitive.clip_rect == Some(Rect::new(0.0, 0.0, 120.0, 80.0))
                        && primitive.clip_mask.is_some()
                        && primitive.brush == expected.with_alpha_factor(0.75)
            ));
        }

        let complex_brush = view_model.state(BackgroundBrush::Solid(Color::rgba(244, 63, 94, 210)));
        let complex_signal = complex_brush.signal();
        let complex: WidgetTree<()> = WidgetTree::new(Stack::new().size(dp(80.0), dp(40.0)).style(
            move |style, _| {
                style.surface.background_brush = Some(complex_signal.clone().into());
                style.surface.background_blur = dp(6.0).into();
                style.surface.shadow = Some(
                    crate::ui::theme::Shadow {
                        offset_x: dp(0.0),
                        offset_y: dp(4.0),
                        blur: dp(12.0),
                        spread: dp(0.0),
                        color: Color::rgba(15, 23, 42, 96),
                    }
                    .into(),
                );
            },
        ));
        let mut direct_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct_complex.run_layout_and_scene(&complex, start);
        super::super::resolved::background_brush_direct_probe::reset();
        let direct_complex_value = direct_complex
            .resolve_all_plain_container_background_brushes_for_benchmark(&complex, start, false);
        assert_eq!(
            super::super::resolved::background_brush_direct_probe::hits(),
            0,
            "complex brush surface must use the complete resolver"
        );
        let mut legacy_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_complex_value = legacy_complex
            .resolve_all_plain_container_background_brushes_for_benchmark(&complex, start, true);
        assert_eq!(direct_complex_value, legacy_complex_value);
        assert!(matches!(direct_complex_value.as_slice(), [Some(_)]));
    }

    #[test]
    fn plain_hidden_container_offset_direct_matches_legacy_full_and_complex_falls_back() {
        let view_model = ViewModelContext::for_benchmarks();
        let offset = view_model.state(Point::new(dp(3.0), dp(4.0)));
        let signal = offset.signal();
        let background = Color::rgba(14, 165, 233, 224);
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(dp(80.0), dp(60.0))
                .overflow(Overflow::Hidden)
                .opacity(0.75)
                .style(|style, _| style.surface.border_radius = Some(dp(12.0).into()))
                .child(Stack::new().size(dp(48.0), dp(32.0)).offset(signal).style(
                    move |style, _| {
                        style.surface.background = Some(background.into());
                        style.surface.border_radius = Some(dp(8.0).into());
                    },
                )),
        );
        let viewport = Rect::new(0.0, 0.0, 100.0, 80.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);

        for (frame_index, expected_offset) in
            [Point::new(dp(3.0), dp(4.0)), Point::new(dp(11.0), dp(9.0))]
                .into_iter()
                .enumerate()
        {
            let frame = start + Duration::from_millis(frame_index as u64);
            if frame_index > 0 {
                offset.set(expected_offset);
                full.invalidate_all();
                full.run_layout_and_scene(&tree, frame);
            }

            super::super::resolved::offset_direct_probe::reset();
            let direct_value =
                direct.resolve_all_plain_container_offsets_for_benchmark(&tree, frame, false);
            assert_eq!(
                super::super::resolved::offset_direct_probe::hits(),
                1,
                "offset frame {frame_index} did not use the strict direct resolver"
            );
            let legacy_value =
                legacy.resolve_all_plain_container_offsets_for_benchmark(&tree, frame, true);
            assert_eq!(direct_value, legacy_value);

            let [Some(snapshot)] = direct_value.as_slice() else {
                panic!("expected one offset snapshot at frame {frame_index}");
            };
            let expected_rect = Rect::new(expected_offset.x, expected_offset.y, dp(48.0), dp(32.0));
            assert_eq!(
                snapshot.background,
                Some((expected_rect, background.with_alpha_factor(0.75)))
            );
            assert_eq!(snapshot.border, None);
            assert_eq!(snapshot.backdrop_blur, None);
            assert_eq!(snapshot.brush, None);
            assert!(!snapshot.has_texture);
            let (_, hit_rect, hit_clip) = snapshot
                .container_occluder
                .expect("visible solid offset surface must retain an Occluder");
            assert_eq!(hit_rect, expected_rect);
            assert_eq!(hit_clip, Some(Rect::new(0.0, 0.0, 80.0, 60.0)));

            let full_scene = full.cached_scene.as_ref().expect("full offset scene");
            assert_eq!(full_scene.scene.shapes.len(), 1);
            assert_eq!(full_scene.scene.shapes[0].rect, expected_rect);
            assert_eq!(
                full_scene.scene.shapes[0].color,
                background.with_alpha_factor(0.75)
            );
            assert!(full_scene.scene.shapes[0].clip_mask.is_some());
            assert_eq!(full_scene.hit_regions.len(), 1);
            assert_eq!(full_scene.hit_regions[0].rect, hit_rect);
            assert_eq!(full_scene.hit_regions[0].clip_rect, hit_clip);
        }

        let complex_offset = view_model.state(Point::ZERO);
        let complex_signal = complex_offset.signal();
        let complex: WidgetTree<()> = WidgetTree::new(
            Stack::new()
                .size(dp(48.0), dp(32.0))
                .offset(complex_signal)
                .style(move |style, _| {
                    style.surface.background = Some(background.into());
                    style.surface.background_blur = dp(6.0).into();
                }),
        );
        let mut direct_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct_complex.run_layout_and_scene(&complex, start);
        super::super::resolved::offset_direct_probe::reset();
        let direct_complex_value = direct_complex
            .resolve_all_plain_container_offsets_for_benchmark(&complex, start, false);
        assert_eq!(super::super::resolved::offset_direct_probe::hits(), 0);
        let mut legacy_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_complex_value =
            legacy_complex.resolve_all_plain_container_offsets_for_benchmark(&complex, start, true);
        assert_eq!(direct_complex_value, legacy_complex_value);
        assert!(matches!(
            direct_complex_value.as_slice(),
            [Some(ContainerOffsetResolveSnapshot {
                backdrop_blur: Some(_),
                ..
            })]
        ));
    }

    #[test]
    fn animated_plain_container_scale_multiframe_direct_matches_legacy_and_full_recollect() {
        const TRANSITION: Duration = Duration::from_millis(400);

        let view_model = ViewModelContext::for_benchmarks();
        let scale = view_model.state(1.0_f32);
        let signal = scale
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let background = Color::rgba(15, 23, 42, 255);
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(100.0), dp(24.0))
            .overflow(Overflow::Hidden)
            .gap(dp(1.0))
            .opacity(0.75);
        for _ in 0..4 {
            let signal = signal.clone();
            row = row.child(
                Stack::new()
                    .size(dp(24.0), dp(24.0))
                    .overflow(Overflow::Hidden)
                    .scale(signal)
                    .style(move |style, _| {
                        style.surface.background = Some(background.into());
                        style.surface.border_radius = Some(dp(6.0).into());
                    }),
            );
        }
        let tree = WidgetTree::new(row);
        let viewport = Rect::new(0.0, 0.0, 100.0, 24.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_scale_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);

        let direct_initial = direct.run_layout_and_scene(&tree, start);
        let legacy_initial = legacy.run_layout_and_scene(&tree, start);
        let full_initial = full.run_layout_and_scene(&tree, start);
        assert_eq!(direct_initial, legacy_initial);
        assert_eq!(direct_initial, full_initial);
        assert_plain_container_animation_scene_equivalent(&direct, &legacy, &full, Duration::ZERO);

        super::super::resolved::scale_direct_probe::reset();
        let direct_values =
            direct.resolve_all_plain_container_scales_for_benchmark(&tree, start, false);
        assert_eq!(super::super::resolved::scale_direct_probe::hits(), 4);
        let legacy_values =
            legacy.resolve_all_plain_container_scales_for_benchmark(&tree, start, true);
        assert_eq!(direct_values, legacy_values);
        assert_eq!(direct_values.len(), 4);
        for value in direct_values.into_iter().flatten() {
            assert!(matches!(
                value.background,
                Some((rect, color, radius))
                    if rect.width == dp(24.0)
                        && rect.height == dp(24.0)
                        && color == background.with_alpha_factor(0.75)
                        && (radius - 6.0).abs() <= f32::EPSILON
            ));
            assert!(value.border.is_none());
            assert!(value.backdrop_blur.is_none());
            assert!(value.brush.is_none());
            assert!(!value.has_texture);
            let (_, hit_rect, _) = value
                .container_occluder
                .expect("solid Scale target should retain its Occluder");
            assert_eq!(Some(hit_rect), value.background.map(|(rect, _, _)| rect));
        }

        scale.set(1.5);
        let animation_start = start + Duration::from_millis(1);
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);

        for (frame_offset, expected_scale) in [
            (Duration::from_millis(100), 1.125_f32),
            (Duration::from_millis(200), 1.25_f32),
            (Duration::from_millis(350), 1.4375_f32),
            (TRANSITION, 1.5_f32),
        ] {
            let frame = animation_start + frame_offset;
            let direct_stats = direct.run_layout_and_scene(&tree, frame);
            let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
            let full_stats = full.run_layout_and_scene(&tree, frame);
            assert_eq!(
                direct_stats, legacy_stats,
                "stats differ at {frame_offset:?}"
            );
            assert_eq!(direct_stats, full_stats, "stats differ at {frame_offset:?}");

            for (label, activity) in [
                ("direct", direct.animation_frame_activity()),
                ("legacy", legacy.animation_frame_activity()),
            ] {
                assert!(activity.refresh_changed, "{label} frame did not refresh");
                assert_eq!(activity.scene_widget_count, 4, "{label} target count");
                assert!(
                    activity.reactive_slot_write_succeeded,
                    "{label} Scale frame should retain one shape and one Occluder per target"
                );
                assert!(activity.scene_patch_succeeded, "{label} scene patch failed");
                assert!(!activity.fell_back, "{label} frame fell back globally");
            }
            assert!(full.animation_frame_activity().full_scene_recollect);
            assert_plain_container_animation_scene_equivalent(
                &direct,
                &legacy,
                &full,
                frame_offset,
            );

            let scene = &direct.cached_scene.as_ref().expect("direct scene").scene;
            assert_eq!(scene.shapes.len(), 4);
            for shape in &scene.shapes {
                assert!(
                    (shape.rect.width.get() - 24.0 * expected_scale).abs() <= 1.0e-4,
                    "unexpected scaled width at {frame_offset:?}: {:?}",
                    shape.rect
                );
                assert!((shape.rect.height.get() - 24.0 * expected_scale).abs() <= 1.0e-4);
                assert_eq!(shape.color, background.with_alpha_factor(0.75));
                assert!((shape.corner_radius - 6.0).abs() <= f32::EPSILON);
            }
        }
    }

    #[test]
    fn complex_container_scale_multiframe_sticky_fallback_matches_full_recollect() {
        const TRANSITION: Duration = Duration::from_millis(480);

        let view_model = ViewModelContext::for_benchmarks();
        let scale = view_model.state(1.0_f32);
        let signal = scale
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let shadow_signal = signal.clone();
        let brush_signal = signal.clone();
        let background = Color::rgba(15, 23, 42, 255);
        let brush_color = Color::rgba(99, 102, 241, 160);
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::new(Axis::Horizontal)
                .size(dp(97.0), dp(32.0))
                .overflow(Overflow::Hidden)
                .gap(dp(1.0))
                .opacity(0.75)
                .child(
                    Stack::new()
                        .size(dp(48.0), dp(32.0))
                        .overflow(Overflow::Hidden)
                        .scale(shadow_signal)
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.border_radius = Some(dp(6.0).into());
                            style.surface.shadow = Some(
                                crate::ui::theme::Shadow {
                                    offset_x: dp(0.0),
                                    offset_y: dp(3.0),
                                    blur: dp(10.0),
                                    spread: dp(0.0),
                                    color: Color::rgba(15, 23, 42, 96),
                                }
                                .into(),
                            );
                        }),
                )
                .child(
                    Stack::new()
                        .size(dp(48.0), dp(32.0))
                        .overflow(Overflow::Hidden)
                        .scale(brush_signal)
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.border_radius = Some(dp(6.0).into());
                            style.surface.background_brush =
                                Some(BackgroundBrush::Solid(brush_color).into());
                        }),
                ),
        );
        let viewport = Rect::new(0.0, 0.0, 97.0, 40.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_scale_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        full.set_force_legacy_scale_reactive_resolve(true);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);
        assert_plain_container_animation_scene_equivalent(&direct, &legacy, &full, Duration::ZERO);

        scale.set(0.75);
        let animation_start = start + Duration::from_millis(1);
        super::super::resolved::scale_direct_probe::reset();
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);

        for (frame_index, frame_offset) in [
            Duration::from_millis(120),
            Duration::from_millis(240),
            Duration::from_millis(440),
            TRANSITION,
        ]
        .into_iter()
        .enumerate()
        {
            let frame = animation_start + frame_offset;
            let direct_stats = direct.run_layout_and_scene(&tree, frame);
            let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
            let full_stats = full.run_layout_and_scene(&tree, frame);
            assert_eq!(
                direct_stats, legacy_stats,
                "stats differ at {frame_offset:?}"
            );
            assert_eq!(
                direct_stats, full_stats,
                "full stats differ at {frame_offset:?}"
            );

            assert_eq!(
                super::super::resolved::scale_direct_probe::attempts(),
                2,
                "complex Scale targets should be classified only at transition start"
            );
            assert_eq!(
                super::super::resolved::scale_direct_probe::prepared_fallbacks(),
                2
            );
            assert_eq!(super::super::resolved::scale_direct_probe::hits(), 0);

            let activity = direct.animation_frame_activity();
            assert!(
                activity.refresh_changed,
                "fallback frame {frame_index} did not refresh"
            );
            assert_eq!(activity.scene_widget_count, 2);
            assert!(
                !activity.reactive_slot_write_succeeded,
                "one unsupported shadow target must reject the all-or-nothing Scale slot batch"
            );
            assert!(activity.scene_patch_succeeded);
            assert!(!activity.fell_back);
            assert!(full.animation_frame_activity().full_scene_recollect);
            assert_plain_container_animation_scene_equivalent(
                &direct,
                &legacy,
                &full,
                frame_offset,
            );

            let scene = &direct.cached_scene.as_ref().expect("direct scene").scene;
            assert_eq!(scene.shapes.len(), 1, "shadow background topology changed");
            assert_eq!(scene.brushes.len(), 1, "brush topology changed");
            assert_eq!(scene.textures.len(), 1, "shadow texture topology changed");
        }
    }

    #[test]
    fn plain_container_scale_reduced_motion_matches_full_resolver_and_settles() {
        const TRANSITION: Duration = Duration::from_secs(1);

        let view_model = ViewModelContext::for_benchmarks();
        let scale = view_model.state(1.0_f32);
        let signal = scale
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let tree: WidgetTree<()> = WidgetTree::new(
            Stack::new()
                .size(dp(24.0), dp(24.0))
                .overflow(Overflow::Hidden)
                .scale(signal)
                .style(|style, _| {
                    style.surface.background = Some(Color::rgba(14, 165, 233, 255).into());
                    style.surface.border_radius = Some(dp(6.0).into());
                }),
        );
        let viewport = Rect::new(0.0, 0.0, 40.0, 40.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);

        scale.set(1.5);
        let animation_start = start + Duration::from_millis(1);
        let _ = direct.resolve_all_plain_container_scales_with_reduced_motion_for_benchmark(
            &tree,
            animation_start,
            false,
            false,
        );
        let _ = legacy.resolve_all_plain_container_scales_with_reduced_motion_for_benchmark(
            &tree,
            animation_start,
            true,
            false,
        );

        let reduced_frame = animation_start + Duration::from_millis(100);
        super::super::resolved::scale_direct_probe::reset();
        let direct_reduced = direct
            .resolve_all_plain_container_scales_with_reduced_motion_for_benchmark(
                &tree,
                reduced_frame,
                false,
                true,
            );
        assert!(
            super::super::resolved::scale_direct_probe::hits() > 0,
            "reduced-motion Scale must use the strict direct resolver"
        );
        let legacy_reduced = legacy
            .resolve_all_plain_container_scales_with_reduced_motion_for_benchmark(
                &tree,
                reduced_frame,
                true,
                true,
            );
        assert_eq!(direct_reduced, legacy_reduced);
        assert!(matches!(
            direct_reduced.as_slice(),
            [Some(ContainerScaleResolveSnapshot {
                background: Some((rect, _, radius)),
                container_occluder: Some((_, hit_rect, _)),
                ..
            })] if rect == hit_rect
                && rect.width == dp(36.0)
                && rect.height == dp(36.0)
                && (*radius - 6.0).abs() <= f32::EPSILON
        ));

        let settled = direct.resolve_all_plain_container_scales_for_benchmark(
            &tree,
            reduced_frame + Duration::from_millis(100),
            false,
        );
        assert_eq!(
            settled, direct_reduced,
            "reduced motion must settle the active slot"
        );
    }

    #[test]
    fn complex_container_scale_variants_use_sticky_full_fallback() {
        let view_model = ViewModelContext::for_benchmarks();
        let scale = view_model.state(1.25_f32);
        let scale_signal = scale.signal();
        let offset = view_model.state(Point::new(dp(2.0), dp(3.0)));
        let offset_signal = offset.signal();
        let background = Color::rgba(15, 23, 42, 255);

        let shadow_scale = scale_signal.clone();
        let brush_scale = scale_signal.clone();
        let child_scale = scale_signal.clone();
        let scroll_scale = scale_signal.clone();
        let focus_scale = scale_signal.clone();
        let clip_scale = scale_signal.clone();
        let offset_scale = scale_signal.clone();
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::new(Axis::Horizontal)
                .size(dp(224.0), dp(48.0))
                .overflow(Overflow::Hidden)
                .child(
                    Stack::new()
                        .size(dp(24.0), dp(24.0))
                        .overflow(Overflow::Hidden)
                        .scale(shadow_scale)
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.shadow = Some(
                                crate::ui::theme::Shadow {
                                    offset_x: dp(0.0),
                                    offset_y: dp(2.0),
                                    blur: dp(6.0),
                                    spread: dp(0.0),
                                    color: Color::rgba(15, 23, 42, 96),
                                }
                                .into(),
                            );
                        }),
                )
                .child(
                    Stack::new()
                        .size(dp(24.0), dp(24.0))
                        .overflow(Overflow::Hidden)
                        .scale(brush_scale)
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.background_brush =
                                Some(BackgroundBrush::Solid(Color::rgba(14, 165, 233, 224)).into());
                        }),
                )
                .child(
                    Stack::new()
                        .size(dp(24.0), dp(24.0))
                        .overflow(Overflow::Hidden)
                        .scale(child_scale)
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                        })
                        .child(Text::new("x")),
                )
                .child(
                    ScrollView::new()
                        .size(dp(24.0), dp(24.0))
                        .scale(scroll_scale)
                        .show_scrollbar(false)
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                        }),
                )
                .child(
                    Stack::new()
                        .size(dp(24.0), dp(24.0))
                        .overflow(Overflow::Hidden)
                        .scale(focus_scale)
                        .focusable(true)
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                        }),
                )
                .child(
                    Stack::new()
                        .size(dp(24.0), dp(24.0))
                        .overflow(Overflow::Hidden)
                        .style(|style, _| {
                            style.surface.border_radius = Some(dp(8.0).into());
                        })
                        .child(
                            Stack::new()
                                .size(dp(32.0), dp(32.0))
                                .overflow(Overflow::Hidden)
                                .scale(clip_scale)
                                .style(move |style, _| {
                                    style.surface.background = Some(background.into());
                                }),
                        ),
                )
                .child(
                    Stack::new()
                        .size(dp(24.0), dp(24.0))
                        .overflow(Overflow::Hidden)
                        .scale(offset_scale)
                        .offset(offset_signal)
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                        }),
                ),
        );
        let viewport = Rect::new(0.0, 0.0, 224.0, 48.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);

        super::super::resolved::scale_direct_probe::reset();
        let direct_values =
            direct.resolve_all_plain_container_scales_for_benchmark(&tree, now, false);
        let attempts = super::super::resolved::scale_direct_probe::attempts();
        assert_eq!(direct_values.len(), 7);
        assert_eq!(attempts, 7, "each complex target should be classified once");
        assert_eq!(super::super::resolved::scale_direct_probe::hits(), 0);
        assert_eq!(
            super::super::resolved::scale_direct_probe::prepared_fallbacks(),
            7
        );

        let repeated = direct.resolve_all_plain_container_scales_for_benchmark(
            &tree,
            now + Duration::from_millis(1),
            false,
        );
        assert_eq!(repeated, direct_values);
        assert_eq!(
            super::super::resolved::scale_direct_probe::attempts(),
            attempts,
            "sticky bit9 should suppress repeated speculative classification"
        );

        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_values =
            legacy.resolve_all_plain_container_scales_for_benchmark(&tree, now, true);
        assert_eq!(direct_values, legacy_values);
    }

    #[test]
    fn plain_container_background_blur_direct_matches_legacy_and_complex_surface_falls_back() {
        let view_model = ViewModelContext::for_benchmarks();
        let blur = view_model.state(dp(8.0));
        let signal = blur.signal();
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(dp(120.0), dp(80.0))
                .overflow(Overflow::Hidden)
                .opacity(0.75)
                .style(|style, _| style.surface.border_radius = Some(dp(12.0).into()))
                .child(
                    Stack::new()
                        .size(dp(80.0), dp(40.0))
                        .style(move |style, _| {
                            style.surface.background = Some(Color::rgba(15, 23, 42, 160).into());
                            style.surface.background_blur = signal.clone().into();
                            style.surface.border_width = Some(dp(2.0).into());
                            style.surface.border_color =
                                Some(Color::rgba(56, 189, 248, 224).into());
                            style.surface.border_radius = Some(dp(10.0).into());
                        }),
                ),
        );
        let viewport = Rect::new(0.0, 0.0, 140.0, 100.0);
        let now = Instant::now();

        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, now);
        super::super::resolved::background_blur_direct_probe::reset();
        let direct_value =
            direct.resolve_all_plain_container_background_blurs_for_benchmark(&tree, now, false);
        assert_eq!(
            super::super::resolved::background_blur_direct_probe::hits(),
            1
        );
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value =
            legacy.resolve_all_plain_container_background_blurs_for_benchmark(&tree, now, true);
        assert_eq!(direct_value, legacy_value);
        assert!(matches!(
            direct_value.as_slice(),
            [Some((primitive, Some(true)))]
                if primitive.rect == Rect::new(2.0, 2.0, 76.0, 36.0)
                    && (primitive.corner_radius - 8.0).abs() <= f32::EPSILON
                    && (primitive.blur_radius - 8.0).abs() <= f32::EPSILON
                    && primitive.clip_rect == Some(Rect::new(0.0, 0.0, 120.0, 80.0))
                    && primitive.clip_mask.is_some()
        ));

        let complex_blur = view_model.state(dp(8.0));
        let complex_signal = complex_blur.signal();
        let complex: WidgetTree<()> = WidgetTree::new(Stack::new().size(dp(80.0), dp(40.0)).style(
            move |style, _| {
                style.surface.background_blur = complex_signal.clone().into();
                style.surface.shadow = Some(crate::ui::theme::Shadow::default().into());
            },
        ));
        let mut direct_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct_complex.run_layout_and_scene(&complex, now);
        super::super::resolved::background_blur_direct_probe::reset();
        let direct_complex_value = direct_complex
            .resolve_all_plain_container_background_blurs_for_benchmark(&complex, now, false);
        assert_eq!(
            super::super::resolved::background_blur_direct_probe::hits(),
            0
        );
        let mut legacy_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_complex_value = legacy_complex
            .resolve_all_plain_container_background_blurs_for_benchmark(&complex, now, true);
        assert_eq!(direct_complex_value, legacy_complex_value);
        assert!(matches!(direct_complex_value.as_slice(), [Some(_)]));
    }

    #[test]
    fn plain_container_border_color_direct_resolve_matches_legacy_and_unsafe_surface_falls_back() {
        let view_model = ViewModelContext::for_benchmarks();
        let border = view_model.state(Color::rgba(14, 165, 233, 224));
        let signal = border.signal();
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::<()>::new(Axis::Vertical).opacity(0.75).child(
                Stack::new()
                    .size(dp(80.0), dp(40.0))
                    .style(move |style, _| {
                        style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                        style.surface.border_width = Some(dp(2.0).into());
                        style.surface.border_color = Some(signal.clone().into());
                    }),
            ),
        );
        let viewport = Rect::new(0.0, 0.0, 120.0, 80.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, now);
        super::super::resolved::border_color_direct_probe::reset();
        let direct_value =
            direct.resolve_all_plain_container_border_colors_for_benchmark(&tree, now, false);
        assert_eq!(
            super::super::resolved::border_color_direct_probe::attempts(),
            1
        );
        assert_eq!(super::super::resolved::border_color_direct_probe::hits(), 1);
        assert_eq!(
            super::super::resolved::border_color_direct_probe::prepared_fallbacks(),
            0
        );
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value =
            legacy.resolve_all_plain_container_border_colors_for_benchmark(&tree, now, true);
        assert_eq!(direct_value, legacy_value);
        assert!(matches!(
            direct_value.as_slice(),
            [Some((rect, stroke_width, color))]
                if *rect == Rect::new(0.0, 0.0, 80.0, 40.0)
                    && (*stroke_width - 2.0).abs() <= f32::EPSILON
                    && *color == Color::rgba(14, 165, 233, 224).with_alpha_factor(0.75)
        ));

        let unstable_border = view_model.state(Color::rgba(244, 63, 94, 210));
        let unstable_signal = unstable_border.signal();
        let unstable: WidgetTree<()> = WidgetTree::new(
            Stack::new()
                .size(dp(80.0), dp(40.0))
                .style(move |style, _| {
                    style.surface.background = Some(Color::TRANSPARENT.into());
                    style.surface.border_width = Some(dp(2.0).into());
                    style.surface.border_color = Some(unstable_signal.clone().into());
                }),
        );
        let mut direct_unstable = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct_unstable.set_force_legacy_border_color_reactive_resolve(true);
        direct_unstable.run_layout_and_scene(&unstable, now);
        direct_unstable.set_force_legacy_border_color_reactive_resolve(false);
        super::super::resolved::border_color_direct_probe::reset();
        let direct_fallback = direct_unstable
            .resolve_all_plain_container_border_colors_for_benchmark(&unstable, now, false);
        assert_eq!(
            super::super::resolved::border_color_direct_probe::attempts(),
            1
        );
        assert_eq!(super::super::resolved::border_color_direct_probe::hits(), 0);
        assert_eq!(
            super::super::resolved::border_color_direct_probe::prepared_fallbacks(),
            1
        );
        let mut legacy_unstable = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_fallback = legacy_unstable
            .resolve_all_plain_container_border_colors_for_benchmark(&unstable, now, true);
        assert_eq!(direct_fallback, legacy_fallback);
        assert_eq!(direct_fallback, vec![None]);
    }

    #[test]
    fn plain_container_border_radius_direct_resolve_matches_legacy_and_complex_surface_falls_back()
    {
        let view_model = ViewModelContext::for_benchmarks();
        let radius = view_model.state(dp(10.0));
        let signal = radius.signal();
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::<()>::new(Axis::Vertical).opacity(0.75).child(
                Stack::new()
                    .size(dp(80.0), dp(40.0))
                    .style(move |style, _| {
                        style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                        style.surface.border_width = Some(dp(2.0).into());
                        style.surface.border_color = Some(Color::rgba(14, 165, 233, 224).into());
                        style.surface.border_radius = Some(signal.clone().into());
                    }),
            ),
        );
        let viewport = Rect::new(0.0, 0.0, 120.0, 80.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, now);
        super::super::resolved::border_radius_direct_probe::reset();
        let direct_value =
            direct.resolve_all_plain_container_border_radii_for_benchmark(&tree, now, false);
        assert_eq!(
            super::super::resolved::border_radius_direct_probe::attempts(),
            1
        );
        assert_eq!(
            super::super::resolved::border_radius_direct_probe::hits(),
            1
        );
        assert_eq!(
            super::super::resolved::border_radius_direct_probe::prepared_fallbacks(),
            0
        );
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value =
            legacy.resolve_all_plain_container_border_radii_for_benchmark(&tree, now, true);
        assert_eq!(direct_value, legacy_value);
        assert!(matches!(
            direct_value.as_slice(),
            [Some((Some((background_rect, background_radius)), Some((border_rect, border_radius))))]
                if *background_rect == Rect::new(2.0, 2.0, 76.0, 36.0)
                    && (*background_radius - 8.0).abs() <= f32::EPSILON
                    && *border_rect == Rect::new(0.0, 0.0, 80.0, 40.0)
                    && (*border_radius - 10.0).abs() <= f32::EPSILON
        ));

        let complex_radius = view_model.state(dp(10.0));
        let complex_signal = complex_radius.signal();
        let complex: WidgetTree<()> = WidgetTree::new(Stack::new().size(dp(80.0), dp(40.0)).style(
            move |style, context| {
                style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                style.surface.border_radius = Some(complex_signal.clone().into());
                style.surface.shadow = Some(context.theme.elevation.md.clone().into());
            },
        ));
        let mut direct_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct_complex.set_force_legacy_border_radius_reactive_resolve(true);
        direct_complex.run_layout_and_scene(&complex, now);
        direct_complex.set_force_legacy_border_radius_reactive_resolve(false);
        super::super::resolved::border_radius_direct_probe::reset();
        let direct_fallback = direct_complex
            .resolve_all_plain_container_border_radii_for_benchmark(&complex, now, false);
        assert_eq!(
            super::super::resolved::border_radius_direct_probe::attempts(),
            1
        );
        assert_eq!(
            super::super::resolved::border_radius_direct_probe::hits(),
            0
        );
        assert_eq!(
            super::super::resolved::border_radius_direct_probe::prepared_fallbacks(),
            1
        );
        let mut legacy_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_fallback = legacy_complex
            .resolve_all_plain_container_border_radii_for_benchmark(&complex, now, true);
        assert_eq!(direct_fallback, legacy_fallback);
        assert_eq!(direct_fallback, vec![None]);
    }

    #[test]
    fn plain_container_border_width_direct_resolve_matches_legacy_and_complex_surface_falls_back() {
        let view_model = ViewModelContext::for_benchmarks();
        let width = view_model.state(dp(3.0));
        let signal = width.signal();
        let tree: WidgetTree<()> = WidgetTree::new(Stack::new().size(dp(80.0), dp(40.0)).style(
            move |style, _| {
                style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                style.surface.border_width = Some(signal.clone().into());
                style.surface.border_color = Some(Color::rgba(14, 165, 233, 224).into());
                style.surface.border_radius = Some(dp(10.0).into());
            },
        ));
        let viewport = Rect::new(0.0, 0.0, 120.0, 80.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, now);
        super::super::resolved::border_width_direct_probe::reset();
        let direct_value =
            direct.resolve_all_plain_container_border_widths_for_benchmark(&tree, now, false);
        assert_eq!(
            super::super::resolved::border_width_direct_probe::attempts(),
            1
        );
        assert_eq!(super::super::resolved::border_width_direct_probe::hits(), 1);
        assert_eq!(
            super::super::resolved::border_width_direct_probe::prepared_fallbacks(),
            0
        );
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value =
            legacy.resolve_all_plain_container_border_widths_for_benchmark(&tree, now, true);
        assert_eq!(direct_value, legacy_value);
        assert!(matches!(
            direct_value.as_slice(),
            [Some((Some((background_rect, background_radius)), Some((border_rect, border_width))))]
                if *background_rect == Rect::new(3.0, 3.0, 74.0, 34.0)
                    && (*background_radius - 7.0).abs() <= f32::EPSILON
                    && *border_rect == Rect::new(0.0, 0.0, 80.0, 40.0)
                    && (*border_width - 3.0).abs() <= f32::EPSILON
        ));

        let complex_width = view_model.state(dp(3.0));
        let complex_signal = complex_width.signal();
        let complex: WidgetTree<()> = WidgetTree::new(Stack::new().size(dp(80.0), dp(40.0)).style(
            move |style, context| {
                style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                style.surface.border_width = Some(complex_signal.clone().into());
                style.surface.border_color = Some(Color::rgba(14, 165, 233, 224).into());
                style.surface.shadow = Some(context.theme.elevation.md.clone().into());
            },
        ));
        let mut direct_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct_complex.set_force_legacy_border_width_reactive_resolve(true);
        direct_complex.run_layout_and_scene(&complex, now);
        direct_complex.set_force_legacy_border_width_reactive_resolve(false);
        super::super::resolved::border_width_direct_probe::reset();
        let direct_fallback = direct_complex
            .resolve_all_plain_container_border_widths_for_benchmark(&complex, now, false);
        assert_eq!(
            super::super::resolved::border_width_direct_probe::attempts(),
            1
        );
        assert_eq!(super::super::resolved::border_width_direct_probe::hits(), 0);
        assert_eq!(
            super::super::resolved::border_width_direct_probe::prepared_fallbacks(),
            1
        );
        let mut legacy_complex = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_fallback = legacy_complex
            .resolve_all_plain_container_border_widths_for_benchmark(&complex, now, true);
        assert_eq!(direct_fallback, legacy_fallback);
        assert_eq!(direct_fallback, vec![None]);
    }

    #[test]
    fn progress_value_direct_resolve_matches_legacy_and_indeterminate_falls_back() {
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::<()>::new(Axis::Vertical).opacity(0.75).child(
                ProgressBar::new(0.37)
                    .size(dp(220.0), dp(32.0))
                    .show_label(true)
                    .style(|style, _| {
                        style.track_color = Color::rgba(15, 23, 42, 160).into();
                        style.fill_color = Color::rgba(14, 165, 233, 224).into();
                    }),
            ),
        );
        let viewport = Rect::new(0.0, 0.0, 240.0, 48.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_value = direct.resolve_all_progress_values_for_benchmark(&tree, now, false);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value = legacy.resolve_all_progress_values_for_benchmark(&tree, now, true);
        assert_eq!(direct_value, legacy_value);
        let value = direct_value
            .into_iter()
            .next()
            .flatten()
            .expect("determinate ProgressValue");
        assert_eq!(value.fill_rect.width, value.track_rect.width * 0.37);
        assert_eq!(
            value.track_color,
            Color::rgba(15, 23, 42, 160).with_alpha_factor(0.75)
        );
        assert_eq!(
            value.fill_color,
            Color::rgba(14, 165, 233, 224).with_alpha_factor(0.75)
        );
        assert_eq!(
            value.label.as_ref().map(|label| label.content.as_str()),
            Some("37%")
        );

        let indeterminate: WidgetTree<()> =
            WidgetTree::new(ProgressBar::indeterminate(true).size(dp(220.0), dp(16.0)));
        let mut direct_indeterminate = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_fallback = direct_indeterminate.resolve_all_progress_values_for_benchmark(
            &indeterminate,
            now,
            false,
        );
        let mut legacy_indeterminate = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_fallback = legacy_indeterminate.resolve_all_progress_values_for_benchmark(
            &indeterminate,
            now,
            true,
        );
        assert_eq!(direct_fallback, legacy_fallback);
        assert_eq!(direct_fallback, vec![None]);
    }

    #[test]
    fn slider_value_direct_resolve_accepts_default_cursor_and_matches_legacy() {
        let view_model = ViewModelContext::for_benchmarks();
        let value = view_model.state(0.37_f32);
        let tree: WidgetTree<()> = WidgetTree::new(
            Slider::new(value.signal(), 0.0, 1.0)
                .step(0.01)
                .size(dp(220.0), dp(32.0)),
        );
        let viewport = Rect::new(0.0, 0.0, 240.0, 48.0);
        let now = Instant::now();

        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, now);
        super::super::resolved::slider_value_direct_probe::reset();
        let direct_value = direct.resolve_all_slider_values_for_benchmark(&tree, now, false);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::attempts(),
            1
        );
        assert_eq!(super::super::resolved::slider_value_direct_probe::hits(), 1);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::prepared_fallbacks(),
            0
        );

        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value = legacy.resolve_all_slider_values_for_benchmark(&tree, now, true);
        assert_eq!(direct_value, legacy_value);
        assert!(matches!(direct_value.as_slice(), [Some(_)]));
    }

    #[test]
    fn slider_value_equal_valued_surface_signal_uses_transient_fallback() {
        let view_model = ViewModelContext::for_benchmarks();
        let value = view_model.state(0.37_f32);
        let surface_opacity = view_model.state(1.0_f32);
        let surface_opacity_signal = surface_opacity.signal();
        let tree: WidgetTree<()> = WidgetTree::new(
            Slider::new(value.signal(), 0.0, 1.0)
                .step(0.01)
                .size(dp(220.0), dp(32.0))
                .style(move |style, _| {
                    style.surface.opacity = surface_opacity_signal.clone().into();
                }),
        );
        let viewport = Rect::new(0.0, 0.0, 240.0, 48.0);
        let now = Instant::now();

        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        super::super::resolved::with_legacy_slider_value_reactive_resolve(true, || {
            direct.run_layout_and_scene(&tree, now);
        });
        super::super::resolved::slider_value_direct_probe::reset();
        let direct_value = direct.resolve_all_slider_values_for_benchmark(&tree, now, false);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::attempts(),
            1
        );
        assert_eq!(super::super::resolved::slider_value_direct_probe::hits(), 0);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::prepared_fallbacks(),
            1
        );

        let repeated = direct.resolve_all_slider_values_for_benchmark(
            &tree,
            now + Duration::from_millis(1),
            false,
        );
        assert_eq!(repeated, direct_value);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::attempts(),
            2,
            "state-resolved surface fallback must remain retryable"
        );
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::prepared_fallbacks(),
            2
        );

        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value = legacy.resolve_all_slider_values_for_benchmark(&tree, now, true);
        assert_eq!(direct_value, legacy_value);
        assert!(matches!(direct_value.as_slice(), [Some(_)]));
    }

    #[test]
    fn slider_value_structural_ticks_fallback_is_sticky_and_resets_with_layout() {
        let view_model = ViewModelContext::for_benchmarks();
        let value = view_model.state(0.37_f32);
        let tree: WidgetTree<()> = WidgetTree::new(
            Slider::new(value.signal(), 0.0, 1.0)
                .step(0.01)
                .show_ticks(true)
                .size(dp(220.0), dp(40.0)),
        );
        let viewport = Rect::new(0.0, 0.0, 240.0, 56.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        super::super::resolved::with_legacy_slider_value_reactive_resolve(true, || {
            direct.run_layout_and_scene(&tree, now);
        });
        super::super::resolved::slider_value_direct_probe::reset();
        let first = direct.resolve_all_slider_values_for_benchmark(&tree, now, false);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::attempts(),
            1
        );
        assert_eq!(super::super::resolved::slider_value_direct_probe::hits(), 0);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::prepared_fallbacks(),
            1
        );
        let repeated = direct.resolve_all_slider_values_for_benchmark(
            &tree,
            now + Duration::from_millis(1),
            false,
        );
        assert_eq!(repeated, first);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::attempts(),
            1,
            "structural ticks fallback should set sticky bit10"
        );

        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_value = legacy.resolve_all_slider_values_for_benchmark(&tree, now, true);
        assert_eq!(first, legacy_value);
        assert_eq!(first, vec![None]);

        direct.invalidate_all();
        super::super::resolved::slider_value_direct_probe::reset();
        let rebuilt = direct.resolve_all_slider_values_for_benchmark(
            &tree,
            now + Duration::from_millis(2),
            false,
        );
        assert_eq!(rebuilt, first);
        let rebuilt_attempts = super::super::resolved::slider_value_direct_probe::attempts();
        assert!(
            rebuilt_attempts >= 1,
            "rebuilt resolved layout should reset the sticky fallback mask"
        );
        assert_eq!(super::super::resolved::slider_value_direct_probe::hits(), 0);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::prepared_fallbacks(),
            rebuilt_attempts
        );
        let rebuilt_repeated = direct.resolve_all_slider_values_for_benchmark(
            &tree,
            now + Duration::from_millis(3),
            false,
        );
        assert_eq!(rebuilt_repeated, first);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::attempts(),
            rebuilt_attempts,
            "rebuilt layout should become sticky again after its first fallback"
        );
    }

    #[test]
    fn slider_value_direct_resolve_matches_legacy_across_orientation_states_and_label() {
        let view_model = ViewModelContext::for_benchmarks();
        let value = view_model.state(0.37_f32);
        let horizontal: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .show_value_label(true)
            .size(dp(220.0), dp(52.0))
            .into();
        let vertical: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .vertical()
            .size(dp(52.0), dp(180.0))
            .into();
        let hovered: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .size(dp(220.0), dp(32.0))
            .into();
        let hovered_id = hovered.id;
        let pressed: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .size(dp(220.0), dp(32.0))
            .into();
        let pressed_id = pressed.id;
        let disabled: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .disable(true)
            .size(dp(220.0), dp(32.0))
            .into();
        let validation: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .validation(crate::foundation::form::ValidationVisualState {
                invalid: true,
                ..Default::default()
            })
            .size(dp(220.0), dp(32.0))
            .into();
        let validation_id = validation.id;

        let tree = WidgetTree::new(
            Flex::<()>::new(Axis::Vertical)
                .size(dp(260.0), dp(360.0))
                .gap(dp(4.0))
                .child(horizontal)
                .child(vertical)
                .child(hovered)
                .child(pressed)
                .child(disabled)
                .child(validation),
        );
        let mut states = WidgetStateMap::default();
        states.set(
            hovered_id,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );
        states.set(
            pressed_id,
            crate::ui::theme::WidgetState {
                pressed: true,
                ..Default::default()
            },
        );
        states.set(
            validation_id,
            crate::ui::theme::WidgetState {
                invalid: true,
                ..Default::default()
            },
        );
        let viewport = Rect::new(0.0, 0.0, 280.0, 380.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        super::super::resolved::with_legacy_slider_value_reactive_resolve(true, || {
            direct.run_layout_and_scene(&tree, now);
        });
        super::super::resolved::slider_value_direct_probe::reset();
        let transition_start = now + Duration::from_millis(1);
        let direct_initial = direct.resolve_all_slider_values_for_benchmark_with_states(
            &tree,
            transition_start,
            false,
            &states,
        );
        let settled = now + Duration::from_secs(1);
        let direct_values = direct
            .resolve_all_slider_values_for_benchmark_with_states(&tree, settled, false, &states);
        let attempts = super::super::resolved::slider_value_direct_probe::attempts();
        let hits = super::super::resolved::slider_value_direct_probe::hits();
        let prepared_fallbacks =
            super::super::resolved::slider_value_direct_probe::prepared_fallbacks();
        assert!(
            hits >= 10,
            "both state frames should hit five eligible sliders"
        );
        assert!(
            prepared_fallbacks >= 1,
            "disabled Slider must retain its same-color fallback"
        );
        assert_eq!(attempts, hits + prepared_fallbacks);

        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        super::super::resolved::with_legacy_slider_value_reactive_resolve(true, || {
            legacy.run_layout_and_scene(&tree, now);
        });
        let legacy_initial = legacy.resolve_all_slider_values_for_benchmark_with_states(
            &tree,
            transition_start,
            true,
            &states,
        );
        let legacy_values = legacy
            .resolve_all_slider_values_for_benchmark_with_states(&tree, settled, true, &states);
        assert_eq!(direct_initial, legacy_initial);
        assert_eq!(direct_values, legacy_values);
        assert_eq!(direct_values.len(), 6);
        assert!(direct_values[0]
            .as_ref()
            .is_some_and(|value| value.label.is_some()));
        assert!(direct_values[1]
            .as_ref()
            .is_some_and(|value| { value.track_rect.height > value.track_rect.width }));
        assert!(
            direct_values[4].is_none(),
            "disabled slider should fall back"
        );
        assert!(direct_values
            .iter()
            .enumerate()
            .all(|(index, value)| index == 4 || value.is_some()));
        assert_ne!(
            direct_values[2]
                .as_ref()
                .expect("hovered slider")
                .thumb_color,
            direct_values[0]
                .as_ref()
                .expect("normal slider")
                .thumb_color,
            "hover state should resolve a distinct retained thumb color"
        );
        assert_ne!(
            direct_values[3]
                .as_ref()
                .expect("pressed slider")
                .thumb_color,
            direct_values[0]
                .as_ref()
                .expect("normal slider")
                .thumb_color,
            "pressed state should resolve a distinct retained thumb color"
        );
        assert_ne!(
            direct_values[5]
                .as_ref()
                .expect("validation slider")
                .active_track_color,
            direct_values[0]
                .as_ref()
                .expect("normal slider")
                .active_track_color,
            "validation state should resolve the error active-track color"
        );
    }

    #[test]
    fn slider_value_direct_resolve_falls_back_for_unsupported_scene_shapes() {
        let view_model = ViewModelContext::for_benchmarks();
        let value = view_model.state(0.37_f32);
        let ticks: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .show_ticks(true)
            .size(dp(220.0), dp(40.0))
            .into();
        let thumb_shadow: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .size(dp(220.0), dp(40.0))
            .style(|style, context| {
                style.thumb_shadow = Some(context.theme.elevation.sm.clone());
            })
            .into();
        let surface_opacity: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .size(dp(220.0), dp(40.0))
            .style(|style, _| {
                style.surface.opacity = 0.75.into();
            })
            .into();
        let surface_offset: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .size(dp(220.0), dp(40.0))
            .style(|style, _| {
                style.surface.offset = Point::new(dp(4.0), dp(2.0)).into();
            })
            .into();
        let newline_formatter: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .show_value_label(true)
            .format_value(|_| String::from("first\nsecond"))
            .size(dp(220.0), dp(52.0))
            .into();
        let degenerate: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .size(dp(0.0), dp(0.0))
            .style(|style, _| {
                style.track_height = Dp::ZERO;
                style.thumb_size = Dp::ZERO;
                style.min_width = Dp::ZERO;
                style.min_height = Dp::ZERO;
            })
            .into();
        let mut nonfinite_offset: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .size(dp(220.0), dp(40.0))
            .into();
        nonfinite_offset.visual.offset = Value::Static(Point::new(dp(f32::INFINITY), dp(0.0)));
        let tree = WidgetTree::new(
            Flex::<()>::new(Axis::Vertical)
                .size(dp(240.0), dp(340.0))
                .gap(dp(4.0))
                .child(ticks)
                .child(thumb_shadow)
                .child(surface_opacity)
                .child(surface_offset)
                .child(newline_formatter)
                .child(degenerate)
                .child(nonfinite_offset),
        );
        let viewport = Rect::new(0.0, 0.0, 260.0, 360.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        super::super::resolved::with_legacy_slider_value_reactive_resolve(true, || {
            direct.run_layout_and_scene(&tree, now);
        });
        super::super::resolved::slider_value_direct_probe::reset();
        let direct_values = direct.resolve_all_slider_values_for_benchmark(&tree, now, false);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::attempts(),
            7
        );
        assert_eq!(super::super::resolved::slider_value_direct_probe::hits(), 0);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::prepared_fallbacks(),
            7
        );

        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_values = legacy.resolve_all_slider_values_for_benchmark(&tree, now, true);
        assert_eq!(direct_values, legacy_values);
        assert_eq!(direct_values.len(), 7);
        assert!(
            direct_values[0].is_none(),
            "ticks must use the full resolver"
        );
        assert!(
            direct_values[1].is_none(),
            "thumb shadow must use the full resolver"
        );
        assert!(
            direct_values[2].is_some(),
            "surface opacity must retain value geometry"
        );
        assert!(
            direct_values[3].is_some(),
            "surface offset must retain value geometry"
        );
        assert!(
            direct_values[4].is_none(),
            "newline formatter must use the prepared fallback"
        );
        assert!(
            direct_values[5].is_none(),
            "degenerate geometry must fall back"
        );
        assert!(
            direct_values[6]
                .as_ref()
                .is_some_and(|value| !value.track_rect.x.get().is_finite()),
            "non-finite visual geometry should be delegated to the full resolver"
        );
    }

    #[test]
    fn slider_value_hover_stylesheet_shadow_fallback_recovers_after_hover() {
        let view_model = ViewModelContext::for_benchmarks();
        let value = view_model.state(0.37_f32);
        let slider: Element<()> = Slider::new(value.signal(), 0.0, 1.0)
            .step(0.01)
            .size(dp(220.0), dp(32.0))
            .into();
        let slider_id = slider.id;
        let tree = WidgetTree::new(slider);
        let mut hovered_states = WidgetStateMap::default();
        hovered_states.set(
            slider_id,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );
        let sheet = crate::ui::widget::StyleSheet::new().slider(
            crate::ui::widget::StyleSelector::state(crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            }),
            |style, context| {
                style.thumb_shadow = Some(context.theme.elevation.sm.clone());
            },
        );
        let viewport = Rect::new(0.0, 0.0, 240.0, 48.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.style_sheet = sheet.clone();
        super::super::resolved::with_legacy_slider_value_reactive_resolve(true, || {
            direct.run_layout_and_scene(&tree, now);
        });
        super::super::resolved::slider_value_direct_probe::reset();
        let direct_hovered = direct.resolve_all_slider_values_for_benchmark_with_states(
            &tree,
            now,
            false,
            &hovered_states,
        );
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::attempts(),
            1
        );
        assert_eq!(super::super::resolved::slider_value_direct_probe::hits(), 0);
        assert_eq!(
            super::super::resolved::slider_value_direct_probe::prepared_fallbacks(),
            1
        );
        assert_eq!(direct_hovered, vec![None]);
        let attempts_after_hover = super::super::resolved::slider_value_direct_probe::attempts();
        let direct_normal = direct.resolve_all_slider_values_for_benchmark_with_states(
            &tree,
            now + Duration::from_millis(1),
            false,
            &WidgetStateMap::default(),
        );
        assert!(matches!(direct_normal.as_slice(), [Some(_)]));
        assert!(
            super::super::resolved::slider_value_direct_probe::attempts() > attempts_after_hover,
            "hover-only shadow fallback must not set sticky bit10"
        );
        assert!(
            super::super::resolved::slider_value_direct_probe::hits() >= 1,
            "leaving hover should retry and hit the direct resolver"
        );

        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.style_sheet = sheet;
        let legacy_hovered = legacy.resolve_all_slider_values_for_benchmark_with_states(
            &tree,
            now,
            true,
            &hovered_states,
        );
        let legacy_normal = legacy.resolve_all_slider_values_for_benchmark_with_states(
            &tree,
            now + Duration::from_millis(1),
            true,
            &WidgetStateMap::default(),
        );
        assert_eq!(direct_hovered, legacy_hovered);
        assert_eq!(direct_normal, legacy_normal);
    }

    #[test]
    fn slider_value_disabled_signal_fallback_recovers_when_enabled() {
        let view_model = ViewModelContext::for_benchmarks();
        let value = view_model.state(0.37_f32);
        let disabled = view_model.state(true);
        let tree: WidgetTree<()> = WidgetTree::new(
            Slider::new(value.signal(), 0.0, 1.0)
                .step(0.01)
                .disable(disabled.signal())
                .size(dp(220.0), dp(32.0)),
        );
        let viewport = Rect::new(0.0, 0.0, 240.0, 48.0);
        let now = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        super::super::resolved::with_legacy_slider_value_reactive_resolve(true, || {
            direct.run_layout_and_scene(&tree, now);
        });
        super::super::resolved::slider_value_direct_probe::reset();
        let direct_disabled = direct.resolve_all_slider_values_for_benchmark(&tree, now, false);
        assert_eq!(direct_disabled, vec![None]);
        assert_eq!(super::super::resolved::slider_value_direct_probe::hits(), 0);
        assert!(super::super::resolved::slider_value_direct_probe::prepared_fallbacks() >= 1);

        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_disabled = legacy.resolve_all_slider_values_for_benchmark(&tree, now, true);
        assert_eq!(direct_disabled, legacy_disabled);

        disabled.set(false);
        let transition_start = now + Duration::from_millis(1);
        let direct_transition =
            direct.resolve_all_slider_values_for_benchmark(&tree, transition_start, false);
        let legacy_transition =
            legacy.resolve_all_slider_values_for_benchmark(&tree, transition_start, true);
        assert_eq!(direct_transition, legacy_transition);
        let attempts_before_settle = super::super::resolved::slider_value_direct_probe::attempts();
        let settled = now + Duration::from_secs(1);
        let direct_enabled = direct.resolve_all_slider_values_for_benchmark(&tree, settled, false);
        let legacy_enabled = legacy.resolve_all_slider_values_for_benchmark(&tree, settled, true);
        assert_eq!(direct_enabled, legacy_enabled);
        assert!(matches!(direct_enabled.as_slice(), [Some(_)]));
        assert!(
            super::super::resolved::slider_value_direct_probe::attempts() > attempts_before_settle,
            "disabled fallback must remain retryable while state colors settle"
        );
        assert!(
            super::super::resolved::slider_value_direct_probe::hits() >= 1,
            "enabled Slider should recover to the direct resolver"
        );
    }

    #[test]
    fn animated_plain_container_opacity_multiframe_direct_matches_legacy_and_full_recollect() {
        const TRANSITION: Duration = Duration::from_millis(480);

        let view_model = ViewModelContext::for_benchmarks();
        let opacity = view_model.state(0.8_f32);
        let signal = opacity
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let background = Color::rgba(15, 23, 42, 255);
        let border = Color::rgba(14, 165, 233, 224);
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(100.0), dp(24.0))
            .gap(dp(1.0))
            .opacity(0.75);
        for _ in 0..4 {
            let signal = signal.clone();
            row = row.child(
                Stack::new()
                    .size(dp(24.0), dp(24.0))
                    .style(move |style, _| {
                        style.surface.background = Some(background.into());
                        style.surface.border_width = Some(dp(2.0).into());
                        style.surface.border_color = Some(border.into());
                        style.surface.border_radius = Some(dp(8.0).into());
                        style.surface.opacity = signal.clone().into();
                    }),
            );
        }
        let tree = WidgetTree::new(row);
        let viewport = Rect::new(0.0, 0.0, 100.0, 24.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_container_opacity_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_initial = direct.run_layout_and_scene(&tree, start);
        let legacy_initial = legacy.run_layout_and_scene(&tree, start);
        let full_initial = full.run_layout_and_scene(&tree, start);
        assert_eq!(direct_initial, legacy_initial);
        assert_eq!(direct_initial, full_initial);
        assert_plain_container_animation_scene_equivalent(&direct, &legacy, &full, Duration::ZERO);

        opacity.set(0.2);
        let animation_start = start + Duration::from_millis(1);
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);

        for (frame_offset, expected_opacity) in [
            (Duration::from_millis(120), 0.65_f32),
            (Duration::from_millis(240), 0.5_f32),
            (Duration::from_millis(440), 0.25_f32),
            (TRANSITION, 0.2_f32),
        ] {
            let frame = animation_start + frame_offset;
            let direct_stats = direct.run_layout_and_scene(&tree, frame);
            let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
            let full_stats = full.run_layout_and_scene(&tree, frame);
            assert_eq!(
                direct_stats, legacy_stats,
                "stats differ at {frame_offset:?}"
            );
            assert_eq!(direct_stats, full_stats, "stats differ at {frame_offset:?}");

            for (label, activity) in [
                ("direct", direct.animation_frame_activity()),
                ("legacy", legacy.animation_frame_activity()),
            ] {
                assert!(activity.refresh_changed, "{label} frame did not refresh");
                assert_eq!(activity.scene_widget_count, 4, "{label} target count");
                assert!(
                    activity.reactive_slot_write_succeeded,
                    "{label} positive-opacity frame should keep stable surface and hit topology"
                );
                assert!(activity.scene_patch_succeeded, "{label} scene patch failed");
                assert!(!activity.fell_back, "{label} frame fell back globally");
            }
            assert!(full.animation_frame_activity().full_scene_recollect);
            assert_plain_container_animation_scene_equivalent(
                &direct,
                &legacy,
                &full,
                frame_offset,
            );

            let shapes = &direct
                .cached_scene
                .as_ref()
                .expect("direct scene")
                .scene
                .shapes;
            assert_eq!(shapes.len(), 8, "four backgrounds and four borders");
            for shape in shapes {
                let source = if shape.stroke_width > 0.0 {
                    border
                } else {
                    background
                };
                assert_eq!(
                    shape.color,
                    source.with_alpha_factor(0.75 * expected_opacity),
                    "unexpected surface alpha at {frame_offset:?}"
                );
            }
        }
    }

    #[test]
    fn complex_container_opacity_multiframe_sticky_fallback_matches_full_recollect() {
        const TRANSITION: Duration = Duration::from_millis(480);

        let view_model = ViewModelContext::for_benchmarks();
        let opacity = view_model.state(0.8_f32);
        let signal = opacity
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let shadow_signal = signal.clone();
        let brush_signal = signal.clone();
        let background = Color::rgba(15, 23, 42, 255);
        let brush_color = Color::rgba(14, 165, 233, 224);
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::new(Axis::Horizontal)
                .size(dp(113.0), dp(40.0))
                .gap(dp(1.0))
                .opacity(0.75)
                .child(
                    Stack::new()
                        .size(dp(56.0), dp(40.0))
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.opacity = shadow_signal.clone().into();
                            style.surface.shadow = Some(
                                crate::ui::theme::Shadow {
                                    offset_x: dp(0.0),
                                    offset_y: dp(3.0),
                                    blur: dp(10.0),
                                    spread: dp(0.0),
                                    color: Color::rgba(15, 23, 42, 96),
                                }
                                .into(),
                            );
                        }),
                )
                .child(
                    Stack::new()
                        .size(dp(56.0), dp(40.0))
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.opacity = brush_signal.clone().into();
                            style.surface.background_brush =
                                Some(crate::ui::widget::BackgroundBrush::Solid(brush_color).into());
                        }),
                ),
        );
        let viewport = Rect::new(0.0, 0.0, 113.0, 48.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_container_opacity_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        // Keep the proof context out of the direct probe. Its measured frames below force a full
        // recollect, but the transition-start frame still runs the ordinary resolver pipeline.
        full.set_force_legacy_container_opacity_reactive_resolve(true);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);
        assert_plain_container_animation_scene_equivalent(&direct, &legacy, &full, Duration::ZERO);

        opacity.set(0.2);
        let animation_start = start + Duration::from_millis(1);
        super::super::resolved::container_opacity_direct_probe::reset();
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);

        for (frame_index, frame_offset) in [
            Duration::from_millis(120),
            Duration::from_millis(240),
            Duration::from_millis(440),
            TRANSITION,
        ]
        .into_iter()
        .enumerate()
        {
            let frame = animation_start + frame_offset;
            let direct_stats = direct.run_layout_and_scene(&tree, frame);
            let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
            let full_stats = full.run_layout_and_scene(&tree, frame);
            assert_eq!(
                direct_stats, legacy_stats,
                "direct/legacy stats differ at {frame_offset:?}"
            );
            assert_eq!(
                direct_stats, full_stats,
                "retained/full stats differ at {frame_offset:?}"
            );

            assert_eq!(
                super::super::resolved::container_opacity_direct_probe::attempts(),
                2,
                "complex candidates should be attempted only on the first frame"
            );
            assert_eq!(
                super::super::resolved::container_opacity_direct_probe::prepared_fallbacks(),
                2,
                "both complex surfaces should reuse their prepared runtime on first fallback"
            );
            assert_eq!(
                super::super::resolved::container_opacity_direct_probe::hits(),
                0,
                "shadow/brush surfaces are deliberately outside strict direct eligibility"
            );

            let activity = direct.animation_frame_activity();
            assert!(
                activity.refresh_changed,
                "frame {frame_index} did not refresh"
            );
            assert_eq!(activity.scene_widget_count, 2);
            assert!(activity.scene_patch_succeeded);
            assert!(
                !activity.fell_back,
                "frame {frame_index} fell back globally"
            );
            assert!(full.animation_frame_activity().full_scene_recollect);
            assert_plain_container_animation_scene_equivalent(
                &direct,
                &legacy,
                &full,
                frame_offset,
            );

            let scene = &direct.cached_scene.as_ref().expect("direct scene").scene;
            assert_eq!(scene.brushes.len(), 1, "brush topology changed");
            assert_eq!(scene.textures.len(), 1, "shadow topology changed");
        }
    }

    #[test]
    fn animated_plain_container_background_direct_scene_matches_legacy_and_full_recollect() {
        let view_model = ViewModelContext::for_benchmarks();
        let background = view_model.state(Color::rgba(14, 165, 233, 224));
        let signal = background
            .signal()
            .animated(crate::animation::Transition::linear(Duration::from_millis(
                480,
            )));
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(100.0), dp(24.0))
            .gap(dp(1.0));
        for _ in 0..4 {
            let signal = signal.clone();
            row = row.child(
                Stack::new()
                    .size(dp(24.0), dp(24.0))
                    .style(move |style, _| {
                        style.surface.background = Some(signal.clone().into());
                    }),
            );
        }
        let tree = WidgetTree::new(row);
        let viewport = Rect::new(0.0, 0.0, 100.0, 24.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_background_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);

        background.set(Color::rgba(244, 63, 94, 176));
        let animation_start = start + Duration::from_millis(1);
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);
        let frame = animation_start + Duration::from_millis(120);
        let direct_stats = direct.run_layout_and_scene(&tree, frame);
        let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
        let full_stats = full.run_layout_and_scene(&tree, frame);
        assert_eq!(direct_stats, legacy_stats);
        assert_eq!(direct_stats, full_stats);
        assert!(
            direct
                .animation_frame_activity()
                .reactive_slot_write_succeeded
        );
        assert!(
            legacy
                .animation_frame_activity()
                .reactive_slot_write_succeeded
        );
        assert!(full.animation_frame_activity().full_scene_recollect);

        let shape_snapshot = |context: &WidgetBenchmarkContext| {
            context
                .cached_scene
                .as_ref()
                .expect("cached scene")
                .scene
                .shapes
                .iter()
                .map(|shape| {
                    (
                        shape.rect,
                        shape.color,
                        shape.corner_radius,
                        shape.stroke_width,
                        shape.clip_rect,
                        shape.clip_mask,
                    )
                })
                .collect::<Vec<_>>()
        };
        let direct_shapes = shape_snapshot(&direct);
        assert_eq!(direct_shapes, shape_snapshot(&legacy));
        assert_eq!(direct_shapes, shape_snapshot(&full));
        let hit_snapshot = |context: &WidgetBenchmarkContext| {
            context
                .cached_scene
                .as_ref()
                .expect("cached scene")
                .hit_regions
                .iter()
                .map(|hit| (hit.rect, hit.clip_rect))
                .collect::<Vec<_>>()
        };
        let direct_hits = hit_snapshot(&direct);
        assert_eq!(direct_hits, hit_snapshot(&legacy));
        assert_eq!(direct_hits, hit_snapshot(&full));
    }

    #[test]
    fn animated_plain_container_border_color_direct_scene_matches_legacy_and_full_recollect() {
        let view_model = ViewModelContext::for_benchmarks();
        let border = view_model.state(Color::rgba(14, 165, 233, 224));
        let signal = border
            .signal()
            .animated(crate::animation::Transition::linear(Duration::from_millis(
                480,
            )));
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(100.0), dp(24.0))
            .gap(dp(1.0));
        for _ in 0..4 {
            let signal = signal.clone();
            row = row.child(
                Stack::new()
                    .size(dp(24.0), dp(24.0))
                    .style(move |style, _| {
                        style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                        style.surface.border_width = Some(dp(1.0).into());
                        style.surface.border_color = Some(signal.clone().into());
                    }),
            );
        }
        let tree = WidgetTree::new(row);
        let viewport = Rect::new(0.0, 0.0, 100.0, 24.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_border_color_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);

        border.set(Color::rgba(244, 63, 94, 176));
        let animation_start = start + Duration::from_millis(1);
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);
        let frame = animation_start + Duration::from_millis(120);
        let direct_stats = direct.run_layout_and_scene(&tree, frame);
        let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
        let full_stats = full.run_layout_and_scene(&tree, frame);
        assert_eq!(direct_stats, legacy_stats);
        assert_eq!(direct_stats, full_stats);
        assert!(
            direct
                .animation_frame_activity()
                .reactive_slot_write_succeeded
        );
        assert!(
            legacy
                .animation_frame_activity()
                .reactive_slot_write_succeeded
        );
        assert!(full.animation_frame_activity().full_scene_recollect);

        let shape_snapshot = |context: &WidgetBenchmarkContext| {
            context
                .cached_scene
                .as_ref()
                .expect("cached scene")
                .scene
                .shapes
                .iter()
                .map(|shape| {
                    (
                        shape.rect,
                        shape.color,
                        shape.corner_radius,
                        shape.stroke_width,
                        shape.clip_rect,
                        shape.clip_mask,
                    )
                })
                .collect::<Vec<_>>()
        };
        let direct_shapes = shape_snapshot(&direct);
        assert_eq!(direct_shapes, shape_snapshot(&legacy));
        assert_eq!(direct_shapes, shape_snapshot(&full));
        let hit_snapshot = |context: &WidgetBenchmarkContext| {
            context
                .cached_scene
                .as_ref()
                .expect("cached scene")
                .hit_regions
                .iter()
                .map(|hit| (hit.rect, hit.clip_rect))
                .collect::<Vec<_>>()
        };
        let direct_hits = hit_snapshot(&direct);
        assert_eq!(direct_hits, hit_snapshot(&legacy));
        assert_eq!(direct_hits, hit_snapshot(&full));
    }

    #[test]
    fn complex_container_border_color_multiframe_sticky_fallback_matches_full_recollect() {
        const TRANSITION: Duration = Duration::from_millis(480);

        let view_model = ViewModelContext::for_benchmarks();
        let border = view_model.state(Color::rgba(14, 165, 233, 224));
        let signal = border
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let shadow_signal = signal.clone();
        let brush_signal = signal.clone();
        let background = Color::rgba(15, 23, 42, 255);
        let brush_color = Color::rgba(99, 102, 241, 160);
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::new(Axis::Horizontal)
                .size(dp(113.0), dp(40.0))
                .gap(dp(1.0))
                .opacity(0.75)
                .child(
                    Stack::new()
                        .size(dp(56.0), dp(40.0))
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.border_width = Some(dp(2.0).into());
                            style.surface.border_color = Some(shadow_signal.clone().into());
                            style.surface.shadow = Some(
                                crate::ui::theme::Shadow {
                                    offset_x: dp(0.0),
                                    offset_y: dp(3.0),
                                    blur: dp(10.0),
                                    spread: dp(0.0),
                                    color: Color::rgba(15, 23, 42, 96),
                                }
                                .into(),
                            );
                        }),
                )
                .child(
                    Stack::new()
                        .size(dp(56.0), dp(40.0))
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.border_width = Some(dp(2.0).into());
                            style.surface.border_color = Some(brush_signal.clone().into());
                            style.surface.background_brush =
                                Some(crate::ui::widget::BackgroundBrush::Solid(brush_color).into());
                        }),
                ),
        );
        let viewport = Rect::new(0.0, 0.0, 113.0, 48.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_border_color_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        full.set_force_legacy_border_color_reactive_resolve(true);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);
        assert_plain_container_animation_scene_equivalent(&direct, &legacy, &full, Duration::ZERO);

        border.set(Color::rgba(244, 63, 94, 176));
        let animation_start = start + Duration::from_millis(1);
        super::super::resolved::border_color_direct_probe::reset();
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);

        for (frame_index, frame_offset) in [
            Duration::from_millis(120),
            Duration::from_millis(240),
            Duration::from_millis(440),
            TRANSITION,
        ]
        .into_iter()
        .enumerate()
        {
            let frame = animation_start + frame_offset;
            let direct_stats = direct.run_layout_and_scene(&tree, frame);
            let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
            let full_stats = full.run_layout_and_scene(&tree, frame);
            assert_eq!(
                direct_stats, legacy_stats,
                "direct/legacy stats differ at {frame_offset:?}"
            );
            assert_eq!(
                direct_stats, full_stats,
                "retained/full stats differ at {frame_offset:?}"
            );

            assert_eq!(
                super::super::resolved::border_color_direct_probe::attempts(),
                2,
                "complex candidates should be attempted only at transition start"
            );
            assert_eq!(
                super::super::resolved::border_color_direct_probe::prepared_fallbacks(),
                2,
                "both complex surfaces should reuse their prepared runtime"
            );
            assert_eq!(
                super::super::resolved::border_color_direct_probe::hits(),
                0,
                "shadow/brush surfaces are deliberately outside strict direct eligibility"
            );

            let activity = direct.animation_frame_activity();
            assert!(
                activity.refresh_changed,
                "frame {frame_index} did not refresh"
            );
            assert_eq!(activity.scene_widget_count, 2);
            assert!(activity.reactive_slot_write_succeeded);
            assert!(activity.scene_patch_succeeded);
            assert!(
                !activity.fell_back,
                "frame {frame_index} fell back globally"
            );
            assert!(full.animation_frame_activity().full_scene_recollect);
            assert_plain_container_animation_scene_equivalent(
                &direct,
                &legacy,
                &full,
                frame_offset,
            );

            let scene = &direct.cached_scene.as_ref().expect("direct scene").scene;
            assert_eq!(scene.brushes.len(), 1, "brush topology changed");
            assert_eq!(scene.textures.len(), 1, "shadow topology changed");
        }
    }

    fn assert_plain_container_animation_scene_equivalent(
        direct: &WidgetBenchmarkContext,
        legacy: &WidgetBenchmarkContext,
        full: &WidgetBenchmarkContext,
        frame: Duration,
    ) {
        let shape_snapshot = |context: &WidgetBenchmarkContext| {
            context
                .cached_scene
                .as_ref()
                .expect("cached scene")
                .scene
                .shapes
                .iter()
                .map(|shape| {
                    (
                        shape.rect,
                        shape.color,
                        shape.corner_radius,
                        shape.stroke_width,
                        shape.clip_rect,
                        shape.clip_mask,
                    )
                })
                .collect::<Vec<_>>()
        };
        let direct_shapes = shape_snapshot(direct);
        assert_eq!(
            direct_shapes,
            shape_snapshot(legacy),
            "direct and legacy scenes differ at {frame:?}"
        );
        assert_eq!(
            direct_shapes,
            shape_snapshot(full),
            "retained and full-recollected scenes differ at {frame:?}"
        );

        let brush_snapshot = |context: &WidgetBenchmarkContext| {
            context
                .cached_scene
                .as_ref()
                .expect("cached scene")
                .scene
                .brushes
                .iter()
                .cloned()
                .collect::<Vec<_>>()
        };
        let direct_brushes = brush_snapshot(direct);
        assert_eq!(
            direct_brushes,
            brush_snapshot(legacy),
            "direct and legacy brushes differ at {frame:?}"
        );
        assert_eq!(
            direct_brushes,
            brush_snapshot(full),
            "retained and full-recollected brushes differ at {frame:?}"
        );

        let texture_snapshot = |context: &WidgetBenchmarkContext| {
            context
                .cached_scene
                .as_ref()
                .expect("cached scene")
                .scene
                .textures
                .iter()
                .map(|texture| {
                    (
                        texture.frame,
                        texture.quad,
                        texture.uv_rect,
                        texture.corner_radius,
                        texture.opacity,
                        texture.clip_rect,
                        texture.clip_mask,
                        texture.mask_tint,
                    )
                })
                .collect::<Vec<_>>()
        };
        let direct_textures = texture_snapshot(direct);
        assert_eq!(
            direct_textures,
            texture_snapshot(legacy),
            "direct and legacy textures differ at {frame:?}"
        );
        assert_eq!(
            direct_textures,
            texture_snapshot(full),
            "retained and full-recollected textures differ at {frame:?}"
        );

        let text_snapshot = |context: &WidgetBenchmarkContext| {
            context
                .cached_scene
                .as_ref()
                .expect("cached scene")
                .scene
                .texts
                .iter()
                .cloned()
                .collect::<Vec<_>>()
        };
        let direct_texts = text_snapshot(direct);
        assert_eq!(
            direct_texts,
            text_snapshot(legacy),
            "direct and legacy texts differ at {frame:?}"
        );
        assert_eq!(
            direct_texts,
            text_snapshot(full),
            "retained and full-recollected texts differ at {frame:?}"
        );

        let command_snapshot = |context: &WidgetBenchmarkContext| {
            let scene = &context.cached_scene.as_ref().expect("cached scene").scene;
            let kind = |command: &crate::ui::widget::common::RenderCommand| match command {
                crate::ui::widget::common::RenderCommand::BackdropBlur(_) => 0_u8,
                crate::ui::widget::common::RenderCommand::Brush(_) => 1,
                crate::ui::widget::common::RenderCommand::CanvasComposite(_) => 2,
                crate::ui::widget::common::RenderCommand::Shape(_) => 3,
                crate::ui::widget::common::RenderCommand::Texture(_) => 4,
                #[cfg(feature = "video")]
                crate::ui::widget::common::RenderCommand::VideoTexture(_) => 5,
                crate::ui::widget::common::RenderCommand::Text(_) => 6,
                crate::ui::widget::common::RenderCommand::TextDecoration(_) => 7,
                crate::ui::widget::common::RenderCommand::Mesh(_) => 8,
            };
            (
                scene.commands.iter().map(kind).collect::<Vec<_>>(),
                scene.overlay_commands.iter().map(kind).collect::<Vec<_>>(),
            )
        };
        let direct_commands = command_snapshot(direct);
        assert_eq!(
            direct_commands,
            command_snapshot(legacy),
            "direct and legacy command streams differ at {frame:?}"
        );
        assert_eq!(
            direct_commands,
            command_snapshot(full),
            "retained and full-recollected command streams differ at {frame:?}"
        );

        let hit_snapshot = |context: &WidgetBenchmarkContext| {
            context
                .cached_scene
                .as_ref()
                .expect("cached scene")
                .hit_regions
                .iter()
                .map(|hit| (hit.rect, hit.clip_rect))
                .collect::<Vec<_>>()
        };
        let direct_hits = hit_snapshot(direct);
        assert_eq!(
            direct_hits,
            hit_snapshot(legacy),
            "direct and legacy hit scenes differ at {frame:?}"
        );
        assert_eq!(
            direct_hits,
            hit_snapshot(full),
            "retained and full-recollected hit scenes differ at {frame:?}"
        );
    }

    #[test]
    fn animated_plain_container_border_radius_multiframe_direct_matches_legacy_and_full_recollect()
    {
        const TRANSITION: Duration = Duration::from_millis(480);
        const BORDER_WIDTH: f32 = 2.0;

        let view_model = ViewModelContext::for_benchmarks();
        let radius = view_model.state(dp(24.0));
        let signal = radius
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(100.0), dp(24.0))
            .gap(dp(1.0))
            .opacity(0.75);
        for _ in 0..4 {
            let signal = signal.clone();
            row = row.child(
                Stack::new()
                    .size(dp(24.0), dp(24.0))
                    .style(move |style, _| {
                        style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                        style.surface.border_width = Some(dp(BORDER_WIDTH).into());
                        style.surface.border_color = Some(Color::rgba(14, 165, 233, 224).into());
                        style.surface.border_radius = Some(signal.clone().into());
                    }),
            );
        }
        let tree = WidgetTree::new(row);
        let viewport = Rect::new(0.0, 0.0, 100.0, 24.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_border_radius_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_initial = direct.run_layout_and_scene(&tree, start);
        let legacy_initial = legacy.run_layout_and_scene(&tree, start);
        let full_initial = full.run_layout_and_scene(&tree, start);
        assert_eq!(direct_initial, legacy_initial);
        assert_eq!(direct_initial, full_initial);
        assert_plain_container_animation_scene_equivalent(&direct, &legacy, &full, Duration::ZERO);

        radius.set(dp(0.0));
        let animation_start = start + Duration::from_millis(1);
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);

        for (frame_offset, expected_radius) in [
            (Duration::from_millis(120), 18.0_f32),
            (Duration::from_millis(240), 12.0_f32),
            (Duration::from_millis(440), 2.0_f32),
            (TRANSITION, 0.0_f32),
        ] {
            let frame = animation_start + frame_offset;
            let direct_stats = direct.run_layout_and_scene(&tree, frame);
            let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
            let full_stats = full.run_layout_and_scene(&tree, frame);
            assert_eq!(
                direct_stats, legacy_stats,
                "stats differ at {frame_offset:?}"
            );
            assert_eq!(direct_stats, full_stats, "stats differ at {frame_offset:?}");

            for (label, activity) in [
                ("direct", direct.animation_frame_activity()),
                ("legacy", legacy.animation_frame_activity()),
            ] {
                assert!(activity.refresh_changed, "{label} frame did not refresh");
                assert_eq!(activity.scene_widget_count, 4, "{label} target count");
                assert!(
                    activity.reactive_slot_write_succeeded,
                    "{label} radius frame should retain its stable two-shape topology"
                );
                assert!(activity.scene_patch_succeeded, "{label} scene patch failed");
                assert!(!activity.fell_back, "{label} frame fell back globally");
            }
            assert!(full.animation_frame_activity().full_scene_recollect);
            assert_plain_container_animation_scene_equivalent(
                &direct,
                &legacy,
                &full,
                frame_offset,
            );

            let shapes = &direct
                .cached_scene
                .as_ref()
                .expect("direct scene")
                .scene
                .shapes;
            assert_eq!(shapes.len(), 8, "radius must preserve background + border");
            for shape in shapes {
                let expected = if shape.stroke_width > 0.0 {
                    expected_radius
                } else {
                    (expected_radius - BORDER_WIDTH).max(0.0)
                };
                assert!(
                    (shape.corner_radius - expected).abs() <= 1.0e-4,
                    "unexpected radius at {frame_offset:?}: actual={}, expected={expected}",
                    shape.corner_radius
                );
                let expected_color = if shape.stroke_width > 0.0 {
                    Color::rgba(14, 165, 233, 224).with_alpha_factor(0.75)
                } else {
                    Color::rgba(15, 23, 42, 255).with_alpha_factor(0.75)
                };
                assert_eq!(shape.color, expected_color);
            }
        }
    }

    #[test]
    fn complex_container_border_radius_multiframe_sticky_fallback_matches_full_recollect() {
        const TRANSITION: Duration = Duration::from_millis(480);

        let view_model = ViewModelContext::for_benchmarks();
        let radius = view_model.state(dp(24.0));
        let signal = radius
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let shadow_signal = signal.clone();
        let brush_signal = signal.clone();
        let background = Color::rgba(15, 23, 42, 255);
        let border = Color::rgba(14, 165, 233, 224);
        let brush_color = Color::rgba(99, 102, 241, 160);
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::new(Axis::Horizontal)
                .size(dp(113.0), dp(40.0))
                .gap(dp(1.0))
                .opacity(0.75)
                .child(
                    Stack::new()
                        .size(dp(56.0), dp(40.0))
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.border_width = Some(dp(2.0).into());
                            style.surface.border_color = Some(border.into());
                            style.surface.border_radius = Some(shadow_signal.clone().into());
                            style.surface.shadow = Some(
                                crate::ui::theme::Shadow {
                                    offset_x: dp(0.0),
                                    offset_y: dp(3.0),
                                    blur: dp(10.0),
                                    spread: dp(0.0),
                                    color: Color::rgba(15, 23, 42, 96),
                                }
                                .into(),
                            );
                        }),
                )
                .child(
                    Stack::new()
                        .size(dp(56.0), dp(40.0))
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.border_width = Some(dp(2.0).into());
                            style.surface.border_color = Some(border.into());
                            style.surface.border_radius = Some(brush_signal.clone().into());
                            style.surface.background_brush =
                                Some(crate::ui::widget::BackgroundBrush::Solid(brush_color).into());
                        }),
                ),
        );
        let viewport = Rect::new(0.0, 0.0, 113.0, 48.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_border_radius_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        full.set_force_legacy_border_radius_reactive_resolve(true);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);
        assert_plain_container_animation_scene_equivalent(&direct, &legacy, &full, Duration::ZERO);

        radius.set(dp(0.0));
        let animation_start = start + Duration::from_millis(1);
        super::super::resolved::border_radius_direct_probe::reset();
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);

        for (frame_index, frame_offset) in [
            Duration::from_millis(120),
            Duration::from_millis(240),
            Duration::from_millis(440),
            TRANSITION,
        ]
        .into_iter()
        .enumerate()
        {
            let frame = animation_start + frame_offset;
            let direct_stats = direct.run_layout_and_scene(&tree, frame);
            let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
            let full_stats = full.run_layout_and_scene(&tree, frame);
            assert_eq!(
                direct_stats, legacy_stats,
                "direct/legacy stats differ at {frame_offset:?}"
            );
            assert_eq!(
                direct_stats, full_stats,
                "retained/full stats differ at {frame_offset:?}"
            );

            assert_eq!(
                super::super::resolved::border_radius_direct_probe::attempts(),
                2,
                "complex candidates should be attempted only at transition start"
            );
            assert_eq!(
                super::super::resolved::border_radius_direct_probe::prepared_fallbacks(),
                2,
                "both complex surfaces should reuse their prepared runtime"
            );
            assert_eq!(
                super::super::resolved::border_radius_direct_probe::hits(),
                0,
                "shadow/brush surfaces are deliberately outside strict direct eligibility"
            );

            let activity = direct.animation_frame_activity();
            assert!(
                activity.refresh_changed,
                "frame {frame_index} did not refresh"
            );
            assert_eq!(activity.scene_widget_count, 2);
            assert!(!activity.reactive_slot_write_succeeded);
            assert!(activity.scene_patch_succeeded);
            assert!(
                !activity.fell_back,
                "frame {frame_index} fell back globally"
            );
            assert!(full.animation_frame_activity().full_scene_recollect);
            assert_plain_container_animation_scene_equivalent(
                &direct,
                &legacy,
                &full,
                frame_offset,
            );

            let scene = &direct.cached_scene.as_ref().expect("direct scene").scene;
            assert_eq!(scene.brushes.len(), 1, "brush topology changed");
            assert_eq!(scene.textures.len(), 1, "shadow topology changed");
        }
    }

    #[test]
    fn animated_plain_container_border_width_multiframe_direct_matches_legacy_and_full_recollect() {
        const TRANSITION: Duration = Duration::from_millis(560);
        const BORDER_RADIUS: f32 = 10.0;

        let view_model = ViewModelContext::for_benchmarks();
        let width = view_model.state(dp(14.0));
        let signal = width
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(100.0), dp(24.0))
            .gap(dp(1.0))
            .opacity(0.75);
        for _ in 0..4 {
            let signal = signal.clone();
            row = row.child(
                Stack::new()
                    .size(dp(24.0), dp(24.0))
                    .style(move |style, _| {
                        style.surface.background = Some(Color::rgba(15, 23, 42, 255).into());
                        style.surface.border_width = Some(signal.clone().into());
                        style.surface.border_color = Some(Color::rgba(14, 165, 233, 224).into());
                        style.surface.border_radius = Some(dp(BORDER_RADIUS).into());
                    }),
            );
        }
        let tree = WidgetTree::new(row);
        let viewport = Rect::new(0.0, 0.0, 100.0, 24.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_border_width_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_initial = direct.run_layout_and_scene(&tree, start);
        let legacy_initial = legacy.run_layout_and_scene(&tree, start);
        let full_initial = full.run_layout_and_scene(&tree, start);
        assert_eq!(direct_initial, legacy_initial);
        assert_eq!(direct_initial, full_initial);
        assert_plain_container_animation_scene_equivalent(&direct, &legacy, &full, Duration::ZERO);

        width.set(dp(0.0));
        let animation_start = start + Duration::from_millis(1);
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);

        for (frame_offset, expected_width, expected_slot_write, expected_shape_count) in [
            (Duration::from_millis(80), 12.0_f32, true, 4_usize),
            (Duration::from_millis(160), 10.0_f32, false, 8_usize),
            (Duration::from_millis(480), 2.0_f32, true, 8_usize),
            (TRANSITION, 0.0_f32, false, 4_usize),
        ] {
            let frame = animation_start + frame_offset;
            let direct_stats = direct.run_layout_and_scene(&tree, frame);
            let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
            let full_stats = full.run_layout_and_scene(&tree, frame);
            assert_eq!(
                direct_stats, legacy_stats,
                "stats differ at {frame_offset:?}"
            );
            assert_eq!(direct_stats, full_stats, "stats differ at {frame_offset:?}");

            for (label, activity) in [
                ("direct", direct.animation_frame_activity()),
                ("legacy", legacy.animation_frame_activity()),
            ] {
                assert!(activity.refresh_changed, "{label} frame did not refresh");
                assert_eq!(activity.scene_widget_count, 4, "{label} target count");
                assert_eq!(
                    activity.reactive_slot_write_succeeded, expected_slot_write,
                    "{label} slot path mismatch at {frame_offset:?}"
                );
                assert!(
                    activity.scene_patch_succeeded,
                    "{label} must either write retained slots or safely patch the bounded subtree"
                );
                assert!(!activity.fell_back, "{label} frame fell back globally");
            }
            assert!(full.animation_frame_activity().full_scene_recollect);
            assert_plain_container_animation_scene_equivalent(
                &direct,
                &legacy,
                &full,
                frame_offset,
            );

            let shapes = &direct
                .cached_scene
                .as_ref()
                .expect("direct scene")
                .scene
                .shapes;
            assert_eq!(shapes.len(), expected_shape_count);
            let borders = shapes
                .iter()
                .filter(|shape| shape.stroke_width > 0.0)
                .collect::<Vec<_>>();
            let backgrounds = shapes
                .iter()
                .filter(|shape| shape.stroke_width == 0.0)
                .collect::<Vec<_>>();
            assert_eq!(borders.len(), usize::from(expected_width > 0.0) * 4);
            assert_eq!(
                backgrounds.len(),
                usize::from(expected_width < 12.0) * 4,
                "the inset background appears only below the half-size clamp boundary"
            );
            for border in borders {
                assert!((border.stroke_width - expected_width).abs() <= 1.0e-4);
                assert!((border.corner_radius - BORDER_RADIUS).abs() <= 1.0e-4);
                assert_eq!(
                    border.color,
                    Color::rgba(14, 165, 233, 224).with_alpha_factor(0.75)
                );
            }
            for background in backgrounds {
                assert!(
                    (background.corner_radius - (BORDER_RADIUS - expected_width).max(0.0)).abs()
                        <= 1.0e-4
                );
                assert_eq!(
                    background.color,
                    Color::rgba(15, 23, 42, 255).with_alpha_factor(0.75)
                );
            }
        }
    }

    #[test]
    fn complex_container_border_width_multiframe_sticky_fallback_matches_full_recollect() {
        const TRANSITION: Duration = Duration::from_millis(480);

        let view_model = ViewModelContext::for_benchmarks();
        let width = view_model.state(dp(8.0));
        let signal = width
            .signal()
            .animated(crate::animation::Transition::linear(TRANSITION));
        let shadow_signal = signal.clone();
        let brush_signal = signal.clone();
        let background = Color::rgba(15, 23, 42, 255);
        let border = Color::rgba(14, 165, 233, 224);
        let brush_color = Color::rgba(99, 102, 241, 160);
        let tree: WidgetTree<()> = WidgetTree::new(
            Flex::new(Axis::Horizontal)
                .size(dp(113.0), dp(40.0))
                .gap(dp(1.0))
                .opacity(0.75)
                .child(
                    Stack::new()
                        .size(dp(56.0), dp(40.0))
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.border_width = Some(shadow_signal.clone().into());
                            style.surface.border_color = Some(border.into());
                            style.surface.border_radius = Some(dp(10.0).into());
                            style.surface.shadow = Some(
                                crate::ui::theme::Shadow {
                                    offset_x: dp(0.0),
                                    offset_y: dp(3.0),
                                    blur: dp(10.0),
                                    spread: dp(0.0),
                                    color: Color::rgba(15, 23, 42, 96),
                                }
                                .into(),
                            );
                        }),
                )
                .child(
                    Stack::new()
                        .size(dp(56.0), dp(40.0))
                        .style(move |style, _| {
                            style.surface.background = Some(background.into());
                            style.surface.border_width = Some(brush_signal.clone().into());
                            style.surface.border_color = Some(border.into());
                            style.surface.border_radius = Some(dp(10.0).into());
                            style.surface.background_brush =
                                Some(crate::ui::widget::BackgroundBrush::Solid(brush_color).into());
                        }),
                ),
        );
        let viewport = Rect::new(0.0, 0.0, 113.0, 48.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_border_width_reactive_resolve(true);
        let mut full = WidgetBenchmarkContext::default().with_viewport(viewport);
        full.set_force_legacy_border_width_reactive_resolve(true);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);
        full.run_layout_and_scene(&tree, start);
        assert_plain_container_animation_scene_equivalent(&direct, &legacy, &full, Duration::ZERO);

        width.set(dp(2.0));
        let animation_start = start + Duration::from_millis(1);
        super::super::resolved::border_width_direct_probe::reset();
        for context in [&mut direct, &mut legacy, &mut full] {
            context.invalidate_all();
            context.run_layout_and_scene(&tree, animation_start);
        }
        full.set_force_full_scene_animation_recollect(true);

        for (frame_index, frame_offset) in [
            Duration::from_millis(120),
            Duration::from_millis(240),
            Duration::from_millis(440),
            TRANSITION,
        ]
        .into_iter()
        .enumerate()
        {
            let frame = animation_start + frame_offset;
            let direct_stats = direct.run_layout_and_scene(&tree, frame);
            let legacy_stats = legacy.run_layout_and_scene(&tree, frame);
            let full_stats = full.run_layout_and_scene(&tree, frame);
            assert_eq!(
                direct_stats, legacy_stats,
                "direct/legacy stats differ at {frame_offset:?}"
            );
            assert_eq!(
                direct_stats, full_stats,
                "retained/full stats differ at {frame_offset:?}"
            );

            assert_eq!(
                super::super::resolved::border_width_direct_probe::attempts(),
                2,
                "complex candidates should be attempted only at transition start"
            );
            assert_eq!(
                super::super::resolved::border_width_direct_probe::prepared_fallbacks(),
                2,
                "both complex surfaces should reuse their prepared runtime"
            );
            assert_eq!(
                super::super::resolved::border_width_direct_probe::hits(),
                0,
                "shadow/brush surfaces are deliberately outside strict direct eligibility"
            );

            let activity = direct.animation_frame_activity();
            assert!(
                activity.refresh_changed,
                "frame {frame_index} did not refresh"
            );
            assert_eq!(activity.scene_widget_count, 2);
            assert!(!activity.reactive_slot_write_succeeded);
            assert!(activity.scene_patch_succeeded);
            assert!(
                !activity.fell_back,
                "frame {frame_index} fell back globally"
            );
            assert!(full.animation_frame_activity().full_scene_recollect);
            assert_plain_container_animation_scene_equivalent(
                &direct,
                &legacy,
                &full,
                frame_offset,
            );

            let scene = &direct.cached_scene.as_ref().expect("direct scene").scene;
            assert_eq!(scene.brushes.len(), 1, "brush topology changed");
            assert_eq!(scene.textures.len(), 1, "shadow topology changed");
        }
    }

    #[test]
    fn fixed_text_content_direct_resolve_matches_legacy_and_text_editor_falls_back() {
        let fixed_text: WidgetTree<()> =
            WidgetTree::new(Text::new("Frame 111111").size(dp(180.0), dp(20.0)));
        let input: WidgetTree<()> = WidgetTree::new(
            Input::new(TextController::new_legacy("Editor fallback")).size(dp(180.0), dp(32.0)),
        );
        let viewport = Rect::new(0.0, 0.0, 220.0, 80.0);
        let now = Instant::now();

        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_text = direct.resolve_first_text_content_for_test(&fixed_text, now, false);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_text = legacy.resolve_first_text_content_for_test(&fixed_text, now, true);
        assert_eq!(direct_text, legacy_text);
        assert!(matches!(
            direct_text,
            Some(ReactiveScenePropertyValue::TextContent { ref content, .. })
                if content.as_ref() == "Frame 111111"
        ));

        let mut direct_input = WidgetBenchmarkContext::default().with_viewport(viewport);
        let direct_input_value =
            direct_input.resolve_first_text_content_for_test(&input, now, false);
        let mut legacy_input = WidgetBenchmarkContext::default().with_viewport(viewport);
        let legacy_input_value =
            legacy_input.resolve_first_text_content_for_test(&input, now, true);
        assert_eq!(direct_input_value, legacy_input_value);
        assert!(matches!(
            direct_input_value,
            Some(ReactiveScenePropertyValue::TextInputContent(_))
        ));
    }

    #[test]
    fn animated_texture_mask_tint_direct_cpu_frame_matches_legacy() {
        let view_model = ViewModelContext::for_benchmarks();
        let color = view_model.state(Color::rgba(14, 165, 233, 224));
        let signal = color
            .signal()
            .animated(crate::animation::Transition::linear(Duration::from_millis(
                480,
            )));
        let mut row = Flex::<()>::new(Axis::Horizontal)
            .size(dp(96.0), dp(20.0))
            .gap(dp(4.0));
        for _ in 0..4 {
            let signal = signal.clone();
            row = row.child(
                Icon::monochrome_svg(MONOCHROME_ICON_SVG)
                    .size(dp(20.0))
                    .style(move |style, _| style.color = signal.clone().into()),
            );
        }
        let tree = WidgetTree::new(row);
        let viewport = Rect::new(0.0, 0.0, 100.0, 24.0);
        let start = Instant::now();
        let mut direct = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut legacy = WidgetBenchmarkContext::default().with_viewport(viewport);
        legacy.set_force_legacy_texture_mask_tint_reactive_resolve(true);
        direct.run_layout_and_scene(&tree, start);
        legacy.run_layout_and_scene(&tree, start);

        color.set(Color::rgba(244, 63, 94, 176));
        let animation_start = start + Duration::from_millis(1);
        direct.invalidate_all();
        legacy.invalidate_all();
        direct.run_layout_and_scene(&tree, animation_start);
        legacy.run_layout_and_scene(&tree, animation_start);

        let frame = animation_start + Duration::from_millis(120);
        direct.run_layout_and_scene(&tree, frame);
        legacy.run_layout_and_scene(&tree, frame);
        for activity in [
            direct.animation_frame_activity(),
            legacy.animation_frame_activity(),
        ] {
            assert!(activity.refresh_changed);
            assert_eq!(activity.scene_widget_count, 4);
            assert!(activity.scene_patch_succeeded);
            assert!(activity.reactive_slot_write_succeeded);
            assert!(!activity.fell_back);
        }
        let direct_state = direct
            .cached_texture_retained_state()
            .expect("direct CPU tint state");
        let legacy_state = legacy
            .cached_texture_retained_state()
            .expect("legacy CPU tint state");
        assert_eq!(direct_state.frames, legacy_state.frames);
        assert_eq!(direct_state.mask_tints, legacy_state.mask_tints);
        assert_eq!(
            direct_state.texture_revisions,
            legacy_state.texture_revisions
        );
        assert_eq!(
            direct_state.media_key_fingerprints,
            legacy_state.media_key_fingerprints
        );
        assert_eq!(
            direct_state.media_layout_fingerprints,
            legacy_state.media_layout_fingerprints
        );
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn paragraph_mask_tint_matches_baked_rgba_with_clip_transform_and_batching() {
        fn colored_text(content: &'static str, color: Color) -> Text {
            Text::new(content)
                .width(dp(100.0))
                .height(dp(36.0))
                .style(move |style, _| style.color = color.into())
        }

        let tree = WidgetTree::new(
            Flex::new(Axis::Horizontal)
                .size(dp(190.0), dp(36.0))
                .overflow(Overflow::Hidden)
                .offset(Point::new(7.0, 5.0))
                .child(colored_text(
                    "shared identity",
                    Color::rgba(25, 110, 230, 217),
                ))
                .child(colored_text(
                    "shared identity",
                    Color::rgba(238, 74, 96, 113),
                )),
        );
        let viewport = Rect::new(0.0, 0.0, 220.0, 60.0);
        let mut optimized = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut baked = WidgetBenchmarkContext::default().with_viewport(viewport);
        if optimized.initialize_headless_gpu().is_err() || baked.initialize_headless_gpu().is_err()
        {
            eprintln!("skipping paragraph mask GPU readback test: no headless adapter");
            return;
        }
        assert!(baked.set_headless_text_mask_tint(false));
        optimized
            .render_cached_scene_to_headless_gpu(&tree, Instant::now())
            .expect("render mask+tint output");
        baked
            .render_cached_scene_to_headless_gpu(&tree, Instant::now())
            .expect("render baked RGBA output");

        let optimized_cache = optimized
            .headless_text_gpu_cache_stats()
            .expect("optimized cache stats");
        let baked_cache = baked
            .headless_text_gpu_cache_stats()
            .expect("baked cache stats");
        assert_eq!(optimized_cache.cache_entries, 1);
        assert_eq!(baked_cache.cache_entries, 2);
        let optimized_draws = optimized
            .headless_gpu_draw_stats()
            .expect("optimized draw stats");
        assert_eq!(optimized_draws.sprite_commands, 2);
        assert_eq!(optimized_draws.sprite_draw_calls, 1);
        assert_eq!(
            optimized_draws,
            baked.headless_gpu_draw_stats().expect("baked draw stats")
        );

        let optimized_pixels = optimized
            .headless_output_rgba()
            .expect("mask output readback");
        let baked_pixels = baked.headless_output_rgba().expect("RGBA output readback");
        assert_eq!(optimized_pixels.len(), baked_pixels.len());
        let max_error = optimized_pixels
            .iter()
            .zip(&baked_pixels)
            .map(|(left, right)| left.abs_diff(*right))
            .max()
            .unwrap_or(0);
        assert!(
            max_error <= 1,
            "mask+tint GPU output diverged from baked RGBA by {max_error} byte levels"
        );
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn paragraph_mask_tint_matches_baked_rgba_in_overlay_stream() {
        let overlay_text = Text::new("overlay tint")
            .width(dp(140.0))
            .height(dp(32.0))
            .style(|style, _| style.color = Color::rgba(99, 72, 221, 149).into());
        let tree: WidgetTree<()> = WidgetTree::new(
            Popover::new(Button::new("Open").size(dp(90.0), dp(36.0)))
                .content(Flex::vertical().child(overlay_text))
                .open(true),
        );
        let viewport = Rect::new(0.0, 0.0, 240.0, 140.0);
        let mut optimized = WidgetBenchmarkContext::default().with_viewport(viewport);
        let mut baked = WidgetBenchmarkContext::default().with_viewport(viewport);
        if optimized.initialize_headless_gpu().is_err() || baked.initialize_headless_gpu().is_err()
        {
            eprintln!("skipping overlay mask GPU readback test: no headless adapter");
            return;
        }
        assert!(baked.set_headless_text_mask_tint(false));
        optimized
            .render_cached_scene_to_headless_gpu(&tree, Instant::now())
            .expect("render overlay mask+tint output");
        baked
            .render_cached_scene_to_headless_gpu(&tree, Instant::now())
            .expect("render overlay baked RGBA output");
        let optimized_pixels = optimized
            .headless_output_rgba()
            .expect("overlay mask readback");
        let baked_pixels = baked.headless_output_rgba().expect("overlay RGBA readback");
        let max_error = optimized_pixels
            .iter()
            .zip(&baked_pixels)
            .map(|(left, right)| left.abs_diff(*right))
            .max()
            .unwrap_or(0);
        assert!(max_error <= 1, "overlay output diverged by {max_error}");
        assert_eq!(
            optimized.headless_gpu_draw_stats(),
            baked.headless_gpu_draw_stats()
        );
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn rgb_and_alpha_change_only_vertices_after_mask_warmup() {
        let view_model = ViewModelContext::for_benchmarks();
        let color = view_model.state(Color::rgba(25, 110, 230, 255));
        let color_signal = color.signal();
        let tree: WidgetTree<()> = WidgetTree::new(
            Text::new("retained RGB theme transition")
                .size(dp(220.0), dp(36.0))
                .style(move |style, _| style.color = color_signal.clone().into()),
        );
        let mut context =
            WidgetBenchmarkContext::default().with_viewport(Rect::new(0.0, 0.0, 240.0, 60.0));
        if context.initialize_headless_gpu().is_err() {
            eprintln!("skipping RGB mask cache test: no headless adapter");
            return;
        }
        context
            .render_cached_scene_to_headless_gpu(&tree, Instant::now())
            .expect("warm paragraph mask");
        assert!(context.reset_headless_text_gpu_cache_activity_stats());
        assert!(context.reset_headless_text_atlas_upload_stats());
        assert!(context.reset_headless_glyph_raster_cache_stats());

        color.set(Color::rgba(238, 74, 96, 91));
        context.recollect_scene_only(&tree, Instant::now());
        context
            .render_cached_scene_to_headless_gpu(&tree, Instant::now())
            .expect("render RGB+alpha vertex-only update");
        let activity = context
            .headless_text_gpu_cache_activity_stats()
            .expect("mask cache activity");
        assert_eq!(activity.hits, 1);
        assert_eq!(activity.misses, 0);
        assert_eq!(activity.atlas_releases, 0);
        assert_eq!(activity.retained_prepare_cache_clears, 0);
        let uploads = context
            .headless_text_atlas_upload_stats()
            .expect("mask upload activity");
        assert_eq!(uploads.write_calls, 0);
        assert_eq!(uploads.uploaded_bytes, 0);
        assert_eq!(
            context
                .headless_glyph_raster_cache_stats()
                .expect("glyph raster activity")
                .image_insertions,
            0
        );
    }

    #[cfg(feature = "bench-support")]
    fn assert_hit_region_metadata_equal(
        actual: &crate::ui::widget::HitRegion<()>,
        expected: &crate::ui::widget::HitRegion<()>,
    ) {
        assert_eq!(actual.rect, expected.rect);
        assert_eq!(actual.clip_rect, expected.clip_rect);
        match (&actual.geometry, &expected.geometry) {
            (crate::ui::widget::HitGeometry::Rect, crate::ui::widget::HitGeometry::Rect) => {}
            (
                crate::ui::widget::HitGeometry::Quad(actual),
                crate::ui::widget::HitGeometry::Quad(expected),
            ) => {
                assert_eq!(actual, expected);
            }
            (
                crate::ui::widget::HitGeometry::Triangles(actual),
                crate::ui::widget::HitGeometry::Triangles(expected),
            ) => {
                assert_eq!(actual.as_ref(), expected.as_ref());
            }
            _ => panic!("hit geometry kind differs"),
        }
        assert_eq!(actual.transform_chain, expected.transform_chain);
        assert_eq!(actual.scope_path, expected.scope_path);
        assert_eq!(actual.gpu_scroll_container, expected.gpu_scroll_container);
        assert_eq!(
            std::mem::discriminant(&actual.interaction),
            std::mem::discriminant(&expected.interaction)
        );
        assert_eq!(
            actual.interaction.widget_id(),
            expected.interaction.widget_id()
        );
        match (&actual.focus, &expected.focus) {
            (Some(actual), Some(expected)) => {
                assert_eq!(actual.widget_id, expected.widget_id);
                assert_eq!(actual.tab_index, expected.tab_index);
                assert_eq!(actual.order, expected.order);
                assert_eq!(actual.scope_path, expected.scope_path);
                assert_eq!(actual.on_focus.is_some(), expected.on_focus.is_some());
                assert_eq!(actual.on_blur.is_some(), expected.on_blur.is_some());
            }
            (None, None) => {}
            _ => panic!("focus metadata presence differs"),
        }
    }

    #[cfg(feature = "bench-support")]
    fn assert_scroll_region_equal(
        actual: &crate::ui::widget::ScrollRegion,
        expected: &crate::ui::widget::ScrollRegion,
    ) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.content_viewport, expected.content_viewport);
        assert_eq!(actual.visible_frame, expected.visible_frame);
        assert_eq!(actual.content_bounds, expected.content_bounds);
        assert_eq!(
            actual.gpu_base_scroll_offset,
            expected.gpu_base_scroll_offset
        );
        assert_eq!(actual.scroll_offset, expected.scroll_offset);
        assert_eq!(actual.overflow_x, expected.overflow_x);
        assert_eq!(actual.overflow_y, expected.overflow_y);
        assert_eq!(actual.horizontal_track, expected.horizontal_track);
        assert_eq!(actual.horizontal_thumb, expected.horizontal_thumb);
        assert_eq!(actual.vertical_track, expected.vertical_track);
        assert_eq!(actual.vertical_thumb, expected.vertical_thumb);
    }

    #[cfg(feature = "bench-support")]
    fn assert_accessibility_fragments_equal(
        actual: &[crate::ui::widget::AccessibilityFragment<()>],
        expected: &[crate::ui::widget::AccessibilityFragment<()>],
    ) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(
                actual.source_window_instance_id,
                expected.source_window_instance_id
            );
            assert_eq!(
                actual.source_publication_generation,
                expected.source_publication_generation
            );
            assert_eq!(
                actual
                    .source_open
                    .as_ref()
                    .map(crate::ui::layout::Value::resolve_untracked),
                expected
                    .source_open
                    .as_ref()
                    .map(crate::ui::layout::Value::resolve_untracked)
            );
            assert_eq!(actual.owner_path, expected.owner_path);
            assert_eq!(actual.scope_path, expected.scope_path);
            assert_eq!(actual.clip_rect, expected.clip_rect);
            assert_eq!(
                actual.has_duplicate_widget_ids,
                expected.has_duplicate_widget_ids
            );
            assert_eq!(actual.resolved_root.id, expected.resolved_root.id);
            assert_eq!(actual.nodes.len(), expected.nodes.len());
            for (actual, expected) in actual.nodes.iter().zip(&expected.nodes) {
                assert_eq!(actual.widget_id, expected.widget_id);
                assert_eq!(actual.resolved_path, expected.resolved_path);
                assert_eq!(actual.bounds, expected.bounds);
                assert_eq!(actual.clip_rect, expected.clip_rect);
                assert_eq!(actual.children, expected.children);
                assert_eq!(actual.hits.len(), expected.hits.len());
                for (actual, expected) in actual.hits.iter().zip(&expected.hits) {
                    assert_hit_region_metadata_equal(actual, expected);
                }
                assert_eq!(actual.scroll_regions.len(), expected.scroll_regions.len());
                for (actual, expected) in actual.scroll_regions.iter().zip(&expected.scroll_regions)
                {
                    assert_scroll_region_equal(actual, expected);
                }
            }
        }
    }

    #[cfg(feature = "bench-support")]
    fn assert_scene_streams_equal(actual: &ComputedScene<()>, expected: &ComputedScene<()>) {
        assert_eq!(actual.scene.counts(), expected.scene.counts());
        assert_eq!(
            actual.scene.command_gpu_scroll_containers(),
            expected.scene.command_gpu_scroll_containers()
        );
        assert_eq!(
            actual.scene.overlay_command_gpu_scroll_containers(),
            expected.scene.overlay_command_gpu_scroll_containers()
        );
        assert_eq!(
            actual.scene.command_transform_chains(),
            expected.scene.command_transform_chains()
        );
        assert_eq!(
            actual.scene.overlay_command_transform_chains(),
            expected.scene.overlay_command_transform_chains()
        );
        assert_eq!(
            actual.scene.overlay_command_sources(),
            expected.scene.overlay_command_sources()
        );
        assert_eq!(
            actual.scene.dirty_draw_ranges(),
            expected.scene.dirty_draw_ranges()
        );
        assert_eq!(actual.hit_regions.len(), expected.hit_regions.len());
        assert_eq!(
            actual.overlay_hit_regions.len(),
            expected.overlay_hit_regions.len()
        );
        assert_eq!(
            actual.overlay_close_handlers.len(),
            expected.overlay_close_handlers.len()
        );
        assert_eq!(actual.focus_scopes, expected.focus_scopes);
        assert_accessibility_fragments_equal(
            &actual.accessibility_fragments,
            &expected.accessibility_fragments,
        );
        assert_eq!(
            actual.carousel_auto_play.len(),
            expected.carousel_auto_play.len()
        );
        assert_eq!(actual.overlay_anchors, expected.overlay_anchors);
        assert_eq!(actual.portal_entries.len(), expected.portal_entries.len());
        assert_eq!(
            actual.external_portal_requests.len(),
            expected.external_portal_requests.len()
        );
        assert_eq!(actual.overlay_layers.len(), expected.overlay_layers.len());
        for (actual, expected) in actual.overlay_layers.iter().zip(&expected.overlay_layers) {
            assert_eq!(actual.commands.len(), expected.commands.len());
            assert_eq!(actual.command_sources, expected.command_sources);
            assert_eq!(actual.backdrop_blurs, expected.backdrop_blurs);
            assert_eq!(actual.shapes.len(), expected.shapes.len());
            assert_eq!(actual.textures.len(), expected.textures.len());
            assert_eq!(actual.meshes.len(), expected.meshes.len());
            assert_eq!(actual.texts, expected.texts);
            assert_eq!(actual.text_decorations, expected.text_decorations);
            assert_eq!(actual.hits.len(), expected.hits.len());
            assert_eq!(actual.close_handlers.len(), expected.close_handlers.len());
            assert_eq!(actual.focus_scopes, expected.focus_scopes);
            assert_accessibility_fragments_equal(
                &actual.accessibility_fragments,
                &expected.accessibility_fragments,
            );
        }
        assert_eq!(actual.overlay_layer_graph, expected.overlay_layer_graph);
        assert_eq!(actual.scroll_regions.len(), expected.scroll_regions.len());
        for (actual, expected) in actual.scroll_regions.iter().zip(&expected.scroll_regions) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.content_viewport, expected.content_viewport);
            assert_eq!(actual.visible_frame, expected.visible_frame);
            assert_eq!(actual.content_bounds, expected.content_bounds);
            assert_eq!(
                actual.gpu_base_scroll_offset,
                expected.gpu_base_scroll_offset
            );
            assert_eq!(actual.scroll_offset, expected.scroll_offset);
            assert_eq!(actual.overflow_x, expected.overflow_x);
            assert_eq!(actual.overflow_y, expected.overflow_y);
            assert_eq!(actual.horizontal_track, expected.horizontal_track);
            assert_eq!(actual.horizontal_thumb, expected.horizontal_thumb);
            assert_eq!(actual.vertical_track, expected.vertical_track);
            assert_eq!(actual.vertical_thumb, expected.vertical_thumb);
        }
        assert_eq!(actual.ime_cursor_area, expected.ime_cursor_area);
        assert_eq!(
            actual.virtual_state_updates.len(),
            expected.virtual_state_updates.len()
        );
        assert_eq!(actual.transform_records, expected.transform_records);
        assert_eq!(
            (
                actual.portal_overlay_counts.shapes,
                actual.portal_overlay_counts.textures,
                actual.portal_overlay_counts.meshes,
                actual.portal_overlay_counts.texts,
                actual.portal_overlay_counts.text_decorations,
                actual.portal_overlay_counts.commands,
                actual.portal_overlay_counts.hits,
                actual.portal_overlay_counts.close_handlers,
                actual.portal_overlay_counts.focus_scopes,
                actual.portal_overlay_counts.accessibility_fragments,
            ),
            (
                expected.portal_overlay_counts.shapes,
                expected.portal_overlay_counts.textures,
                expected.portal_overlay_counts.meshes,
                expected.portal_overlay_counts.texts,
                expected.portal_overlay_counts.text_decorations,
                expected.portal_overlay_counts.commands,
                expected.portal_overlay_counts.hits,
                expected.portal_overlay_counts.close_handlers,
                expected.portal_overlay_counts.focus_scopes,
                expected.portal_overlay_counts.accessibility_fragments,
            )
        );
        assert_eq!(
            actual.dependencies.dependency_count(),
            expected.dependencies.dependency_count()
        );
        assert_eq!(
            actual.dependencies.has_global_dependency(),
            expected.dependencies.has_global_dependency()
        );
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn scene_cursor_delta_and_prefix_match_full_clone_snapshots_across_streams() {
        let base_tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(dp(320.0), dp(120.0))
                .overflow_y(crate::ui::layout::Overflow::Scroll)
                .child(Button::new("base action"))
                .child(Text::new("base content").height(dp(300.0))),
        );
        let popover: Element<()> = Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
            .content(
                Flex::vertical()
                    .child(Text::new("overlay body"))
                    .child(Button::new("overlay action")),
            )
            .open(true)
            .into();
        let portal: Element<()> = Portal::new(Button::new("Portal action"))
            .anchor(Rect::new(dp(12.0), dp(12.0), dp(1.0), dp(1.0)))
            .into();
        let overlay_tree: WidgetTree<()> = WidgetTree::new(Stack::new().child([popover, portal]));
        let mut base_context = WidgetBenchmarkContext::default();
        let _ = base_context.run_layout_and_scene(&base_tree, Instant::now());
        let base = base_context.cached_scene.as_ref().unwrap().clone();
        let mut overlay_context = WidgetBenchmarkContext::default();
        let _ = overlay_context.run_layout_and_scene(&overlay_tree, Instant::now());
        let addition = overlay_context.cached_scene.as_ref().unwrap().clone();
        assert!(!addition.accessibility_fragments.is_empty());

        let delta_cursor = base.cursor();
        let prefix_cursor = base.prefix_cursor();
        let full_clone_snapshot = base.clone();
        let mut current = base.clone();
        current.extend(&addition);

        let cursor_delta = current.delta_since_cursor(&delta_cursor);
        let clone_delta = current.delta_since(&full_clone_snapshot);
        assert_scene_streams_equal(&cursor_delta, &clone_delta);
        assert_eq!(
            cursor_delta.scene.counts(),
            current
                .scene
                .delta_since(&full_clone_snapshot.scene)
                .counts()
        );

        let cursor_prefix = current.prefix_at_cursor(&prefix_cursor);
        assert_scene_streams_equal(&cursor_prefix, &full_clone_snapshot);
    }

    #[test]
    fn reuses_layout_cache_for_stable_tree() {
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .width(dp(320.0))
                .child(Text::new("alpha"))
                .child(Text::new("beta")),
        );
        let mut bench = WidgetBenchmarkContext::default();

        let _ = bench.run_layout(&tree, Instant::now());
        let first_layout_ptr = bench
            .cached_layout
            .as_ref()
            .map(|layout| layout as *const _)
            .expect("first layout should be cached");

        let _ = bench.run_layout(&tree, Instant::now());
        let second_layout_ptr = bench
            .cached_layout
            .as_ref()
            .map(|layout| layout as *const _)
            .expect("second layout should be cached");

        assert_eq!(first_layout_ptr, second_layout_ptr);
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn shallow_deep_leaf_scene_patch_runs_once() {
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .width(dp(320.0))
                .height(dp(120.0))
                .padding(crate::ui::layout::Insets::all(dp(8.0)))
                .child(
                    Flex::new(Axis::Vertical)
                        .width(dp(240.0))
                        .padding(crate::ui::layout::Insets::all(dp(4.0)))
                        .child(Text::new("leaf")),
                ),
        );
        let mut bench = WidgetBenchmarkContext::default();
        let _ = bench.run_layout_and_scene(&tree, Instant::now());

        assert!(bench.patch_single_deep_leaf_scene(&tree, Instant::now()));
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn repeated_deep_leaf_scene_patch_does_not_grow_stack() {
        let mut node = Flex::new(Axis::Vertical)
            .width(dp(720.0))
            .padding(crate::ui::layout::Insets::all(dp(1.0)))
            .height(dp(28.0));

        for _ in 0..4 {
            node = Flex::new(Axis::Vertical)
                .width(dp(720.0))
                .padding(crate::ui::layout::Insets::all(dp(1.0)))
                .child(node);
        }

        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .width(dp(760.0))
                .height(dp(720.0))
                .padding(crate::ui::layout::Insets::all(dp(8.0)))
                .child(node),
        );
        let mut bench = WidgetBenchmarkContext::default();
        let _ = bench.run_layout_and_scene(&tree, Instant::now());

        for _ in 0..4 {
            assert!(bench.patch_single_deep_leaf_scene(&tree, Instant::now()));
        }
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn reused_recompose_matches_legacy_clone_across_all_scene_streams() {
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(dp(420.0), dp(180.0))
                .overflow_y(crate::ui::layout::Overflow::Scroll)
                .child(
                    Flex::new(Axis::Vertical)
                        .height(dp(420.0))
                        .child(Button::new("first"))
                        .child(
                            Flex::new(Axis::Vertical)
                                .child(Text::new("nested leaf"))
                                .child(Button::new("second")),
                        ),
                ),
        );
        let now = Instant::now();
        let mut reused = WidgetBenchmarkContext::default();
        let mut legacy = WidgetBenchmarkContext::default();
        let _ = reused.run_layout_and_scene(&tree, now);
        let _ = legacy.run_layout_and_scene(&tree, now);

        assert!(reused.patch_single_deep_leaf_scene(&tree, now));
        assert!(legacy.patch_single_deep_leaf_scene_legacy_recompose(&tree, now));
        assert_scene_streams_equal(
            reused.cached_scene.as_ref().expect("reused scene"),
            legacy.cached_scene.as_ref().expect("legacy scene"),
        );

        let reused_layout = reused.cached_layout.as_ref().expect("reused layout");
        for widget_id in reused_layout.all_widget_ids() {
            match (
                reused.cached_scene_chunks.get(&widget_id),
                legacy.cached_scene_chunks.get(&widget_id),
            ) {
                (Some(actual), Some(expected)) => assert_scene_streams_equal(actual, expected),
                (None, None) => {}
                _ => panic!("chunk presence differs for {widget_id:?}"),
            }
        }
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn reused_multi_root_recompose_matches_legacy_clone() {
        let mut body = Flex::new(Axis::Vertical);
        for branch in 0..8 {
            body = body.child(
                Flex::new(Axis::Vertical)
                    .child(Text::new(format!("branch {branch}")))
                    .child(Flex::new(Axis::Vertical).child(Button::new("leaf"))),
            );
        }
        let tree = WidgetTree::new(body);
        let now = Instant::now();
        let mut reused = WidgetBenchmarkContext::default();
        let mut legacy = WidgetBenchmarkContext::default();
        let _ = reused.run_layout_and_scene(&tree, now);
        let _ = legacy.run_layout_and_scene(&tree, now);

        assert!(reused.patch_multiple_deep_leaf_scenes(&tree, 8, now));
        assert!(legacy.patch_multiple_deep_leaf_scenes_legacy_recompose(&tree, 8, now));
        assert_scene_streams_equal(
            reused.cached_scene.as_ref().expect("reused scene"),
            legacy.cached_scene.as_ref().expect("legacy scene"),
        );
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn reused_recompose_keeps_large_ancestor_stream_storage() {
        let mut node = Flex::new(Axis::Vertical).height(dp(28.0));
        for _ in 0..12 {
            node = Flex::new(Axis::Vertical).child(node);
        }
        let tree = WidgetTree::new(Flex::new(Axis::Vertical).child(node));
        let now = Instant::now();
        let mut bench = WidgetBenchmarkContext::default();
        let _ = bench.run_layout_and_scene(&tree, now);
        let root_id = bench.cached_layout.as_ref().unwrap().root_id();
        let (before_ptr, before_capacity) = {
            let root = bench.cached_scene_chunks.get(&root_id).unwrap();
            (root.scroll_regions.as_ptr(), root.scroll_regions.capacity())
        };
        assert!(
            before_capacity > 1,
            "test requires heap-backed ancestor stream"
        );

        assert!(bench.patch_single_deep_leaf_scene(&tree, now));

        let root = bench.cached_scene_chunks.get(&root_id).unwrap();
        assert_eq!(root.scroll_regions.capacity(), before_capacity);
        assert_eq!(root.scroll_regions.as_ptr(), before_ptr);
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn reused_recompose_missing_child_preserves_old_ancestor_for_fallback() {
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .child(Text::new("first"))
                .child(Flex::new(Axis::Vertical).child(Text::new("second"))),
        );
        let now = Instant::now();
        let mut bench = WidgetBenchmarkContext::default();
        let _ = bench.run_layout_and_scene(&tree, now);
        let layout = bench.cached_layout.as_ref().unwrap();
        let root_id = layout.root_id();
        let child_id = layout
            .all_widget_ids()
            .find(|id| layout.parent_of(*id) == Some(root_id))
            .expect("root child");
        let expected = bench.cached_scene_chunks.get(&root_id).unwrap().clone();
        bench.cached_scene_chunks.remove(&child_id);

        assert!(layout
            .recompose_scene_chunk(
                root_id,
                &bench.cached_chunk_parts,
                &mut bench.cached_scene_chunks,
            )
            .is_none());
        assert_scene_streams_equal(bench.cached_scene_chunks.get(&root_id).unwrap(), &expected);
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn cached_hit_path_len_reads_real_scene_hit_regions() {
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .width(dp(320.0))
                .height(dp(120.0))
                .padding(crate::ui::layout::Insets::all(dp(8.0)))
                .child(Button::new("Inspect").size(dp(120.0), dp(36.0))),
        );
        let mut bench = WidgetBenchmarkContext::default();

        let hit_len = bench.cached_hit_path_len(&tree, Point::new(24.0, 24.0), Instant::now());

        assert!(hit_len > 0);
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn cached_subtree_sizes_match_layout_index_for_every_widget() {
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .child(Text::new("root child"))
                .child(
                    Flex::new(Axis::Horizontal).child(Button::new("one")).child(
                        Flex::new(Axis::Vertical)
                            .child(Text::new("two"))
                            .child(Button::new("three")),
                    ),
                ),
        );
        let mut bench = WidgetBenchmarkContext::default();
        let _ = bench.run_layout(&tree, Instant::now());
        let layout = bench.cached_layout.as_ref().expect("layout should exist");

        assert_eq!(layout.subtree_sizes.len(), layout.paths.len());
        for widget_id in layout.paths.keys().copied() {
            assert_eq!(
                layout.subtree_size(widget_id),
                layout.subtree_widget_ids(widget_id).len(),
                "cached subtree size differs for {widget_id:?}"
            );
        }
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn static_scroll_child_bounds_are_cached_across_scene_recollects() {
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(dp(320.0), dp(120.0))
                .overflow_y(crate::ui::layout::Overflow::Scroll)
                .child(Stack::new().size(dp(280.0), dp(600.0))),
        );
        let mut bench = WidgetBenchmarkContext::default();

        let _ = bench.run_layout_and_scene(&tree, Instant::now());
        let layout = bench
            .cached_layout
            .as_ref()
            .expect("layout should be cached");
        assert!(layout
            .layout_root
            .cached_child_content_bounds
            .get()
            .is_some());

        let before = layout
            .layout_root
            .cached_child_content_bounds
            .get()
            .copied();
        let _ = bench.recollect_scene_only(&tree, Instant::now());
        let after = bench
            .cached_layout
            .as_ref()
            .expect("layout should remain cached")
            .layout_root
            .cached_child_content_bounds
            .get()
            .copied();
        assert_eq!(after, before);
    }

    #[cfg(feature = "bench-support")]
    fn build_indexed_scroll_tree(rows: usize) -> (WidgetTree<()>, WidgetId) {
        let mut content = Flex::new(Axis::Vertical).width(dp(280.0));
        for row in 0..rows {
            content = content.child(
                Text::new(format!("indexed row {row:04}"))
                    .width(dp(260.0))
                    .height(dp(24.0)),
            );
        }
        let scroller: crate::ui::widget::Element<()> = Flex::new(Axis::Vertical)
            .size(dp(320.0), dp(120.0))
            .overflow_y(crate::ui::layout::Overflow::Scroll)
            .child(content)
            .into();
        let scroller_id = scroller.id;
        (WidgetTree::new(scroller), scroller_id)
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn indexed_scroll_child_collection_matches_full_scan() {
        let (tree, scroller_id) = build_indexed_scroll_tree(200);
        let viewport = Rect::new(0.0, 0.0, 320.0, 120.0);
        let offset = Point::new(dp(0.0), dp(1_800.0));

        let mut indexed = WidgetBenchmarkContext::new().with_viewport(viewport);
        let _ = indexed.run_layout(&tree, Instant::now());
        indexed.set_scroll_offset(scroller_id, offset);
        let _ = indexed.recollect_scene_only(&tree, Instant::now());
        assert!(matches!(
            indexed
                .cached_layout
                .as_ref()
                .expect("indexed layout should exist")
                .layout_root
                .children[0]
                .cached_child_cull_index
                .get(),
            Some(Some(_))
        ));
        let indexed_scene = indexed
            .cached_scene
            .as_ref()
            .expect("indexed scene should exist");
        let indexed_shapes = indexed_scene
            .scene
            .shapes
            .iter()
            .map(|shape| {
                (
                    shape.rect,
                    shape.color,
                    shape.corner_radius,
                    shape.stroke_width,
                    shape.clip_rect,
                    shape.clip_mask,
                )
            })
            .collect::<Vec<_>>();
        let indexed_texts = indexed_scene.scene.texts.clone();
        let indexed_commands = indexed_scene
            .scene
            .commands
            .iter()
            .map(std::mem::discriminant)
            .collect::<Vec<_>>();
        let indexed_scrolls = indexed_scene
            .scroll_regions
            .iter()
            .map(|region| {
                (
                    region.id,
                    region.content_viewport,
                    region.visible_frame,
                    region.content_bounds,
                    region.scroll_offset,
                    region.horizontal_track,
                    region.horizontal_thumb,
                    region.vertical_track,
                    region.vertical_thumb,
                )
            })
            .collect::<Vec<_>>();

        let mut full_scan = WidgetBenchmarkContext::new().with_viewport(viewport);
        let _ = full_scan.run_layout(&tree, Instant::now());
        full_scan.set_scroll_offset(scroller_id, offset);
        full_scan.disable_cached_child_culling();
        let _ = full_scan.recollect_scene_only(&tree, Instant::now());
        let full_scene = full_scan
            .cached_scene
            .as_ref()
            .expect("full-scan scene should exist");
        let full_scrolls = full_scene
            .scroll_regions
            .iter()
            .map(|region| {
                (
                    region.id,
                    region.content_viewport,
                    region.visible_frame,
                    region.content_bounds,
                    region.scroll_offset,
                    region.horizontal_track,
                    region.horizontal_thumb,
                    region.vertical_track,
                    region.vertical_thumb,
                )
            })
            .collect::<Vec<_>>();

        let full_shapes = full_scene
            .scene
            .shapes
            .iter()
            .map(|shape| {
                (
                    shape.rect,
                    shape.color,
                    shape.corner_radius,
                    shape.stroke_width,
                    shape.clip_rect,
                    shape.clip_mask,
                )
            })
            .collect::<Vec<_>>();
        let full_commands = full_scene
            .scene
            .commands
            .iter()
            .map(std::mem::discriminant)
            .collect::<Vec<_>>();
        assert_eq!(indexed_shapes, full_shapes);
        assert_eq!(indexed_texts, full_scene.scene.texts);
        assert_eq!(indexed_commands, full_commands);
        assert_eq!(indexed_scrolls, full_scrolls);
        assert_eq!(
            indexed_scene.hit_regions.len(),
            full_scene.hit_regions.len()
        );
        assert_eq!(
            indexed_scene.overlay_hit_regions.len(),
            full_scene.overlay_hit_regions.len()
        );
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn layout_recompute_invalidates_and_rebuilds_child_cull_index() {
        let context = ViewModelContext::for_benchmarks();
        let first_height = context.state(Length::Px(dp(24.0)));
        let first: crate::ui::widget::Element<()> = Text::new("resizable row")
            .width(dp(260.0))
            .height(first_height.signal())
            .into();
        let first_id = first.id;
        let mut content = Flex::new(Axis::Vertical).width(dp(280.0)).child(first);
        for row in 1..20 {
            content = content.child(
                Text::new(format!("stable row {row:02}"))
                    .width(dp(260.0))
                    .height(dp(24.0)),
            );
        }
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(dp(320.0), dp(120.0))
                .overflow_y(crate::ui::layout::Overflow::Scroll)
                .child(content),
        );
        let mut bench =
            WidgetBenchmarkContext::new().with_viewport(Rect::new(0.0, 0.0, 320.0, 120.0));

        let _ = bench.run_layout_and_scene(&tree, Instant::now());
        let before_end = bench
            .cached_layout
            .as_ref()
            .expect("layout should exist")
            .layout_root
            .children[0]
            .cached_child_cull_index
            .get()
            .and_then(Option::as_ref)
            .expect("index should be built")
            .intervals[0]
            .end;

        first_height.set(Length::Px(dp(48.0)));
        {
            let layout = bench.cached_layout.as_mut().expect("layout should exist");
            layout
                .update_layout_style_slots(
                    &[first_id],
                    &bench.font_manager,
                    &bench.theme,
                    &bench.media,
                    &mut bench.animations,
                    bench.viewport,
                    Instant::now(),
                )
                .expect("layout slot update should succeed");
            assert!(layout.layout_root.children[0]
                .cached_child_cull_index
                .get()
                .is_none());
        }
        let _ = bench.recollect_scene_only(&tree, Instant::now());
        let after_end = bench
            .cached_layout
            .as_ref()
            .expect("layout should remain cached")
            .layout_root
            .children[0]
            .cached_child_cull_index
            .get()
            .and_then(Option::as_ref)
            .expect("index should be rebuilt")
            .intervals[0]
            .end;

        assert_eq!(after_end - before_end, dp(24.0));
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn wrapped_scroll_content_falls_back_to_full_child_scan() {
        let content = Flex::new(Axis::Vertical)
            .wrap(Wrap::Wrap)
            .width(dp(280.0))
            .child(Text::new("one").size(dp(120.0), dp(24.0)))
            .child(Text::new("two").size(dp(120.0), dp(24.0)));
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(dp(320.0), dp(120.0))
                .overflow_y(crate::ui::layout::Overflow::Scroll)
                .child(content),
        );
        let mut bench = WidgetBenchmarkContext::default();

        let _ = bench.run_layout_and_scene(&tree, Instant::now());
        assert!(matches!(
            bench
                .cached_layout
                .as_ref()
                .expect("layout should exist")
                .layout_root
                .children[0]
                .cached_child_cull_index
                .get(),
            Some(None)
        ));
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn absolute_scroll_child_falls_back_to_full_child_scan() {
        let content = Flex::new(Axis::Vertical)
            .width(dp(280.0))
            .child(
                Text::new("absolute")
                    .size(dp(120.0), dp(24.0))
                    .position_absolute(),
            )
            .child(Text::new("flow").size(dp(120.0), dp(24.0)));
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(dp(320.0), dp(120.0))
                .overflow_y(crate::ui::layout::Overflow::Scroll)
                .child(content),
        );
        let mut bench = WidgetBenchmarkContext::default();

        let _ = bench.run_layout_and_scene(&tree, Instant::now());
        assert!(matches!(
            bench
                .cached_layout
                .as_ref()
                .expect("layout should exist")
                .layout_root
                .children[0]
                .cached_child_cull_index
                .get(),
            Some(None)
        ));
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn nested_scroll_child_falls_back_to_full_child_scan() {
        let nested = Flex::new(Axis::Vertical)
            .size(dp(260.0), dp(80.0))
            .overflow_y(crate::ui::layout::Overflow::Scroll)
            .child(Text::new("nested content").size(dp(240.0), dp(240.0)));
        let content = Flex::new(Axis::Vertical)
            .width(dp(280.0))
            .child(nested)
            .child(Text::new("tail").size(dp(240.0), dp(24.0)));
        let tree = WidgetTree::new(
            Flex::new(Axis::Vertical)
                .size(dp(320.0), dp(120.0))
                .overflow_y(crate::ui::layout::Overflow::Scroll)
                .child(content),
        );
        let mut bench = WidgetBenchmarkContext::default();

        let _ = bench.run_layout_and_scene(&tree, Instant::now());
        assert!(matches!(
            bench
                .cached_layout
                .as_ref()
                .expect("layout should exist")
                .layout_root
                .children[0]
                .cached_child_cull_index
                .get(),
            Some(None)
        ));
    }

    #[cfg(feature = "bench-support")]
    #[test]
    fn dynamic_scroll_child_offset_bypasses_bounds_cache() {
        let context = ViewModelContext::for_benchmarks();
        let offset = context.state(Point::ZERO);
        let scroller: crate::ui::widget::Element<()> = Flex::new(Axis::Vertical)
            .size(dp(320.0), dp(120.0))
            .overflow_y(crate::ui::layout::Overflow::Scroll)
            .child(
                Stack::new()
                    .size(dp(280.0), dp(200.0))
                    .offset(offset.signal()),
            )
            .into();
        let scroller_id = scroller.id;
        let tree = WidgetTree::new(scroller);
        let mut bench = WidgetBenchmarkContext::default();

        let _ = bench.run_layout_and_scene(&tree, Instant::now());
        let before = bench
            .cached_scene
            .as_ref()
            .and_then(|scene| {
                scene
                    .scroll_regions
                    .iter()
                    .find(|region| region.id == scroller_id)
            })
            .expect("scroll region should exist")
            .content_bounds;
        assert!(bench
            .cached_layout
            .as_ref()
            .expect("layout should be cached")
            .layout_root
            .cached_child_content_bounds
            .get()
            .is_none());
        assert!(matches!(
            bench
                .cached_layout
                .as_ref()
                .expect("layout should be cached")
                .layout_root
                .cached_child_cull_index
                .get(),
            Some(None)
        ));

        offset.set(Point::new(dp(0.0), dp(40.0)));
        let _ = bench.recollect_scene_only(&tree, Instant::now());
        let after = bench
            .cached_scene
            .as_ref()
            .and_then(|scene| {
                scene
                    .scroll_regions
                    .iter()
                    .find(|region| region.id == scroller_id)
            })
            .expect("scroll region should exist")
            .content_bounds;

        assert_eq!(after.bottom() - before.bottom(), dp(40.0));
        assert!(bench
            .cached_layout
            .as_ref()
            .expect("layout should remain cached")
            .layout_root
            .cached_child_content_bounds
            .get()
            .is_none());
    }

    #[cfg(feature = "collect-profile")]
    fn build_profile_scroll_tree(node_count: usize) -> WidgetTree<()> {
        let mut content = Flex::new(Axis::Vertical)
            .width(dp(1240.0))
            .padding(Insets::all(dp(8.0)))
            .gap(dp(6.0));
        for row in 0..node_count {
            let line = format!("Row {row} content line with a bit of repeated text to shape");
            let card = Stack::new()
                .width(dp(1200.0))
                .padding(Insets::all(dp(6.0)))
                .child(Text::new(format!("Row {row}")))
                .child(Text::new(line))
                .child(
                    Flex::new(Axis::Horizontal)
                        .gap(dp(4.0))
                        .child(Text::new("left metric"))
                        .child(Text::new("center metric"))
                        .child(Text::new("right metric")),
                );
            content = content.child(card);
        }
        WidgetTree::new(
            Flex::new(Axis::Vertical)
                .width(dp(1280.0))
                .height(dp(800.0))
                .overflow_y(Overflow::Scroll)
                .child(content),
        )
    }

    /// 手动运行: `cargo test --features collect-profile profile_recollect_breakdown -- --ignored --nocapture`
    /// 打印每相位独占耗时占比,用来在结构性优化前定位真实热点。
    #[cfg(feature = "collect-profile")]
    #[test]
    #[ignore]
    fn profile_recollect_breakdown() {
        for node_count in [200_usize, 1000_usize] {
            let tree = build_profile_scroll_tree(node_count);
            let mut bench =
                WidgetBenchmarkContext::default().with_viewport(Rect::new(0.0, 0.0, 1280.0, 800.0));
            // 预热布局 + 字体缓存。
            let _ = bench.recollect_scene_only(&tree, Instant::now());

            const RUNS: usize = 30;
            crate::ui::widget::core::collect_profile::reset();
            let wall = Instant::now();
            for _ in 0..RUNS {
                let _ = bench.recollect_scene_only(&tree, Instant::now());
            }
            let wall_ms = wall.elapsed().as_secs_f64() * 1000.0 / RUNS as f64;
            let b = crate::ui::widget::core::collect_profile::snapshot();
            let per = |total: f64| total / RUNS as f64;
            let nodes = b.node_count / RUNS as u64;
            let visible = b.visible_node_count / RUNS as u64;
            let ratio = if visible == 0 {
                f64::INFINITY
            } else {
                nodes as f64 / visible as f64
            };
            println!(
                "n={node_count}: wall={wall_ms:.2}ms/frame nodes={nodes} visible={visible} \
                 recollect/visible={ratio:.2} \
                 visual_state={:.2}ms surface={:.2}ms kind_body(incl recursion)={:.2}ms \
                 text={:.2}ms bookkeeping={:.2}ms",
                per(b.visual_state_ms),
                per(b.surface_ms),
                per(b.kind_body_ms),
                per(b.text_ms),
                per(b.bookkeeping_ms),
            );
        }
    }
}
