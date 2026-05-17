use super::*;

#[test]
fn centers_text_using_actual_render_height() {
    let inner = Rect::new(12.0, 8.0, 180.0, 24.0);
    let frame = centered_text_frame(inner, 56.0, 18.0, 18.0, false);

    assert_eq!(frame.x, 12.0);
    assert_eq!(frame.y, 11.0);
    assert_eq!(frame.width, 56.0);
    assert_eq!(frame.height, 18.0);
}

#[test]
fn centers_text_horizontally_when_requested() {
    let inner = Rect::new(12.0, 8.0, 180.0, 24.0);
    let frame = centered_text_frame(inner, 56.0, 18.0, 18.0, true);

    assert_eq!(frame.x, 74.0);
    assert_eq!(frame.y, 11.0);
    assert_eq!(frame.width, 56.0);
    assert_eq!(frame.height, 18.0);
}

#[test]
fn text_background_matches_measured_text_width() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let background = crate::foundation::color::Color::RED;
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(52.0), dp(52.0)).center().child(
            Text::new("A").style(move |mode| {
                let mut style = text_style(mode, None);
                style.surface.background = Some(background.into());
                style
            }),
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

    let text = rendered
        .primitives
        .texts
        .first()
        .expect("text primitive should exist");
    let background_shape = rendered
        .primitives
        .shapes
        .iter()
        .find(|primitive| primitive.color == background && primitive.rect.width.get() < 52.0)
        .expect("text background should exist");

    assert!((background_shape.rect.width.get() - text.frame.width.get()).abs() <= 1.0);
    assert!((background_shape.rect.height.get() - text.frame.height.get()).abs() <= 1.0);
}

#[test]
fn larger_font_sizes_scale_default_line_height() {
    let theme = Theme::default();
    let mut text = Text::new("Background Effects Gallery");
    let style = text_style(resolved_theme_mode(&theme), Some(sp(30.0)));
    super::apply_text_widget_style(&mut text, &style);
    let (font_size, line_height, _) = resolved_text_metrics(&text, &theme, UnitContext::default());

    assert_eq!(font_size, 30.0);
    assert_eq!(line_height, 41.25);
}

