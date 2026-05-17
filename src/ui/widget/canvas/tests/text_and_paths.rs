use super::*;

#[test]
fn canvas_text_overflow_ellipsis_can_be_configured() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas
            .set_paragraph_style(super::super::CanvasParagraphStyle {
                overflow: CanvasTextOverflow::Ellipsis,
                ..Default::default()
            })
            .draw_text(Rect::new(0.0, 0.0, 60.0, 20.0), "hello");
    });
    let rendered = rendered_items(&scene);
    let text = rendered[0]
        .output
        .texts
        .first()
        .expect("text primitive should exist");

    assert_eq!(text.overflow, CanvasTextOverflow::Ellipsis);
}

#[test]
fn rich_text_records_span_payload() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas.draw_rich_text(
            Rect::new(0.0, 0.0, 120.0, 32.0),
            vec![
                CanvasTextSpan::new("Hello ").style(CanvasTextStyle {
                    color: Color::WHITE,
                    ..Default::default()
                }),
                CanvasTextSpan::new("Canvas").style(CanvasTextStyle {
                    color: Color::hexa(0x38BDF8FF),
                    font_weight: FontWeight::Bold,
                    ..Default::default()
                }),
            ],
        );
    });
    let rendered = rendered_items(&scene);
    let text = rendered[0]
        .output
        .texts
        .first()
        .expect("text primitive should exist");

    let spans = text.rich_spans.as_ref().expect("rich spans should exist");
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].content, "Hello ");
    assert_eq!(spans[1].content, "Canvas");
    assert_eq!(spans[1].font_weight, FontWeight::Bold);
}

#[test]
fn mask_records_composite_mask_commands() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas.save();
        canvas.circle(30.0, 30.0, 20.0).mask();
        canvas.fill_rect(0.0, 0.0, 60.0, 60.0);
        canvas.restore();
    });
    let rendered = rendered_items(&scene);
    let composite = rendered[0]
        .output
        .commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::CanvasComposite(primitive) => Some(primitive),
            _ => None,
        })
        .expect("composite should exist");

    assert!(composite.mask_commands.is_some());
}

#[test]
fn blur_and_color_filter_effects_flow_into_composite() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas
            .set_effects(vec![
                CanvasEffect::Blur(dp(6.0)),
                CanvasEffect::ColorFilter(CanvasColorFilter::tint(
                    Color::hexa(0x22C55EFF),
                    0.4,
                )),
            ])
            .fill_rect(0.0, 0.0, 40.0, 40.0);
    });
    let rendered = rendered_items(&scene);
    let composite = rendered[0]
        .output
        .commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::CanvasComposite(primitive) => Some(primitive),
            _ => None,
        })
        .expect("effect stack should force composite");

    assert!(composite.blur_radius > 0.0);
    assert!(composite.color_filter.is_some());
}

#[test]
fn inner_shadow_effect_flows_into_composite() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas
            .set_effects(vec![CanvasEffect::InnerShadow(
                super::super::CanvasInnerShadow::new(
                    Color::hexa(0x111827AA),
                    Point::new(3.0, 4.0),
                    dp(8.0),
                ),
            )])
            .fill_rect(0.0, 0.0, 40.0, 40.0);
    });
    let rendered = rendered_items(&scene);
    let composite = rendered[0]
        .output
        .commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::CanvasComposite(primitive) => Some(primitive),
            _ => None,
        })
        .expect("effect stack should force composite");

    assert_eq!(composite.inner_shadow_color, Some(Color::hexa(0x111827AA)));
    assert_eq!(composite.inner_shadow_offset, Point::new(3.0, 4.0));
    assert_eq!(composite.inner_shadow_blur_radius, 8.0);
}

#[test]
fn svg_elliptical_arc_generates_curve_segments() {
    let path = PathBuilder::new()
        .svg_path("M 10 10 A 30 20 0 0 1 60 40")
        .expect("svg path should parse");

    assert!(path
        .commands_internal()
        .iter()
        .any(|command| matches!(command, PathCommand::CubicTo { .. })));
}

#[test]
fn even_odd_boolean_conversion_preserves_hole() {
    let path = PathBuilder::new()
        .rect(0.0, 0.0, 100.0, 100.0)
        .rect(25.0, 25.0, 50.0, 50.0)
        .fill_rule(CanvasFillRule::EvenOdd);

    let polygon = path
        .to_multi_polygon_with_rule(CanvasFillRule::EvenOdd)
        .expect("closed rings should polygonize");

    assert_eq!(polygon.0.len(), 1);
    assert_eq!(polygon.0[0].interiors().len(), 1);
}

#[test]
fn path_boolean_difference_returns_hollow_shape() {
    let outer = PathBuilder::new().rect(0.0, 0.0, 100.0, 100.0);
    let inner = PathBuilder::new().rect(25.0, 25.0, 50.0, 50.0);
    let diff = outer.difference(&inner).expect("difference should succeed");
    let polygon = diff
        .to_multi_polygon_with_rule(CanvasFillRule::NonZero)
        .expect("difference result should polygonize");

    assert_eq!(polygon.0.len(), 1);
    assert_eq!(polygon.0[0].interiors().len(), 1);
}

#[test]
fn draw_image_with_options_records_configuration() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas.draw_image_with_options(
            Rect::new(0.0, 0.0, 100.0, 60.0),
            MediaSource::bytes(vec![137, 80, 78, 71]),
            CanvasImageOptions::new()
                .fit(ContentFit::Cover)
                .corner_radius(dp(12.0))
                .source_rect(Rect::new(10.0, 20.0, 30.0, 40.0)),
        );
    });

    let super::super::CanvasItem::Image(image) = &scene.items()[0] else {
        panic!("expected image item");
    };
    assert_eq!(image.fit, ContentFit::Cover);
    assert_eq!(image.corner_radius, dp(12.0));
    assert_eq!(image.source_rect, Some(Rect::new(10.0, 20.0, 30.0, 40.0)));
}

#[test]
fn source_rect_is_normalized_and_converted_to_uv() {
    let intrinsic = IntrinsicSize {
        width: 200.0,
        height: 100.0,
    };
    let normalized = normalized_source_rect(Some(Rect::new(-20.0, 10.0, 80.0, 120.0)), intrinsic)
        .expect("source rect should normalize");
    let uv = source_rect_to_uv_rect(normalized, intrinsic).expect("uv rect should resolve");

    assert_eq!(normalized, Rect::new(0.0, 10.0, 60.0, 90.0));
    assert_eq!(uv, Rect::new(0.0, 0.1, 0.3, 0.9));
}
