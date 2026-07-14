use super::*;
use crate::ui::theme::Density;
use crate::widgets::{
    Divider, DividerStyle, Pagination, PaginationStyle, ProgressBar, ProgressBarStyle, Spinner,
    SpinnerStyle,
};

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

    let (start_track, start_segments) = render_indeterminate_progress_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        start,
    );
    let (end_track, end_segments) = render_indeterminate_progress_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        start + Duration::from_millis(900),
    );

    assert_eq!(start_track.rect, end_track.rect);
    assert_eq!(start_segments.len(), 1);
    assert_eq!(end_segments.len(), 1);
    assert!(start_segments[0].rect.x < start_track.rect.x);
    assert!((start_segments[0].rect.right().get() - start_track.rect.x.get()).abs() <= 0.01);
    assert!(
        (start_segments[0].rect.x.get() - end_segments[0].rect.x.get()).abs() <= 0.01,
        "phase wrap should keep the marquee segment continuous"
    );

    let expected_clip_mask = Some(ClipMask {
        rect: start_track.rect,
        corner_radius: start_track.corner_radius,
    });
    for segment in start_segments.iter().chain(end_segments.iter()) {
        assert_eq!(segment.clip_rect, Some(start_track.rect));
        assert_eq!(segment.clip_mask, expected_clip_mask);
    }
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

#[test]
fn feedback_components_follow_real_light_dark_and_density_scenes() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let progress: WidgetTree<()> = WidgetTree::new(
        ProgressBar::new(0.5)
            .width(dp(160.0))
            .show_label(true)
            .label("50%"),
    );
    let spinner: WidgetTree<()> = WidgetTree::new(Spinner::new());
    let divider: WidgetTree<()> = WidgetTree::new(Divider::new().dashed(true).width(dp(200.0)));

    for light in [true, false] {
        for (density, expected_progress_height, expected_spinner_size) in [
            (Density::Compact, dp(4.0), dp(16.0)),
            (Density::Comfortable, dp(6.0), dp(20.0)),
            (Density::Spacious, dp(8.0), dp(24.0)),
        ] {
            let mut theme = if light { Theme::light() } else { Theme::dark() };
            theme.density = density;

            let mut animations = AnimationEngine::default();
            let progress_scene = progress.render_output(
                &font_manager,
                &theme,
                &media,
                &mut animations,
                None,
                None,
                &HashMap::new(),
                Rect::new(0.0, 0.0, 180.0, 48.0),
                None,
                None,
                None,
                None,
                false,
            );
            let progress_style = ProgressBarStyle::default_for_theme(&theme);
            let track = &progress_scene.primitives.shapes[0];
            let fill = &progress_scene.primitives.shapes[1];
            assert_eq!(track.rect.height, expected_progress_height);
            assert_eq!(track.color, progress_style.track_color.resolve());
            assert_eq!(fill.color, theme.colors.primary);
            assert_eq!(track.corner_radius, expected_progress_height.get() * 0.5);
            let label = progress_scene
                .primitives
                .texts
                .iter()
                .find(|text| text.content.as_ref() == "50%")
                .expect("labeled progress should emit its real text primitive");
            assert_eq!(label.color, theme.colors.on_surface_muted);

            let mut animations = AnimationEngine::default();
            let spinner_scene = spinner.render_output(
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
            let spinner_style = SpinnerStyle::default_for_theme(&theme);
            assert_eq!(spinner_style.size, expected_spinner_size);
            assert_eq!(spinner_scene.primitives.meshes.len(), 2);
            let (min_x, max_x, min_y, max_y) = spinner_scene
                .primitives
                .meshes
                .iter()
                .flat_map(|mesh| mesh.vertices.iter())
                .fold(
                    (
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                    ),
                    |(min_x, max_x, min_y, max_y), vertex| {
                        (
                            min_x.min(vertex.position[0]),
                            max_x.max(vertex.position[0]),
                            min_y.min(vertex.position[1]),
                            max_y.max(vertex.position[1]),
                        )
                    },
                );
            assert!(((max_x - min_x) - expected_spinner_size.get()).abs() <= 0.01);
            assert!(((max_y - min_y) - expected_spinner_size.get()).abs() <= 0.01);

            let mut animations = AnimationEngine::default();
            let divider_scene = divider.render_output(
                &font_manager,
                &theme,
                &media,
                &mut animations,
                None,
                None,
                &HashMap::new(),
                Rect::new(0.0, 0.0, 220.0, 24.0),
                None,
                None,
                None,
                None,
                false,
            );
            let divider_style = DividerStyle::default_for_theme(&theme);
            let first_dash = divider_scene
                .primitives
                .shapes
                .first()
                .expect("dashed divider should emit a real line segment");
            assert_eq!(first_dash.color, theme.colors.outline_muted);
            assert!((first_dash.rect.width - divider_style.dash_length).abs() <= dp(0.01));
        }
    }
}

