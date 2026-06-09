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
        .find(|text| text.content.as_ref() == "Accept")
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
fn checkbox_label_registers_text_color_animation_key() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let checkbox: Element<()> = Checkbox::new(false).label("Accept").into();
    let checkbox_id = checkbox.id;
    let tree: WidgetTree<()> = WidgetTree::new(checkbox);

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
        .any(|text| text.content.as_ref() == "Accept"));
    assert!(
        animations.contains_key(crate::animation::AnimationKey::Widget {
            id: checkbox_id.raw(),
            property: crate::animation::WidgetProperty::TextColor,
        })
    );
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
    let box_fill = rendered
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0 && shape.color == checked_style.background)
        .expect("checked checkbox should render checked fill");
    let checkmark = rendered
        .primitives
        .textures
        .iter()
        .find(|texture| texture.opacity > 0.0)
        .expect("checked checkbox should render checkmark icon");
    let checkmark_center_x = checkmark.frame.x + checkmark.frame.width / 2.0;
    let checkmark_center_y = checkmark.frame.y + checkmark.frame.height / 2.0;
    let box_center_x = box_fill.rect.x + box_fill.rect.width / 2.0;
    let box_center_y = box_fill.rect.y + box_fill.rect.height / 2.0;
    assert!((checkmark_center_x - box_center_x).abs().get() < 0.01);
    assert!((checkmark_center_y - box_center_y).abs().get() < 0.01);
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
        false,
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
fn checkbox_validation_invalid_uses_theme_error_border() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(false).validation(
        crate::foundation::form::ValidationVisualState {
            invalid: true,
            ..Default::default()
        },
    ));

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
        .any(|shape| shape.stroke_width > 0.0 && shape.color == theme.colors.error));
}

#[test]
fn checkbox_checked_content_uses_default_transition() {
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
    let immediate_checked_fill = checked
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0)
        .expect("checkbox fill should render");
    let control_size = UnitContext::default().resolve_dp(checked_style.size);
    assert_eq!(immediate_checked_fill.rect.width, control_size);
    assert_eq!(immediate_checked_fill.rect.height, control_size);
    assert_ne!(immediate_checked_fill.color, checked_style.background);
    assert!(animations.has_active_animations());

    let mut sampled_transition = false;
    let mut settled_checked = false;
    for _ in 0..18 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let rendered = checked_tree.render_output(
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
        let fill = rendered
            .primitives
            .shapes
            .iter()
            .find(|shape| shape.stroke_width == 0.0)
            .expect("checkbox fill should keep rendering");
        if fill.color != immediate_checked_fill.color && fill.color != checked_style.background {
            sampled_transition = true;
        }
        let checkmark_alpha = rendered
            .primitives
            .textures
            .iter()
            .map(|texture| texture.opacity)
            .fold(0.0, f32::max);
        if fill.color == checked_style.background && checkmark_alpha >= 1.0 {
            settled_checked = true;
            break;
        }
    }
    assert!(sampled_transition);
    assert!(settled_checked);

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
    let immediate_unchecked_fill = unchecked
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0)
        .expect("checkbox fill should render while unchecking");
    assert_eq!(immediate_unchecked_fill.color, checked_style.background);
    assert!(animations.has_active_animations());

    let unchecked_style =
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), false);
    let mut settled_unchecked = false;
    for _ in 0..18 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let rendered = unchecked_tree.render_output(
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
        let fill = rendered
            .primitives
            .shapes
            .iter()
            .find(|shape| shape.stroke_width == 0.0)
            .expect("checkbox fill should keep rendering while unchecking");
        let has_checkmark = rendered
            .primitives
            .textures
            .iter()
            .any(|texture| texture.opacity > 0.0);
        if fill.color == unchecked_style.background && !has_checkmark {
            settled_unchecked = true;
            break;
        }
    }
    assert!(settled_unchecked);
}

#[test]
fn checkbox_checked_content_respects_reduced_motion() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let checkbox: Element<()> = Checkbox::new(false).into();
    let checkbox_id = checkbox.id;
    let unchecked_tree: WidgetTree<()> = WidgetTree::new(checkbox.clone());

    unchecked_tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        true,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    let mut checked_checkbox: Element<()> = Checkbox::new(true).into();
    checked_checkbox.id = checkbox_id;
    let checked_tree: WidgetTree<()> = WidgetTree::new(checked_checkbox);
    let checked = checked_tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        true,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
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
        .find(|shape| shape.stroke_width == 0.0)
        .expect("checked checkbox fill should render immediately");
    assert_eq!(checked_fill.color, checked_style.background);
    let checkmark = checked
        .primitives
        .textures
        .iter()
        .find(|texture| texture.opacity > 0.0)
        .expect("checked checkbox should render checkmark immediately");
    assert_eq!(checkmark.opacity, 1.0);
    assert!(!animations.has_active_animations());

    let unchecked = unchecked_tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        true,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let unchecked_style =
        default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), false);
    let unchecked_fill = unchecked
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0)
        .expect("unchecked checkbox fill should render immediately");
    assert_eq!(unchecked_fill.color, unchecked_style.background);
    assert!(unchecked.primitives.textures.is_empty());
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
        false,
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
    assert!(rendered.primitives.textures.is_empty());
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
