use super::*;

#[test]
fn checkbox_without_label_measures_to_theme_box_size() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(false));
    let expected = UnitContext::default().resolve_dp(
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), false).size,
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
fn checkbox_label_extends_measure_and_hit_region() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(false).label("Accept"));
    let checkbox_style =
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), false);
    let size = UnitContext::default().resolve_dp(checkbox_style.size);
    let gap = UnitContext::default().resolve_dp(checkbox_style.label_gap);

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
        .find(|text| text.content == "Accept")
        .expect("checkbox label should render");

    assert_eq!(label.frame.x, size + gap);
    assert!(label.frame.y >= Dp::ZERO);
    assert!(label.frame.y <= dp(12.0));
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
    assert!(matches!(hit, Some(super::HitInteraction::Checkbox { .. })));
}

#[test]
fn checked_checkbox_renders_checked_background_and_checkmark() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(true));
    let checked_style =
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), true);

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
        .any(|shape| shape.color == checked_style.background));
    let checkmark = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content == super::CHECKBOX_CHECKMARK_ICON)
        .expect("checked checkbox should render checkmark icon");
    assert_eq!(checkmark.color, Color::WHITE);
    assert!(checkmark.force_color);
    assert!(checkmark.font_family.is_some());
    let checkmark_center_x = checkmark.frame.x + checkmark.frame.width / 2.0;
    let checkmark_center_y = checkmark.frame.y + checkmark.frame.height / 2.0;
    assert!((checkmark_center_x - Dp::new(8.0)).abs().get() < 0.01);
    assert!((checkmark_center_y - Dp::new(21.0)).abs().get() < 0.01);
}

#[test]
fn hovered_checkbox_uses_primary_border_without_changing_background() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let checkbox: Element<()> = Checkbox::new(false).into();
    let checkbox_id = checkbox.id;
    let tree: WidgetTree<()> = WidgetTree::new(checkbox);
    let mut states = WidgetStateMap::default();
    states.set(
        checkbox_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        None,
        None,
        &states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let hovered_style = default_checkbox_style(
        &theme,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| { shape.stroke_width == 0.0 && shape.color == hovered_style.background }));
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| { shape.stroke_width > 0.0 && shape.color == hovered_style.border }));
}

#[test]
fn checkbox_checked_content_switches_without_animation() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let checkbox: Element<()> = Checkbox::new(false).into();
    let checkbox_id = checkbox.id;
    let unchecked_tree: WidgetTree<()> = WidgetTree::new(checkbox.clone());

    unchecked_tree.render_output(
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
    assert!(!animations.has_active_animations());

    let mut checked_checkbox: Element<()> = Checkbox::new(true).into();
    checked_checkbox.id = checkbox_id;
    let checked_tree: WidgetTree<()> = WidgetTree::new(checked_checkbox);
    let checked = checked_tree.render_output(
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
    let checked_style =
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), true);
    let checked_fill = checked
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0 && shape.color == checked_style.background)
        .expect("checked fill should render immediately");
    let control_size = UnitContext::default().resolve_dp(checked_style.size);
    assert_eq!(checked_fill.rect.width, control_size);
    assert_eq!(checked_fill.rect.height, control_size);
    assert!(!animations.has_active_animations());

    let unchecked = unchecked_tree.render_output(
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
    assert!(unchecked.primitives.shapes.iter().all(|shape| {
        shape.stroke_width == 0.0 && shape.color != checked_style.background
            || shape.stroke_width > 0.0
    }));
    assert!(!animations.has_active_animations());
}

#[test]
fn focused_unchecked_checkbox_keeps_default_box_colors() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let checkbox: Element<()> = Checkbox::new(false).into();
    let checkbox_id = checkbox.id;
    let tree: WidgetTree<()> = WidgetTree::new(checkbox);
    let mut states = WidgetStateMap::default();
    states.set(
        checkbox_id,
        crate::ui::theme::WidgetState {
            focused: true,
            ..Default::default()
        },
    );
    let default_style =
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), false);

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        None,
        None,
        &states,
        &HashMap::new(),
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
        .any(|shape| shape.stroke_width == 0.0 && shape.color == default_style.background));
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width > 0.0 && shape.color == default_style.border));
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.stroke_width == theme.focus_ring.width.get()
            && shape.color == theme.focus_ring.color
            && shape.rect.width > dp(16.0)));
    assert!(rendered
        .primitives
        .texts
        .iter()
        .all(|text| text.content != super::CHECKBOX_CHECKMARK_ICON));
}

#[test]
fn disabled_checkbox_exposes_disabled_hit_for_cursor_only() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(false).disable(true));

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