#[test]
fn pagination_real_scene_tracks_density_hover_disabled_and_selection() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> =
        WidgetTree::new(Pagination::new(1usize, 3usize).page_size_options(Vec::new()));

    for (density, expected_width, expected_height) in [
        (Density::Compact, dp(32.0), dp(32.0)),
        (Density::Comfortable, dp(40.0), dp(40.0)),
        (Density::Spacious, dp(48.0), dp(48.0)),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        let mut animations = AnimationEngine::default();
        let initial = tree.compute_scene(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 420.0, 64.0),
            None,
            None,
            None,
            None,
            false,
        );
        let rendered = initial.rendered();
        let page_rect = |label: &str| {
            let text = rendered
                .primitives
                .texts
                .iter()
                .find(|text| text.content.as_ref() == label)
                .unwrap_or_else(|| panic!("missing pagination label {label}"));
            let center = Point::new(
                text.frame.x + text.frame.width * 0.5,
                text.frame.y + text.frame.height * 0.5,
            );
            initial
                .hit_regions
                .iter()
                .find(|region| region.rect.contains(center))
                .map(|region| region.rect)
                .unwrap_or_else(|| panic!("missing hit region for pagination label {label}"))
        };
        let page_one = page_rect("1");
        let page_two = page_rect("2");
        assert_eq!(page_one.width, expected_width);
        assert_eq!(page_two.width, expected_width);
        assert_eq!(page_one.height, expected_height);
        assert_eq!(page_two.height, expected_height);
        let style = PaginationStyle::default_for_theme(&theme);
        assert!((page_two.x - page_one.right() - style.gap).abs() <= dp(0.01));
    }

    for mut theme in [Theme::light(), Theme::dark()] {
        theme.density = Density::Comfortable;
        let mut animations = AnimationEngine::default();
        let initial = tree.compute_scene(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 420.0, 64.0),
            None,
            None,
            None,
            None,
            false,
        );
        let rendered = initial.rendered();
        let page_two_text = rendered
            .primitives
            .texts
            .iter()
            .find(|text| text.content.as_ref() == "2")
            .expect("page two text should render");
        let center = Point::new(
            page_two_text.frame.x + page_two_text.frame.width * 0.5,
            page_two_text.frame.y + page_two_text.frame.height * 0.5,
        );
        let page_two_id = initial
            .hit_regions
            .iter()
            .find_map(|region| {
                if !region.rect.contains(center) {
                    return None;
                }
                match &region.interaction {
                    HitInteraction::Widget { id, .. } => Some(*id),
                    _ => None,
                }
            })
            .expect("page two should expose an interactive hit region");
        let mut states = WidgetStateMap::default();
        let mut hovered = states.get(page_two_id);
        hovered.hovered = true;
        states.set(page_two_id, hovered);
        let mut animations = AnimationEngine::default();
        let scene = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            false,
            None,
            None,
            &states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 420.0, 64.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert!(scene
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == theme.colors.primary_container));
        let hover_color = theme.colors.primary_container.with_alpha_factor(0.46);
        let hover_shape = scene
            .primitives
            .shapes
            .iter()
            .find(|shape| shape.color == hover_color)
            .expect("hovered page should emit a soft primary state layer");
        assert_eq!(hover_shape.corner_radius, theme.radius.lg.get());
        let prev = scene
            .primitives
            .texts
            .iter()
            .find(|text| text.content.as_ref() == "Prev")
            .expect("disabled previous label should render");
        assert_eq!(prev.color, theme.colors.on_disabled.with_alpha_factor(0.55));
        let selected = scene
            .primitives
            .texts
            .iter()
            .find(|text| text.content.as_ref() == "1")
            .expect("selected page label should render");
        assert_eq!(selected.color, theme.colors.on_primary_container);
    }
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
    Vec<crate::ui::widget::common::RenderPrimitive>,
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
    (
        rendered.primitives.shapes[0],
        rendered.primitives.shapes[1..2].to_vec(),
    )
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
