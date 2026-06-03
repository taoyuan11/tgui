use super::*;

#[test]
fn radio_without_label_measures_to_theme_control_size() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Radio::new(false));
    let expected = UnitContext::default().resolve_dp(
        default_radio_style(&theme, crate::ui::theme::WidgetState::default(), false).size,
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| { shape.rect.width == expected && shape.rect.height == expected }));
}

#[test]
fn radio_label_extends_measure_and_hit_region() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Radio::new(false).label("Email"));
    let radio_style = default_radio_style(&theme, crate::ui::theme::WidgetState::default(), false);
    let size = UnitContext::default().resolve_dp(radio_style.size);
    let gap = UnitContext::default().resolve_dp(radio_style.label_gap);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let label = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content == "Email")
        .expect("radio label should render");

    assert_eq!(label.frame.x, size + gap);
    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 40.0),
        Some(Point::new(label.frame.right() - 1.0, label.frame.y + 1.0)),
        None,
    );
    assert!(matches!(hit, Some(super::HitInteraction::Radio { .. })));
}

#[test]
fn checked_radio_renders_indicator() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Radio::new(true));
    let checked_style = default_radio_style(&theme, crate::ui::theme::WidgetState::default(), true);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.color == checked_style.indicator));
    assert!(rendered.primitives.texts.is_empty());
}

#[test]
fn disabled_radio_exposes_disabled_hit_for_cursor_only() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Radio::new(false).disable(true));

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        Some(Point::new(4.0, 4.0)),
        None,
    );

    assert!(matches!(hit, Some(super::HitInteraction::Disabled { .. })));
}
