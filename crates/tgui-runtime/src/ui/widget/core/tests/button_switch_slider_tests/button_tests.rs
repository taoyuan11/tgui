use super::*;

#[test]
fn button_label_is_horizontally_centered_but_text_is_not() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let text_tree: WidgetTree<()> = WidgetTree::new(
        Text::new("Center")
            .padding(Insets::all(dp(16.0)))
            .size(dp(160.0), dp(48.0)),
    );
    let text_render = text_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 48.0),
        None,
        None,
        None,
        None,
        false,
    );

    let button_tree: WidgetTree<()> =
        WidgetTree::new(crate::ui::widget::Button::new("Center").size(dp(160.0), dp(48.0)));
    let button_render = button_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 48.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(text_render.primitives.texts.len(), 1);
    assert_eq!(button_render.primitives.texts.len(), 1);
    assert!(button_render.primitives.texts[0].frame.x > text_render.primitives.texts[0].frame.x);
}

#[test]
fn disabled_button_exposes_disabled_hit_for_cursor_only() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::Button::new("disabled")
            .disable(true)
            .size(dp(120.0), dp(40.0)),
    );

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        Some(Point::new(dp(10.0), dp(10.0))),
        None,
    );
    assert!(matches!(hit, Some(super::HitInteraction::Disabled { .. })));
}

#[test]
fn button_uses_theme_radius_by_default() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(crate::ui::widget::Button::new("radius").size(dp(120.0), dp(40.0)));
    let default_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState::default(),
        crate::ui::widget::common::ButtonVariantKind::Primary,
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
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
        .any(|shape| shape.corner_radius == default_style.radius.get()));
    assert!(!rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width > 0.0 && shape.color == default_style.border_color));
}

#[test]
fn primary_button_hover_background_uses_transition() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let button: Element<()> = crate::ui::widget::Button::new("hover")
        .size(dp(120.0), dp(40.0))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(button);
    let mut hovered_state = WidgetStateMap::default();
    hovered_state.set(
        button_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
    );

    let normal = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let start_background = normal
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0)
        .expect("button should render a filled background")
        .color;

    let hovered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &hovered_state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let immediate_background = hovered
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.stroke_width == 0.0)
        .expect("hovered button should render a filled background")
        .color;

    let start_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState::default(),
        crate::ui::widget::common::ButtonVariantKind::Primary,
    );
    let hovered_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState {
            hovered: true,
            ..Default::default()
        },
        crate::ui::widget::common::ButtonVariantKind::Primary,
    );
    let mut sampled_transition = None;
    let mut settled_background = immediate_background;

    for _ in 0..18 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            false,
            None,
            None,
            &hovered_state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let background = rendered
            .primitives
            .shapes
            .iter()
            .find(|shape| shape.stroke_width == 0.0)
            .expect("hovered button should keep a filled background")
            .color;
        if background != start_background && background != hovered_style.background {
            sampled_transition = Some(background);
        }
        settled_background = background;
        if background == hovered_style.background {
            break;
        }
    }

    assert_eq!(start_background, start_style.background);
    assert_eq!(immediate_background, start_background);
    assert_eq!(settled_background, hovered_style.background);
    if let Some(mid_background) = sampled_transition {
        assert_ne!(mid_background, start_background);
        assert_ne!(mid_background, hovered_style.background);
    }
}

#[test]
fn pressed_button_background_takes_priority_over_focus_fill() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let button: Element<()> = crate::ui::widget::Button::new("focus")
        .size(dp(120.0), dp(40.0))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(button);
    let mut state = WidgetStateMap::default();
    state.set(
        button_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            focused: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let pressed_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            focused: true,
            ..Default::default()
        },
        crate::ui::widget::common::ButtonVariantKind::Primary,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width == 0.0 && shape.color == pressed_style.background));
}

#[test]
fn focused_secondary_button_keeps_default_border() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let button: Element<()> = crate::ui::widget::Button::new("focus")
        .secondary()
        .size(dp(120.0), dp(40.0))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(button);
    let mut state = WidgetStateMap::default();
    state.set(
        button_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            focused: true,
            focus_visible: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let focused_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            focused: true,
            focus_visible: true,
            ..Default::default()
        },
        crate::ui::widget::common::ButtonVariantKind::Secondary,
    );
    let default_style = default_button_style(
        &theme,
        Default::default(),
        crate::ui::widget::common::ButtonVariantKind::Secondary,
    );
    let hovered_pressed_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            ..Default::default()
        },
        crate::ui::widget::common::ButtonVariantKind::Secondary,
    );

    assert_eq!(
        focused_style.border_color,
        hovered_pressed_style.border_color
    );

    assert!(
        rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.stroke_width > 0.0
                && shape.color == hovered_pressed_style.border_color)
    );
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.stroke_width == theme.focus_ring.width.get()
            && shape.color == theme.focus_ring.color
            && shape.rect.width > dp(120.0)));
    assert_eq!(default_style.border_color, default_style.border_color);
}

#[test]
fn explicit_button_transparent_border_overrides_theme_border() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::Button::new("border")
            .style_full(|ctx| button_style(ctx, None, Some(dp(0.0)), Some(Color::TRANSPARENT)))
            .size(dp(120.0), dp(40.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let default_style = default_button_style(
        &theme,
        crate::ui::theme::WidgetState::default(),
        crate::ui::widget::common::ButtonVariantKind::Primary,
    );

    assert!(!rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.stroke_width > 0.0 && shape.color == default_style.border_color));
}

#[test]
fn explicit_button_radius_overrides_theme_radius() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::Button::new("radius")
            .style_full(|ctx| button_style(ctx, Some(dp(12.0)), None, None))
            .size(dp(120.0), dp(40.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
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
        .any(|shape| shape.corner_radius == 12.0));
}
