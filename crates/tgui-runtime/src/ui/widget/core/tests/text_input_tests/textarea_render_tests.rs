use super::*;

#[test]
fn keyboard_focused_textarea_uses_active_border_without_second_focus_ring() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let textarea: Element<()> = Textarea::new("first line\nsecond line")
        .size(dp(220.0), dp(72.0))
        .into();
    let textarea_id = textarea.id;
    let tree = WidgetTree::new(textarea);
    let mut states = WidgetStateMap::default();
    states.set(
        textarea_id,
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
        Rect::new(0.0, 0.0, 220.0, 72.0),
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
fn textarea_uses_scroll_offset_when_unfocused() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let text: Element<()> = Textarea::new("line 0\nline 1\nline 2\nline 3")
        .height(dp(52.0))
        .into();
    let text_id = text.id;
    let tree = WidgetTree::new(text);
    let viewport = Rect::new(0.0, 0.0, 220.0, 52.0);

    let baseline = tree.render_output(
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
    );

    let mut scroll_offsets = HashMap::new();
    scroll_offsets.insert(text_id, Point::new(Dp::ZERO, dp(18.0)));
    let scrolled = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &scroll_offsets,
        viewport,
        None,
        None,
        None,
        None,
        false,
    );

    let baseline_text = baseline
        .primitives
        .texts
        .iter()
        .find(|primitive| primitive.content.contains("line 0"))
        .expect("baseline textarea text should render");
    let scrolled_text = scrolled
        .primitives
        .texts
        .iter()
        .find(|primitive| primitive.content.contains("line 0"))
        .expect("scrolled textarea text should render");

    assert!(scrolled_text.frame.y < baseline_text.frame.y);
}

#[test]
fn textarea_only_emits_visible_text_primitives_for_large_content() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let content = (0..100)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let tree: WidgetTree<()> = WidgetTree::new(Textarea::new(content).height(dp(52.0)));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 52.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered.primitives.texts.len() <= 3);
    assert!(rendered
        .primitives
        .texts
        .iter()
        .all(|primitive| !primitive.content.contains("line 50")));
}

#[test]
fn textarea_shows_scrollbar_by_default() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Textarea::new("line 0\nline 1\nline 2\nline 3\nline 4\nline 5").height(dp(52.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 52.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(!rendered.scroll_regions.is_empty());
    assert!(rendered
        .scroll_regions
        .iter()
        .any(|region| region.vertical_thumb.is_some()));
    assert!(!rendered.primitives.overlay_shapes.is_empty());
}

#[test]
fn textarea_keeps_wrapped_text_and_caret_clear_of_vertical_scrollbar() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let content = "W".repeat(240);
    let text: Element<()> = Textarea::new(content.clone())
        .size(dp(220.0), dp(52.0))
        .auto_wrap(true)
        .into();
    let text_id = text.id;
    let tree = WidgetTree::new(text);
    let viewport = Rect::new(0.0, 0.0, 220.0, 52.0);

    let baseline = tree.render_output(
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
    );

    let baseline_region = baseline
        .scroll_regions
        .iter()
        .find(|region| region.id == text_id)
        .expect("textarea should register a scroll region");
    let style = TextareaStyle::default_for_theme(&theme);
    let text = super::text_with_typography(content.clone(), &style.text_style);
    let (font_size, line_height, letter_spacing) =
        resolved_text_metrics(&text, &theme, UnitContext::default());
    let request = TextFontRequest {
        preferred_font: text.font_family.as_deref().or(theme
            .typography
            .body
            .font_family
            .as_deref()),
        weight: text.font_weight.unwrap_or(theme.typography.body.weight),
    };
    let layout = font_manager.measure_text_layout_wrapped(
        &content,
        request,
        font_size,
        line_height,
        letter_spacing,
        crate::ui::widget::text_input_layout_width(
            baseline_region.content_viewport,
            true,
            true,
            super::CARET_WIDTH,
        ),
    );
    let cursor = layout.line_end(0);

    let focused = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        Some(text_id),
        Some(&TextEditState {
            cursor,
            anchor: cursor,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }),
        Some(text_id),
        Some(&TextEditState::caret_at(&content)),
        true,
    );

    let scroll_region = focused
        .scroll_regions
        .iter()
        .find(|region| region.id == text_id)
        .expect("textarea should register a scroll region");
    let vertical_track = scroll_region
        .vertical_track
        .expect("textarea should show a vertical scrollbar");
    let max_right = vertical_track.x + dp(0.1);

    assert!(focused
        .primitives
        .texts
        .iter()
        .all(|primitive| primitive.frame.right() <= max_right));

    let caret = focused
        .primitives
        .overlay_text_decorations
        .iter()
        .flat_map(|primitive| primitive.segments.iter())
        .find(|rect| (rect.width.get() - super::CARET_WIDTH).abs() <= 0.01)
        .expect("caret should be rendered");
    assert!(
        caret.right() <= max_right,
        "caret_right={} track_x={} viewport_right={}",
        caret.right().get(),
        vertical_track.x.get(),
        scroll_region.content_viewport.right().get(),
    );
}

#[test]
fn textarea_can_hide_scrollbar() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Textarea::new("line 0\nline 1\nline 2\nline 3\nline 4\nline 5")
            .height(dp(52.0))
            .show_scrollbar(false),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 52.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .scroll_regions
        .iter()
        .any(|region| region.vertical_thumb.is_some() || region.horizontal_thumb.is_some()));
    assert!(rendered.primitives.overlay_shapes.is_empty());
}

#[test]
fn textarea_auto_wrap_false_enables_horizontal_scroll_region() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let long_line = "0123456789abcdef0123456789abcdef0123456789abcdef";
    let tree: WidgetTree<()> = WidgetTree::new(
        Textarea::new(long_line)
            .size(dp(120.0), dp(60.0))
            .auto_wrap(false),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 60.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .scroll_regions
        .iter()
        .any(|region| region.overflow_x == Overflow::Scroll));
}
