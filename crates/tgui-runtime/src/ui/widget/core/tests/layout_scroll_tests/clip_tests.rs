use super::*;
use crate::ui::widget::Button;

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
                .style_full(|ctx| {
                    container_style(
                        ctx,
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
                        .style_full(|ctx| {
                            container_style(
                                ctx,
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
            .style_full(|ctx| {
                container_style(
                    ctx,
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
            .child(Stack::new().size(dp(100.0), dp(100.0)).style_full(|ctx| {
                container_style(
                    ctx,
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
            .style_full(|ctx| {
                container_style(
                    ctx,
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
            .child(Stack::new().size(dp(100.0), dp(40.0)).style_full(|ctx| {
                container_style(
                    ctx,
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

#[test]
fn rounded_overflow_clips_child_focus_ring_overlay() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let button: Element<()> = Button::new("Save")
        .size(dp(80.0), dp(36.0))
        .position_absolute()
        .left(dp(10.0))
        .top(dp(38.0))
        .into();
    let button_id = button.id;
    let tree = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(100.0), dp(64.0))
            .style_full(|ctx| {
                container_style(
                    ctx,
                    Some(crate::foundation::color::Color::WHITE),
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(dp(16.0)),
                    None,
                )
            })
            .overflow(Overflow::Hidden)
            .child(button),
    );
    let mut widget_states = WidgetStateMap::default();
    widget_states.set(
        button_id,
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
        &mut animations,
        false,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 64.0),
        None,
        None,
        None,
        None,
        false,
    );

    let focus_ring = rendered
        .primitives
        .overlay_shapes
        .iter()
        .find(|primitive| primitive.stroke_width == theme.focus_ring.width.get())
        .expect("focused child button should render a focus ring");
    assert!(
        focus_ring.rect.bottom() > dp(64.0),
        "test setup should place the focus ring beyond the clipping edge"
    );
    assert_eq!(focus_ring.clip_rect, Some(Rect::new(0.0, 0.0, 100.0, 64.0)));
    assert_eq!(
        focus_ring.clip_mask,
        Some(ClipMask {
            rect: Rect::new(0.0, 0.0, 100.0, 64.0),
            corner_radius: 16.0,
        })
    );
}

#[test]
fn scroll_collect_skips_fully_clipped_plain_subtrees() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let mut content = Flex::new(Axis::Vertical).width(dp(200.0));
    let mut offscreen_row_ids = Vec::new();

    for row in 0..10 {
        let row_element: Element<()> = Stack::new()
            .height(dp(40.0))
            .child(Text::new(format!("row {row}")))
            .into();
        if row >= 5 {
            offscreen_row_ids.push(row_element.id);
        }
        content = content.child(row_element);
    }

    let viewport = Rect::new(0.0, 0.0, 240.0, 120.0);
    let tree = WidgetTree::new(
        Stack::new()
            .size(dp(200.0), dp(120.0))
            .overflow_y(Overflow::Scroll)
            .child(content),
    );
    let now = Instant::now();
    let layout = tree.build_scene_layout_at(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        now,
    );
    let default_style_sheet = crate::ui::widget::StyleSheet::default();
    let collected = tree.collect_scene_cache_from_layout_with_focus_value_at(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        now,
        &HashMap::new(),
        None,
        None,
        &default_style_sheet,
    );

    assert!(
        offscreen_row_ids
            .iter()
            .all(|id| !collected.chunks.contains_key(id)
                && !collected.visual_contexts.contains_key(id)),
        "fully clipped plain rows should not allocate scene cache chunks"
    );
}

#[test]
fn painted_stack_overlay_occludes_button_hit() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let covered_button: Element<()> = Button::new("covered").into();
    let overlay: Element<()> = Stack::new()
        .size(dp(100.0), dp(100.0))
        .style_full(|ctx| {
            container_style(
                ctx,
                Some(crate::foundation::color::Color::hexa(0x00000055)),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
        })
        .into();
    let tree = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(100.0), dp(100.0))
            .child([covered_button, overlay]),
    );

    let computed = tree.compute_scene_with_units_and_widget_state(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    let hit_path = WidgetTree::hit_path_from_computed(&computed, Point::new(50.0, 50.0));

    assert_eq!(hit_path.len(), 1);
    assert!(matches!(
        hit_path.last(),
        Some(super::HitInteraction::Occluder { .. })
    ));
}
