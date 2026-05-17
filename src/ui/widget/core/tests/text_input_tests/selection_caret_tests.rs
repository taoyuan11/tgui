use super::*;

#[test]
fn selectable_text_renders_selection_highlight() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let text: Element<()> = Text::new("hello").user_select(true).into();
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
        Rect::new(0.0, 0.0, 160.0, 40.0),
        None,
        None,
        Some(text_id),
        Some(&TextEditState {
            cursor: 5,
            anchor: 1,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|primitive| { primitive.color == theme.colors.selection.with_alpha_factor(1.0) }));
}

#[test]
fn textarea_renders_multiline_caret_on_second_line() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let text: Element<()> = Textarea::new("hello\nworld").height(dp(120.0)).into();
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
        Rect::new(0.0, 0.0, 220.0, 120.0),
        Some(text_id),
        Some(&TextEditState {
            cursor: "hello\nwo".len(),
            anchor: "hello\nwo".len(),
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        Some(text_id),
        Some(&TextEditState::caret_at("hello\nworld")),
        true,
    );

    let caret = rendered
        .primitives
        .overlay_shapes
        .last()
        .expect("caret should be rendered");
    assert!(caret.rect.y > dp(20.0));
}

