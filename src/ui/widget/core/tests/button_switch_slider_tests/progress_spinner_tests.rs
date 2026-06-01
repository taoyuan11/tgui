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
