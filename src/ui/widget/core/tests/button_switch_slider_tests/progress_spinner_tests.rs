use super::*;
use crate::widgets::{Divider, ProgressBar, Spinner};

#[test]
fn progress_bar_renders_track_fill_and_label() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        ProgressBar::new(1.5)
            .width(dp(220.0))
            .show_label(true)
            .label("完成"),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 48.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered.primitives.shapes.len() >= 2);
    assert!(!rendered.primitives.texts.is_empty());
    let track = rendered.primitives.shapes[0].rect;
    let fill = rendered.primitives.shapes[1].rect;
    assert!((fill.width.get() - track.width.get()).abs() <= 0.01);
}

#[test]
fn progress_bar_indeterminate_reduced_motion_uses_static_segment() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 240.0, 40.0);
    let tree: WidgetTree<()> = WidgetTree::new(ProgressBar::indeterminate(true).width(dp(220.0)));

    let mut animated_engine = AnimationEngine::default();
    let animated = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animated_engine,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );

    let mut reduced_engine = AnimationEngine::default();
    let reduced = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut reduced_engine,
        true,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );

    assert!(animated_engine.has_active_animations());
    assert!(!reduced_engine.has_active_animations());
    assert!(animated.primitives.shapes.len() >= 2);
    assert!(reduced.primitives.shapes.len() >= 2);

    let reduced_track = reduced.primitives.shapes[0].rect;
    let reduced_segment = reduced.primitives.shapes[1].rect;
    let centered_x = reduced_track.x + ((reduced_track.width - reduced_segment.width) * 0.5);
    assert!((reduced_segment.x.get() - centered_x.get()).abs() <= 0.01);
}

#[test]
fn progress_bar_indeterminate_segment_travels_through_track_edges() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 240.0, 40.0);
    let tree: WidgetTree<()> = WidgetTree::new(ProgressBar::indeterminate(true).width(dp(220.0)));
    let mut animations = AnimationEngine::default();
    let start = Instant::now();

    let (start_track, start_segment) = render_indeterminate_progress_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        start,
    );
    let (end_track, end_segment) = render_indeterminate_progress_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        start + Duration::from_millis(900),
    );

    assert_eq!(start_track.rect, end_track.rect);
    assert!(start_segment.rect.x < start_track.rect.x);
    assert!((start_segment.rect.right().get() - start_track.rect.x.get()).abs() <= 0.01);
    assert!((end_segment.rect.x.get() - end_track.rect.right().get()).abs() <= 0.01);
    assert!(end_segment.rect.right() > end_track.rect.right());

    let expected_clip_mask = Some(ClipMask {
        rect: start_track.rect,
        corner_radius: start_track.corner_radius,
    });
    assert_eq!(start_segment.clip_rect, Some(start_track.rect));
    assert_eq!(end_segment.clip_rect, Some(end_track.rect));
    assert_eq!(start_segment.clip_mask, expected_clip_mask);
    assert_eq!(end_segment.clip_mask, expected_clip_mask);
}

#[test]
fn spinner_renders_mesh_and_reduced_motion_disables_rotation_animation() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 64.0, 64.0);
    let tree: WidgetTree<()> = WidgetTree::new(Spinner::new().size(dp(28.0), dp(28.0)));

    let mut animated_engine = AnimationEngine::default();
    let animated = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animated_engine,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );

    let mut reduced_engine = AnimationEngine::default();
    let reduced = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut reduced_engine,
        true,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );

    assert!(!animated.primitives.meshes.is_empty());
    assert!(!reduced.primitives.meshes.is_empty());
    assert!(animated_engine.has_active_animations());
    assert!(!reduced_engine.has_active_animations());
}

