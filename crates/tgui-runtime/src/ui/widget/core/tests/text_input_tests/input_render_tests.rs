use super::*;

#[test]
fn pointer_focused_input_uses_active_border_without_focus_ring() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let input: Element<()> = Input::new("focused").size(dp(220.0), dp(40.0)).into();
    let input_id = input.id;
    let tree = WidgetTree::new(input);
    let mut states = WidgetStateMap::default();
    states.set(
        input_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            focused: true,
            focus_visible: false,
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
        Rect::new(0.0, 0.0, 220.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered.primitives.shapes.iter().any(|shape| {
        shape.stroke_width == theme.border.thin.get() && shape.color == theme.colors.primary
    }));
    assert!(!rendered.primitives.overlay_shapes.iter().any(|shape| {
        shape.color == theme.focus_ring.color && shape.stroke_width == theme.focus_ring.width.get()
    }));
}

#[test]
fn keyboard_focused_input_renders_theme_focus_ring_without_layout_inset() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let input: Element<()> = Input::new("focus visible").size(dp(220.0), dp(40.0)).into();
    let input_id = input.id;
    let tree = WidgetTree::new(input);
    let mut states = WidgetStateMap::default();
    states.set(
        input_id,
        crate::ui::theme::WidgetState {
            focused: true,
            focus_visible: true,
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
        Rect::new(0.0, 0.0, 220.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    let ring = rendered
        .primitives
        .overlay_shapes
        .iter()
        .find(|shape| {
            shape.color == theme.focus_ring.color
                && shape.stroke_width == theme.focus_ring.width.get()
        })
        .expect("keyboard-focused Input should render the theme focus ring");
    assert!(ring.rect.width > dp(220.0));
    assert!(ring.rect.height > dp(40.0));
}

#[test]
fn input_renders_composition_preview_text() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let text: Element<()> = Input::new("abc").into();
    let text_id = text.id;
    let tree = WidgetTree::new(text);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 60.0),
        Some(text_id),
        Some(&TextEditState {
            cursor: 2,
            anchor: 2,
            composition: Some(crate::ui::widget::CompositionState {
                replace_range: (1, 2),
                text: "XYZ".to_string(),
                cursor: Some((0, 2)),
            }),
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        Some(text_id),
        Some(&TextEditState::caret_at("abc")),
        true,
    );

    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|primitive| primitive.content.as_ref() == "aXYZc"));
}

#[test]
fn input_keeps_same_font_family_when_mixed_with_cjk_text() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let latin_tree: WidgetTree<()> = WidgetTree::new(Input::new("abc123"));
    let latin_id = latin_tree.root.id;
    let latin = latin_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 60.0),
        Some(latin_id),
        Some(&TextEditState::caret_at("abc123")),
        Some(latin_id),
        Some(&TextEditState::caret_at("abc123")),
        true,
    );

    let mixed_tree: WidgetTree<()> = WidgetTree::new(Input::new("abc123中文"));
    let mixed_id = mixed_tree.root.id;
    let mixed = mixed_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 60.0),
        Some(mixed_id),
        Some(&TextEditState::caret_at("abc123中文")),
        Some(mixed_id),
        Some(&TextEditState::caret_at("abc123中文")),
        true,
    );

    let latin_font = latin
        .primitives
        .texts
        .last()
        .expect("latin input text should be rendered")
        .font_family
        .clone();
    let mixed_font = mixed
        .primitives
        .texts
        .last()
        .expect("mixed input text should be rendered")
        .font_family
        .clone();

    assert_eq!(latin_font, mixed_font);
}

#[test]
fn input_uses_custom_selection_and_caret_colors() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let selection = Color::hexa(0x11AA33FF);
    let caret = Color::hexa(0xCC2211FF);
    let tree: WidgetTree<()> = WidgetTree::new(Input::new("hello").style_full(move |ctx| {
        let mut style = InputStyle::default_for_theme(ctx.theme);
        style.selection = Some(selection.into());
        style.caret = Some(caret.into());
        style
    }));
    let text_id = tree.root.id;

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 60.0),
        Some(text_id),
        Some(&TextEditState {
            cursor: 4,
            anchor: 1,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        Some(text_id),
        Some(&TextEditState::caret_at("hello")),
        true,
    );

    assert!(rendered
        .primitives
        .text_decorations
        .iter()
        .any(
            |primitive| primitive.color == selection.with_alpha_factor(1.0)
                && !primitive.segments.is_empty()
        ));
    assert!(rendered
        .primitives
        .overlay_text_decorations
        .iter()
        .any(|primitive| primitive.color == caret.with_alpha_factor(1.0)
            && !primitive.segments.is_empty()));
}

#[test]
fn input_validation_invalid_uses_theme_error_border() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Input::new("bad").validation(
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
        Rect::new(0.0, 0.0, 220.0, 60.0),
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
        .any(|primitive| primitive.stroke_width > 0.0 && primitive.color == theme.colors.error));
}

#[test]
fn single_line_input_scroll_clips_text_to_inner_content_rect() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Input::new("0123456789abcdef0123456789").size(dp(96.0), dp(40.0)));
    let text_id = tree.root.id;

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 96.0, 40.0),
        Some(text_id),
        Some(&TextEditState {
            cursor: "0123456789abcdef0123456789".len(),
            anchor: "0123456789abcdef0123456789".len(),
            composition: None,
            scroll_x: dp(80.0),
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        Some(text_id),
        Some(&TextEditState::caret_at("0123456789abcdef0123456789")),
        true,
    );

    let text = rendered
        .primitives
        .texts
        .last()
        .expect("input text should be rendered");
    let expected_clip = Rect::new(12.0, 8.0, 72.0, 24.0);
    assert_eq!(text.clip_rect, Some(expected_clip));
    assert!(text.frame.x < expected_clip.x);
    assert!(text.frame.width > expected_clip.width);
}
