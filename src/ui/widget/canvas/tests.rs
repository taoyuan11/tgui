use super::{
    canvas_scene_bounds, normalized_source_rect, source_rect_to_uv_rect,
    tessellate_axis_aligned_rounded_rect, tessellate_canvas_scene_items, CanvasBrush,
    CanvasColorFilter, CanvasEffect, CanvasFillRule, CanvasGradientStop, CanvasImageOptions,
    CanvasRecorder, CanvasScene, CanvasShadow, CanvasStroke, CanvasTextOverflow, CanvasTextSpan,
    CanvasTextStyle, PathBuilder, PathCommand,
};
use crate::foundation::binding::InvalidationSignal;
use crate::foundation::color::Color;
use crate::media::{ContentFit, IntrinsicSize, MediaManager, MediaSource};
use crate::text::font::{FontCatalog, FontManager, FontWeight};
use crate::ui::layout::Value;
use crate::ui::unit::dp;
use crate::ui::unit::UnitContext;
use crate::ui::widget::{Point, Rect, RenderCommand};

mod bounds_and_recording;
mod scene_queries;
mod text_and_paths;

fn test_media() -> MediaManager {
    MediaManager::new(InvalidationSignal::new())
}

fn rendered_items(scene: &CanvasScene) -> Vec<super::CanvasSceneItemRender> {
    let font_manager = FontManager::new(&FontCatalog::default());
    tessellate_canvas_scene_items(
        scene,
        Point::ZERO,
        1.0,
        None,
        None,
        &font_manager,
        &test_media(),
        UnitContext::default(),
    )
}