#[test]
fn spinner_mesh_uses_dense_segments_and_antialiased_edges() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Spinner::new().size(dp(28.0), dp(28.0)));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 64.0, 64.0),
        None,
        None,
        None,
        None,
        false,
    );

    let track_mesh = rendered
        .primitives
        .meshes
        .iter()
        .max_by_key(|mesh| mesh.vertices.len())
        .expect("spinner should render a track mesh");
    let (min_alpha, max_alpha) = track_mesh
        .vertices
        .iter()
        .map(|vertex| vertex.stop_colors[0][3])
        .fold((f32::INFINITY, 0.0_f32), |(min, max), alpha| {
            (min.min(alpha), max.max(alpha))
        });

    assert!(track_mesh.vertices.len() > 400);
    assert!(min_alpha <= 0.001);
    assert!(max_alpha > 0.05);
}

fn render_indeterminate_progress_at(
    tree: &WidgetTree<()>,
    font_manager: &FontManager,
    theme: &Theme,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
    now: Instant,
) -> (
    crate::ui::widget::common::RenderPrimitive,
    crate::ui::widget::common::RenderPrimitive,
) {
    let rendered = tree
        .compute_scene_with_units_and_widget_state_at(
            font_manager,
            theme,
            media,
            UnitContext::default(),
            animations,
            false,
            None,
            None,
            &WidgetStateMap::default(),
            &HashMap::new(),
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            false,
            now,
        )
        .rendered();

    assert!(rendered.primitives.shapes.len() >= 2);
    (rendered.primitives.shapes[0], rendered.primitives.shapes[1])
}

fn render_divider(
    divider: Divider<()>,
    viewport: Rect,
) -> crate::ui::widget::common::RenderedWidgetScene {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(divider);
    tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    )
}

#[test]
fn divider_descriptor_mounts() {
    let rendered = render_divider(
        Divider::new().width(dp(200.0)),
        Rect::new(0.0, 0.0, 240.0, 24.0),
    );
    assert!(!rendered.primitives.shapes.is_empty());
}

#[test]
fn horizontal_divider_renders_single_segment() {
    // 测试环境下 root widget 由 taffy 撑满 viewport（240x24）。
    let rendered = render_divider(
        Divider::new().horizontal().width(dp(200.0)),
        Rect::new(0.0, 0.0, 240.0, 24.0),
    );
    assert_eq!(rendered.primitives.shapes.len(), 1);
    let line = rendered.primitives.shapes[0].rect;
    // 线沿水平方向铺满 frame，粗细 1dp 且垂直居中。
    assert!(line.width.get() >= 200.0, "unexpected line rect: {line:?}");
    assert!((line.height.get() - 1.0).abs() <= 0.5);
    assert!((line.y.get() - (24.0 - 1.0) * 0.5).abs() <= 0.5);
}

#[test]
fn vertical_divider_renders_single_segment() {
    let rendered = render_divider(
        Divider::new().vertical().height(dp(100.0)),
        Rect::new(0.0, 0.0, 24.0, 120.0),
    );
    assert_eq!(rendered.primitives.shapes.len(), 1);
    let line = rendered.primitives.shapes[0].rect;
    // 线沿垂直方向铺满 frame，粗细 1dp 且水平居中。
    assert!((line.width.get() - 1.0).abs() <= 0.5);
    assert!(line.height.get() >= 100.0, "unexpected line rect: {line:?}");
    assert!((line.x.get() - (24.0 - 1.0) * 0.5).abs() <= 0.5);
}

#[test]
fn dashed_divider_emits_multiple_segments() {
    let rendered = render_divider(
        Divider::new().dashed(true).width(dp(200.0)),
        Rect::new(0.0, 0.0, 240.0, 24.0),
    );
    assert!(rendered.primitives.shapes.len() > 1);
}

#[test]
fn divider_with_label_splits_line_and_emits_text() {
    let rendered = render_divider(
        Divider::new().label("OR").width(dp(200.0)),
        Rect::new(0.0, 0.0, 240.0, 24.0),
    );
    assert_eq!(rendered.primitives.shapes.len(), 2);
    assert!(!rendered.primitives.texts.is_empty());
}
