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
        .find(|text| text.content.as_ref() == "Email")
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
fn radio_label_registers_text_color_animation_key() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let radio: Element<()> = Radio::new(false).label("Email").into();
    let radio_id = radio.id;
    let tree: WidgetTree<()> = WidgetTree::new(radio);

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

    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|text| text.content.as_ref() == "Email"));
    assert!(
        animations.contains_key(crate::animation::AnimationKey::Widget {
            id: radio_id.raw(),
            property: crate::animation::WidgetProperty::TextColor,
        })
    );
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
fn default_radio_uses_one_outline_shape_and_an_optional_indicator() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 80.0, 40.0);

    let mut unchecked_animations = AnimationEngine::default();
    let unchecked: WidgetTree<()> = WidgetTree::new(Radio::new(false));
    let unchecked = unchecked.render_output(
        &font_manager,
        &theme,
        &media,
        &mut unchecked_animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert_eq!(unchecked.primitives.shapes.len(), 1);
    assert_eq!(unchecked.primitives.overlay_shapes.len(), 0);

    let mut checked_animations = AnimationEngine::default();
    let checked: WidgetTree<()> = WidgetTree::new(Radio::new(true));
    let checked = checked.render_output(
        &font_manager,
        &theme,
        &media,
        &mut checked_animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert_eq!(checked.primitives.shapes.len(), 1);
    assert_eq!(checked.primitives.overlay_shapes.len(), 1);
}

#[test]
fn filled_radio_override_restores_the_background_shape() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let fill = Color::hexa(0xE2E8F0FF);
    let tree: WidgetTree<()> = WidgetTree::new(Radio::new(false).style(move |style, _| {
        style.background = StateValue::new(fill.into());
        style.background_checked = StateValue::new(fill.into());
    }));
    let mut animations = AnimationEngine::default();
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

    assert_eq!(rendered.primitives.shapes.len(), 2);
    assert_eq!(rendered.primitives.shapes[0].color, fill);
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
