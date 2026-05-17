use super::*;

#[test]
fn clipped_children_keep_clip_rect_and_do_not_hit_outside_parent() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree = WidgetTree::new(
        Stack::new().child(
            Stack::new()
                .size(dp(100.0), dp(100.0))
                .style(|mode| {
                    container_style(
                        mode,
                        Some(Color::hexa(0x1E293BFF)),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                })
                .child(
                    Stack::new()
                        .size(dp(80.0), dp(80.0))
                        .style(|mode| {
                            container_style(
                                mode,
                                Some(Color::hexa(0x38BDF8FF)),
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                                Some(Point::new(dp(60.0), dp(0.0))),
                            )
                        })
                        .on_click(Command::new(|_: &mut ()| {})),
                ),
        ),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(
        rendered
            .primitives
            .shapes
            .last()
            .and_then(|primitive| primitive.clip_rect),
        Some(Rect::new(0.0, 0.0, 100.0, 100.0))
    );

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Some(Point::new(dp(120.0), dp(20.0))),
        None,
    );
    assert!(hit.is_none());
}

#[test]
fn overflow_clips_children_to_inside_of_border() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(100.0), dp(100.0))
            .overflow(Overflow::Hidden)
            .style(|mode| {
                container_style(
                    mode,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some((dp(4.0), crate::foundation::color::Color::WHITE)),
                    None,
                    None,
                )
            })
            .child(Stack::new().size(dp(100.0), dp(100.0)).style(|mode| {
                container_style(
                    mode,
                    Some(crate::foundation::color::Color::BLACK),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            })),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    let child_shape = rendered
        .primitives
        .shapes
        .iter()
        .find(|primitive| primitive.color == crate::foundation::color::Color::BLACK)
        .expect("child shape should exist");
    assert_eq!(child_shape.clip_rect, Some(Rect::new(4.0, 4.0, 92.0, 92.0)));
}

#[test]
fn rounded_overflow_clips_children_with_parent_corner_mask() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(100.0), dp(100.0))
            .style(|mode| {
                container_style(
                    mode,
                    Some(crate::foundation::color::Color::WHITE),
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(dp(18.0)),
                    None,
                )
            })
            .overflow(Overflow::Hidden)
            .child(Stack::new().size(dp(100.0), dp(40.0)).style(|mode| {
                container_style(
                    mode,
                    Some(crate::foundation::color::Color::BLACK),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            })),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    let child_shape = rendered
        .primitives
        .shapes
        .iter()
        .find(|primitive| primitive.color == crate::foundation::color::Color::BLACK)
        .expect("child shape should exist");
    assert_eq!(
        child_shape.clip_mask,
        Some(ClipMask {
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            corner_radius: 18.0,
        })
    );
}

