use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::animation::AnimationEngine;
use crate::foundation::binding::InvalidationSignal;
use crate::media::MediaManager;
use crate::text::font::{FontCatalog, FontManager};
use crate::ui::theme::Theme;
use crate::ui::unit::{Dp, UnitContext};
use crate::ui::widget::{
    ComputedScene, Point, Rect, VisualContextSnapshot, WidgetId, WidgetStateMap,
};
use smallvec::SmallVec;

use super::{CollectedSceneCache, ResolvedSceneLayout, SceneChunkParts, WidgetTree};

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
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuTextCacheActivityStats {
    pub hits: usize,
    pub misses: usize,
    pub atlas_releases: usize,
    pub retained_prepare_cache_clears: usize,
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
pub struct GpuTextureSceneStats {
    pub texture_commands: usize,
    pub unique_texture_ids: usize,
    pub unique_clip_rects: usize,
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
}

/// benchmark 的轻量统计摘要，避免直接暴露内部布局/scene 类型。
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WidgetBenchmarkStats {
    pub dependency_count: usize,
    pub has_global_dependency: bool,
    pub shape_count: usize,
    pub text_count: usize,
    pub texture_count: usize,
    pub overlay_shape_count: usize,
    pub hit_region_count: usize,
    pub scroll_region_count: usize,
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

impl WidgetBenchmarkContext {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            font_manager: FontManager::new(&FontCatalog::default()),
            theme: Theme::default(),
            media: MediaManager::new(InvalidationSignal::new()),
            animations: AnimationEngine::default(),
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
        let (hits, misses, atlas_releases, retained_prepare_cache_clears) =
            self.gpu_renderer.as_ref()?.text_cache_activity_stats();
        Some(GpuTextCacheActivityStats {
            hits,
            misses,
            atlas_releases,
            retained_prepare_cache_clears,
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
        self.patch_scene_roots(&[leaf], false)
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
        self.patch_scene_roots_with_strategy(&[leaf], false, true)
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
        !roots.is_empty() && self.patch_scene_roots_with_strategy(&roots, false, legacy_recompose)
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
        if !self.patch_scene_roots(&[root], false) {
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

    fn cached_scroll_target_with_strategy(
        &mut self,
        tree: &WidgetTree<()>,
        point: Point,
        delta: Point,
        now: Instant,
        use_index: bool,
    ) -> Option<WidgetId> {
        self.sync_cache(tree, now, true);
        let computed = self.cached_scene.as_ref()?;
        let regions = computed.scroll_regions.as_slice();
        let indexed = use_index
            .then(|| computed.scroll_region_lookup_index())
            .flatten();

        let visit = |index: usize| {
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
                    return Some(id);
                }
            }
        } else {
            for region_index in (0..regions.len()).rev() {
                if let Some(id) = visit(region_index) {
                    return Some(id);
                }
            }
        }
        None
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
        let tree_ptr = tree as *const WidgetTree<()> as usize;
        if self.last_tree_ptr != Some(tree_ptr) || self.cached_layout.is_none() {
            self.rebuild_layout(tree, now);
            self.last_tree_ptr = Some(tree_ptr);
        } else {
            self.refresh_animation_caches(now, need_scene);
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
            &Default::default(),
        );
        self.store_scene_cache(collected);
    }

    fn clear_scene_cache(&mut self) {
        self.cached_scene = None;
        self.cached_scene_chunks.clear();
        self.cached_chunk_parts.clear();
        self.cached_visual_contexts.clear();
    }

    fn refresh_animation_caches(&mut self, now: Instant, need_scene: bool) {
        let refresh = self.animations.refresh(now);
        if !refresh.changed {
            return;
        }

        let mut layout_roots = SmallVec::<[WidgetId; 16]>::new();
        let mut scene_roots = SmallVec::<[WidgetId; 16]>::new();
        if let Some(layout) = self.cached_layout.as_ref() {
            if !refresh.layout_widget_ids.is_empty() {
                let affected_ids = refresh
                    .layout_widget_ids
                    .iter()
                    .map(|widget_id| WidgetId::from_raw(*widget_id))
                    .collect::<HashSet<_>>();
                layout_roots = self.highest_layout_roots(layout, &affected_ids);
            }
            if !refresh.scene_widget_ids.is_empty() || !refresh.layout_widget_ids.is_empty() {
                let mut affected_ids = refresh
                    .scene_widget_ids
                    .iter()
                    .map(|widget_id| WidgetId::from_raw(*widget_id))
                    .collect::<HashSet<_>>();
                affected_ids.extend(
                    refresh
                        .layout_widget_ids
                        .iter()
                        .map(|widget_id| WidgetId::from_raw(*widget_id)),
                );
                scene_roots = self.highest_layout_roots(layout, &affected_ids);
            }
        }

        if !layout_roots.is_empty() && !self.patch_layout_roots(&layout_roots, now) {
            self.cached_layout = None;
            self.clear_scene_cache();
            return;
        }

        if !need_scene {
            if !layout_roots.is_empty() {
                self.clear_scene_cache();
            }
            return;
        }

        if !scene_roots.is_empty() {
            let resolve_roots = layout_roots.is_empty();
            if !self.patch_scene_roots(&scene_roots, resolve_roots) {
                self.clear_scene_cache();
            }
        }
    }

    fn highest_layout_roots(
        &self,
        layout: &ResolvedSceneLayout<()>,
        affected_ids: &HashSet<WidgetId>,
    ) -> SmallVec<[WidgetId; 16]> {
        let mut roots = SmallVec::<[WidgetId; 16]>::new();
        for widget_id in affected_ids.iter().copied() {
            let mut parent = layout.parent_of(widget_id);
            let mut is_highest = true;
            while let Some(current) = parent {
                if affected_ids.contains(&current) {
                    is_highest = false;
                    break;
                }
                parent = layout.parent_of(current);
            }
            if is_highest {
                roots.push(widget_id);
            }
        }
        roots.sort_by_key(|widget_id| std::cmp::Reverse(layout.depth_of(*widget_id)));
        roots
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

    fn patch_scene_roots(&mut self, roots: &[WidgetId], resolve_roots: bool) -> bool {
        self.patch_scene_roots_with_strategy(roots, resolve_roots, false)
    }

    fn patch_scene_roots_with_strategy(
        &mut self,
        roots: &[WidgetId],
        resolve_roots: bool,
        legacy_recompose: bool,
    ) -> bool {
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
            let default_style_sheet = crate::ui::widget::StyleSheet::default();
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
                    &default_style_sheet,
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

        for patch in patches {
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

        let Some(root_chunk) = self.cached_scene_chunks.get(&root_id).cloned() else {
            return false;
        };
        self.cached_scene = Some(root_chunk);
        true
    }

    fn store_scene_cache(&mut self, collected: CollectedSceneCache<()>) {
        self.cached_scene = Some(collected.computed);
        self.cached_scene_chunks = collected.chunks;
        self.cached_chunk_parts = collected.chunk_parts;
        self.cached_visual_contexts = collected.visual_contexts;
    }
}

impl Default for WidgetBenchmarkContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::binding::ViewModelContext;
    use crate::foundation::color::Color;
    #[cfg(feature = "collect-profile")]
    use crate::ui::layout::Insets;
    use crate::ui::layout::{Axis, Length, Overflow, Wrap};
    use crate::ui::unit::dp;
    use crate::ui::widget::{Button, Flex, Point, Popover, Stack, Text};

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
        let overlay_tree: WidgetTree<()> = WidgetTree::new(
            Popover::new(Button::new("More").size(dp(90.0), dp(36.0)))
                .content(
                    Flex::vertical()
                        .child(Text::new("overlay body"))
                        .child(Button::new("overlay action")),
                )
                .open(true),
        );
        let mut base_context = WidgetBenchmarkContext::default();
        let _ = base_context.run_layout_and_scene(&base_tree, Instant::now());
        let base = base_context.cached_scene.as_ref().unwrap().clone();
        let mut overlay_context = WidgetBenchmarkContext::default();
        let _ = overlay_context.run_layout_and_scene(&overlay_tree, Instant::now());
        let addition = overlay_context.cached_scene.as_ref().unwrap().clone();

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
