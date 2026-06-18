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
        .text_decorations
        .iter()
        .any(|primitive| {
            primitive.color == theme.colors.selection.with_alpha_factor(1.0)
                && !primitive.segments.is_empty()
        }));
}

#[test]
fn multiline_selectable_text_selection_uses_single_bounded_decoration() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let content = "line 1\nline 2\nline 3";
    let text: Element<()> = Text::new(content).user_select(true).into();
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
        None,
        None,
        Some(text_id),
        Some(&TextEditState {
            cursor: content.len(),
            anchor: 0,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        false,
    );

    assert_eq!(rendered.primitives.text_decorations.len(), 1);
    assert!(
        rendered.primitives.text_decorations[0].segments.len() > 1,
        "multiline selection should store multiple rects inside one decoration"
    );
    assert_eq!(
        rendered
            .primitives
            .commands
            .iter()
            .filter(|command| matches!(
                command,
                crate::ui::widget::RenderCommand::TextDecoration(_)
            ))
            .count(),
        1,
        "selection must stay one scene command regardless of line count"
    );
}

#[test]
fn multiline_selectable_text_uses_full_primitive_height_without_selection() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let content = "line 1\nline 2\nline 3";
    let text_widget = Text::new(content).user_select(true);
    let (_, line_height, _) = resolved_text_metrics(&text_widget, &theme, UnitContext::default());
    let text: Element<()> = text_widget.into();
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
        None,
        None,
        None,
        None,
        false,
    );

    let primitive = rendered
        .primitives
        .texts
        .iter()
        .find(|primitive| primitive.content.as_ref() == content)
        .expect("selectable text primitive should be rendered");
    assert!(
        primitive.frame.height.get() >= line_height * 3.0,
        "multiline text should render with enough height for all lines, got {:?}",
        primitive.frame.height
    );
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
        .overlay_text_decorations
        .last()
        .expect("caret should be rendered");
    let caret_rect = caret
        .segments
        .last()
        .expect("caret segment should be rendered");
    assert!(caret_rect.y > dp(20.0));
}
