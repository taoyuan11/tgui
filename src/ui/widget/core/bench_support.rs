use std::time::Instant;

use crate::animation::AnimationEngine;
use crate::foundation::binding::InvalidationSignal;
use crate::media::MediaManager;
use crate::text::font::{FontCatalog, FontManager};
use crate::ui::theme::Theme;
use crate::ui::unit::UnitContext;
use crate::ui::widget::common::Rect;

use super::WidgetTree;

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
        }
    }

    #[allow(dead_code)]
    pub fn with_viewport(mut self, viewport: Rect) -> Self {
        self.viewport = viewport;
        self
    }

    #[allow(dead_code)]
    pub fn run_layout(
        &mut self,
        tree: &WidgetTree<()>,
        now: Instant,
    ) -> WidgetBenchmarkStats {
        let layout = tree.build_scene_layout_at(
            &self.font_manager,
            &self.theme,
            &self.media,
            &mut self.animations,
            self.units,
            self.viewport,
            now,
        );

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
        let computed = tree.compute_scene_with_units_and_widget_state_at(
            &self.font_manager,
            &self.theme,
            &self.media,
            self.units,
            &mut self.animations,
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
            false,
            now,
        );

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
}

impl Default for WidgetBenchmarkContext {
    fn default() -> Self {
        Self::new()
    }
}
