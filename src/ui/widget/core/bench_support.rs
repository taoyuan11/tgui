use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::animation::AnimationEngine;
use crate::foundation::binding::InvalidationSignal;
use crate::media::MediaManager;
use crate::text::font::{FontCatalog, FontManager};
use crate::ui::theme::Theme;
use crate::ui::unit::UnitContext;
use crate::ui::widget::{ComputedScene, Rect, VisualContextSnapshot, WidgetId, WidgetStateMap};
use smallvec::SmallVec;

use super::{CollectedSceneCache, ResolvedSceneLayout, SceneChunkParts, WidgetTree};

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
    last_tree_ptr: Option<usize>,
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
            last_tree_ptr: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_viewport(mut self, viewport: Rect) -> Self {
        self.viewport = viewport;
        self
    }

    #[allow(dead_code)]
    pub fn invalidate_all(&mut self) {
        self.cached_layout = None;
        self.clear_scene_cache();
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
            &Default::default(),
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
        let empty_widget_states = WidgetStateMap::default();
        let empty_select_states = HashMap::new();
        let empty_scroll_offsets = HashMap::new();

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

            let mut patches = SmallVec::<[ScenePatch; 8]>::new();
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
                    &empty_scroll_offsets,
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
            if layout
                .recompose_scene_chunk(
                    ancestor,
                    &self.cached_chunk_parts,
                    &mut self.cached_scene_chunks,
                )
                .is_none()
            {
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
    use crate::ui::layout::Axis;
    #[cfg(feature = "collect-profile")]
    use crate::ui::layout::{Insets, Overflow};
    use crate::ui::unit::dp;
    #[cfg(feature = "collect-profile")]
    use crate::ui::widget::Stack;
    use crate::ui::widget::{Flex, Text};

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
            println!(
                "n={node_count}: wall={wall_ms:.2}ms/frame nodes={} \
                 visual_state={:.2}ms surface={:.2}ms kind_body(incl recursion)={:.2}ms \
                 text={:.2}ms bookkeeping={:.2}ms",
                b.node_count / RUNS as u64,
                per(b.visual_state_ms),
                per(b.surface_ms),
                per(b.kind_body_ms),
                per(b.text_ms),
                per(b.bookkeeping_ms),
            );
        }
    }
}
