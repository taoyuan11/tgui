use super::*;

#[test]
fn canvas_without_explicit_size_uses_item_bounds_for_layout() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let canvas: Element<()> = Canvas::new(CanvasRecorder::build(|canvas| {
        canvas
            .next_item_id(1_u64)
            .set_fill(Color::WHITE)
            .begin_path()
            .move_to(0.0, 0.0)
            .line_to(80.0, 0.0)
            .line_to(80.0, 30.0)
            .line_to(0.0, 30.0)
            .close_path()
            .fill();
    }))
    .cursor(crate::ui::widget::CursorStyle::Pointer)
    .into();
    let canvas_id = canvas.id;
    let tree = WidgetTree::new(Stack::new().child(canvas));

    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    let widget_region = computed
        .hit_regions
        .iter()
        .find(|region| matches!(region.interaction, super::HitInteraction::Widget { id, .. } if id == canvas_id))
        .expect("canvas widget region should exist");
    assert_eq!(widget_region.rect.width, 80.0);
    assert_eq!(widget_region.rect.height, 30.0);
}

#[test]
fn background_brush_generates_brush_primitive() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(120.0), dp(80.0)).style(|mode| {
            container_style(
                mode,
                None,
                Some(
                    BackgroundLinearGradient::new(
                        Point::new(dp(0.0), dp(0.0)),
                        Point::new(dp(120.0), dp(80.0)),
                        vec![
                            BackgroundGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                            BackgroundGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                        ],
                    )
                    .into(),
                ),
                None,
                None,
                None,
                None,
                Some(dp(12.0)),
                None,
            )
        }));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.brushes.len(), 1);
    assert!(matches!(
        rendered.primitives.brushes[0].brush,
        crate::ui::widget::BackgroundBrush::LinearGradient(_)
    ));
}

#[test]
fn background_brush_takes_priority_over_background_color() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(Stack::new().size(dp(120.0), dp(80.0)).style(|mode| {
            container_style(
                mode,
                Some(Color::hexa(0xEF4444FF)),
                Some(
                    BackgroundRadialGradient::new(
                        Point::new(dp(60.0), dp(40.0)),
                        dp(72.0),
                        vec![
                            BackgroundGradientStop::new(0.0, Color::hexa(0xFFFFFFAA)),
                            BackgroundGradientStop::new(1.0, Color::hexa(0x2563EB00)),
                        ],
                    )
                    .into(),
                ),
                None,
                None,
                None,
                None,
                None,
                None,
            )
        }));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.brushes.len(), 1);
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .all(|shape| shape.color != Color::hexa(0xEF4444FF)));
}

#[test]
fn background_brush_keeps_clip_rect() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::new()
            .size(dp(100.0), dp(100.0))
            .overflow(Overflow::Hidden)
            .child(Stack::new().size(dp(120.0), dp(80.0)).style(|mode| {
                container_style(
                    mode,
                    None,
                    Some(
                        BackgroundLinearGradient::new(
                            Point::new(dp(0.0), dp(0.0)),
                            Point::new(dp(120.0), dp(80.0)),
                            vec![
                                BackgroundGradientStop::new(0.0, Color::hexa(0x14B8A6FF)),
                                BackgroundGradientStop::new(1.0, Color::hexa(0x0F766EFF)),
                            ],
                        )
                        .into(),
                    ),
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

    assert_eq!(rendered.primitives.brushes.len(), 1);
    assert_eq!(
        rendered.primitives.brushes[0].clip_rect,
        Some(Rect::new(0.0, 0.0, 100.0, 100.0))
    );
}
