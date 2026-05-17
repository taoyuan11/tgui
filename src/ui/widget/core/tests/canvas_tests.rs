use super::*;

#[test]
fn canvas_renders_fill_and_stroke_meshes() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(1_u64)
                .set_fill(Color::hexa(0x22C55EFF))
                .set_stroke(CanvasStroke::new(dp(4.0), Color::WHITE))
                .begin_path()
                .move_to(10.0, 10.0)
                .line_to(100.0, 10.0)
                .line_to(100.0, 60.0)
                .line_to(10.0, 60.0)
                .close_path()
                .fill_and_stroke();
        }))
        .size(dp(120.0), dp(80.0)),
    );

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

    assert_eq!(rendered.primitives.meshes.len(), 2);
    assert!(!rendered.primitives.commands.is_empty());
}

#[test]
fn canvas_border_radius_clips_item_meshes() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(1_u64)
                .set_fill(Color::hexa(0x22C55EFF))
                .fill_rect(0.0, 0.0, 120.0, 80.0);
        }))
        .size(dp(120.0), dp(80.0))
        .style(|mode| canvas_style(mode, dp(18.0))),
    );

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

    let expected_clip = Some(ClipMask {
        rect: Rect::new(0.0, 0.0, 120.0, 80.0),
        corner_radius: 18.0,
    });
    let mesh_matches = rendered
        .primitives
        .meshes
        .iter()
        .all(|mesh| mesh.clip_mask == expected_clip);
    let shape_matches = rendered
        .primitives
        .shapes
        .iter()
        .all(|shape| shape.clip_mask == expected_clip);

    assert!(!rendered.primitives.meshes.is_empty() || !rendered.primitives.shapes.is_empty());
    assert!(mesh_matches);
    assert!(shape_matches);
}

#[test]
fn canvas_hit_testing_prefers_topmost_item() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(1_u64)
                .set_fill(Color::hexa(0x1D4ED8FF))
                .fill_rect(0.0, 0.0, 80.0, 80.0)
                .next_item_id(2_u64)
                .set_fill(Color::hexa(0xF97316FF))
                .fill_rect(20.0, 20.0, 70.0, 70.0);
        }))
        .size(dp(120.0), dp(120.0))
        .on_item_click(ValueCommand::new(|_: &mut (), _| {})),
    );

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 120.0),
        Some(Point::new(dp(30.0), dp(30.0))),
        None,
    );

    assert!(matches!(
        hit,
        Some(super::HitInteraction::CanvasItem { item_id, .. }) if item_id == 2_u64.into()
    ));
}

#[test]
fn canvas_text_paragraph_style_is_carried_into_text_primitive() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(1_u64)
                .set_paragraph_style(CanvasParagraphStyle {
                    wrap: CanvasTextWrap::Glyph,
                    horizontal_align: CanvasTextHorizontalAlign::Center,
                    vertical_align: CanvasTextVerticalAlign::End,
                    ..Default::default()
                })
                .draw_text(Rect::new(0.0, 0.0, 160.0, 80.0), "wrapped canvas text");
        }))
        .size(dp(160.0), dp(80.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 80.0),
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
        .expect("canvas text primitive should exist");
    assert_eq!(text.wrap, CanvasTextWrap::Glyph);
    assert_eq!(text.horizontal_align, CanvasTextHorizontalAlign::Center);
    assert_eq!(text.vertical_align, CanvasTextVerticalAlign::End);
}

#[test]
fn canvas_rotated_text_generates_transformed_quad() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .save()
                .rotate(0.5)
                .next_item_id(1_u64)
                .draw_text(Rect::new(20.0, 20.0, 80.0, 40.0), "rotate me")
                .restore();
        }))
        .size(dp(120.0), dp(120.0)),
    );

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

    let text = rendered
        .primitives
        .texts
        .first()
        .expect("canvas text should render");
    assert!(text.quad.is_some());
}

#[test]
fn canvas_clip_and_layer_emit_composite_commands() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Canvas::new(CanvasRecorder::build(|canvas| {
            canvas
                .save()
                .begin_path()
                .rounded_rect(0.0, 0.0, 80.0, 80.0, 12.0)
                .clip()
                .next_item_id(1_u64)
                .set_fill(Color::WHITE)
                .fill_rect(0.0, 0.0, 80.0, 80.0)
                .restore()
                .save()
                .begin_path()
                .circle(40.0, 40.0, 24.0)
                .clip()
                .next_item_id(2_u64)
                .set_fill(Color::hexa(0x1D4ED8FF))
                .fill_rect(20.0, 20.0, 40.0, 40.0)
                .restore();
        }))
        .size(dp(120.0), dp(120.0)),
    );

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

    let composite_count = rendered
        .primitives
        .commands
        .iter()
        .filter(|command| {
            matches!(
                command,
                crate::ui::widget::RenderCommand::CanvasComposite(_)
            )
        })
        .count();
    assert!(composite_count >= 2);
}

#[test]
fn canvas_composite_bounds_include_canvas_origin() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::new().padding(Insets::all(dp(24.0))).child(
            Canvas::new(CanvasRecorder::build(|canvas| {
                canvas
                    .save()
                    .begin_path()
                    .rounded_rect(10.0, 12.0, 40.0, 30.0, 8.0)
                    .clip()
                    .next_item_id(1_u64)
                    .set_fill(Color::hexa(0x1D4ED8FF))
                    .fill_rect(10.0, 12.0, 40.0, 30.0)
                    .restore();
            }))
            .size(dp(120.0), dp(120.0)),
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
        Rect::new(0.0, 0.0, 180.0, 180.0),
        None,
        None,
        None,
        None,
        false,
    );

    let composite = rendered
        .primitives
        .commands
        .iter()
        .find_map(|command| match command {
            crate::ui::widget::RenderCommand::CanvasComposite(primitive) => Some(primitive),
            _ => None,
        })
        .expect("composite command should exist");

    assert_eq!(composite.bounds.x, dp(34.0));
    assert_eq!(composite.bounds.y, dp(36.0));
}

#[test]
fn canvas_outside_clip_does_not_emit_composite_commands() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::new()
            .size(dp(120.0), dp(120.0))
            .overflow_y(Overflow::Scroll)
            .child(
                Stack::new().height(dp(520.0)).child(
                    Canvas::new(CanvasRecorder::build(|canvas| {
                        canvas
                            .save()
                            .begin_path()
                            .rounded_rect(0.0, 0.0, 80.0, 80.0, 12.0)
                            .clip()
                            .next_item_id(1_u64)
                            .set_fill(Color::hexa(0x1D4ED8FF))
                            .fill_rect(0.0, 0.0, 80.0, 80.0)
                            .restore();
                    }))
                    .size(dp(120.0), dp(120.0))
                    .top(dp(340.0)),
                ),
            ),
    );

    let mut scroll_offsets = HashMap::new();
    scroll_offsets.insert(tree.root.id, Point::new(dp(0.0), dp(220.0)));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &scroll_offsets,
        Rect::new(0.0, 0.0, 120.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered.primitives.commands.iter().all(|command| !matches!(
        command,
        crate::ui::widget::RenderCommand::CanvasComposite(_)
    )));
}

