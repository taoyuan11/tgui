use super::*;

#[test]
fn wrapped_flex_align_start_packs_lines_from_cross_axis_start() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let child_color = crate::foundation::color::Color::hexa(0x22C55EFF);
    let tree: WidgetTree<()> = WidgetTree::new(
        crate::ui::widget::Flex::horizontal()
            .wrap(crate::ui::layout::Wrap::Wrap)
            .align(crate::ui::layout::Align::Start)
            .justify(crate::ui::layout::Justify::Start)
            .gap(dp(10.0))
            .child([
                Stack::new().size(dp(60.0), dp(40.0)).style(move |mode| {
                    container_style(
                        mode,
                        Some(child_color),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                }),
                Stack::new().size(dp(60.0), dp(40.0)).style(move |mode| {
                    container_style(
                        mode,
                        Some(child_color),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                }),
                Stack::new().size(dp(60.0), dp(40.0)).style(move |mode| {
                    container_style(
                        mode,
                        Some(child_color),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                }),
            ]),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 140.0, 240.0),
        None,
        None,
        None,
        None,
        false,
    );
    let child_rects: Vec<_> = rendered
        .primitives
        .shapes
        .iter()
        .filter(|shape| shape.color == child_color)
        .map(|shape| shape.rect)
        .collect();

    assert_eq!(child_rects.len(), 3);
    assert_eq!(child_rects[0], Rect::new(0.0, 0.0, 60.0, 40.0));
    assert_eq!(child_rects[1], Rect::new(70.0, 0.0, 60.0, 40.0));
    assert_eq!(child_rects[2], Rect::new(0.0, 50.0, 60.0, 40.0));
}

#[test]
fn scroll_offsets_are_clamped_to_content_bounds() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let scroller: super::Element<()> = Stack::new()
        .size(dp(100.0), dp(100.0))
        .overflow_y(Overflow::Scroll)
        .style(|mode| {
            container_style(
                mode,
                Some(crate::foundation::color::Color::hexa(0x111827FF)),
                None,
                None,
                None,
                None,
                Some((dp(4.0), crate::foundation::color::Color::WHITE)),
                None,
                None,
            )
        })
        .child(Stack::new().size(dp(100.0), dp(300.0)).style(|mode| {
            container_style(
                mode,
                Some(crate::foundation::color::Color::hexa(0x22C55EFF)),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
        }))
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(Stack::new().child(scroller));

    let mut scroll_offsets = HashMap::new();
    scroll_offsets.insert(scroller_id, Point::new(dp(0.0), dp(500.0)));
    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &scroll_offsets,
        Rect::new(0.0, 0.0, 100.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );

    let region = rendered
        .scroll_regions
        .into_iter()
        .find(|region| region.id == scroller_id)
        .expect("scroll region should exist");
    assert_eq!(region.content_viewport, Rect::new(4.0, 4.0, 92.0, 92.0));
    assert_eq!(region.scroll_offset.y, 204.0);
    assert_eq!(region.max_offset().y, 204.0);
}

#[test]
fn scroll_content_bounds_include_container_bottom_padding() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let scroller: super::Element<()> = Stack::new()
        .size(dp(100.0), dp(100.0))
        .padding(Insets::all(dp(20.0)))
        .overflow_y(Overflow::Scroll)
        .child(Stack::new().size(dp(60.0), dp(120.0)).style(|mode| {
            container_style(
                mode,
                Some(crate::foundation::color::Color::hexa(0x22C55EFF)),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
        }))
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);

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

    let region = rendered
        .scroll_regions
        .into_iter()
        .find(|region| region.id == scroller_id)
        .expect("scroll region should exist");
    assert_eq!(region.content_viewport, Rect::new(0.0, 0.0, 100.0, 100.0));
    assert_eq!(region.content_bounds.bottom(), dp(160.0));
    assert_eq!(region.max_offset().y, 60.0);
}

#[test]
fn scroll_container_with_grid_of_canvas_cards_produces_vertical_scroll_range() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let card: Element<()> = Stack::new()
        .height(dp(180.0))
        .child(
            Canvas::new(CanvasRecorder::build(|canvas| {
                canvas
                    .next_item_id(1_u64)
                    .set_fill(Color::hexa(0x1D4ED8FF))
                    .fill_rect(0.0, 0.0, 80.0, 80.0);
            }))
            .size(dp(120.0), dp(120.0)),
        )
        .into();
    let scroller: Element<()> = Stack::new()
        .size(dp(320.0), dp(240.0))
        .overflow_y(Overflow::Scroll)
        .child(
            crate::ui::widget::Grid::columns([
                crate::ui::layout::fr(1.0),
                crate::ui::layout::fr(1.0),
            ])
            .height(dp(780.0))
            .gap(dp(12.0))
            .child(card.clone())
            .child(card.clone())
            .child(card.clone())
            .child(card),
        )
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 240.0),
        None,
        None,
        None,
        None,
        false,
    );

    let region = rendered
        .scroll_regions
        .into_iter()
        .find(|region| region.id == scroller_id)
        .expect("scroll region should exist");
    assert!(region.max_offset().y > Dp::ZERO);
}

#[test]
fn scroll_containers_render_scrollbar_track_and_thumb() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let scroller: super::Element<()> = Stack::new()
        .size(dp(120.0), dp(120.0))
        .overflow_y(Overflow::Scroll)
        .style(|mode| {
            let mut style = ContainerStyle::default_for(mode);
            style.scrollbar.thumb_color = Some(crate::foundation::color::Color::BLACK);
            style.scrollbar.track_color = Some(crate::foundation::color::Color::WHITE);
            style.scrollbar.hover_thumb_color =
                Some(crate::foundation::color::Color::hexa(0x112233FF));
            style.scrollbar.active_thumb_color =
                Some(crate::foundation::color::Color::hexa(0x445566FF));
            style
        })
        .child(Stack::new().size(dp(120.0), dp(260.0)).style(|mode| {
            container_style(
                mode,
                Some(crate::foundation::color::Color::hexa(0x1D4ED8FF)),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
        }))
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    let overlay_shapes = rendered.primitives.overlay_shapes;
    assert!(overlay_shapes
        .iter()
        .any(|primitive| primitive.color == crate::foundation::color::Color::WHITE));
    assert!(overlay_shapes
        .iter()
        .any(|primitive| primitive.color == crate::foundation::color::Color::BLACK));

    let hovered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        Some(ScrollbarHandle {
            id: scroller_id,
            axis: ScrollbarAxis::Vertical,
        }),
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(hovered
        .primitives
        .overlay_shapes
        .iter()
        .any(|primitive| primitive.color == crate::foundation::color::Color::hexa(0x112233FF)));

    let active = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        Some(ScrollbarHandle {
            id: scroller_id,
            axis: ScrollbarAxis::Vertical,
        }),
        Some(ScrollbarHandle {
            id: scroller_id,
            axis: ScrollbarAxis::Vertical,
        }),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(active
        .primitives
        .overlay_shapes
        .iter()
        .any(|primitive| primitive.color == crate::foundation::color::Color::hexa(0x445566FF)));
}
