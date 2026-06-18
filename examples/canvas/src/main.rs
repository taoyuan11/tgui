use std::fmt::Write as _;

use tgui::prelude::*;

const CARD_CANVAS_WIDTH: f32 = 520.0;
const CARD_CANVAS_HEIGHT: f32 = 260.0;
const CARD_PANEL_HEIGHT: f32 = 386.0;
const LAB_CANVAS_HEIGHT: f32 = 296.0;
const RETAINED_CARD_HEIGHT: f32 = 612.0;
const EVENT_CARD_HEIGHT: f32 = 448.0;

fn logo_source() -> MediaSource {
    MediaSource::bytes(include_bytes!("../../../docs/public/images/tgui_logo.png"))
}

fn text_style(ctx: &StyleContext<'_>, size: Sp) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = size;
    style
}

fn muted_text_style(ctx: &StyleContext<'_>, size: Sp) -> TextWidgetStyle {
    let mut style = text_style(ctx, size);
    style.color = Color::hexa(0x475569FF).into();
    style
}

fn hero_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hexa(0xE2E8F0FF).into());
    style.surface.border_radius = Some(dp(24.0).into());
    style
}

fn card_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hexa(0xF8FAFCFF).into());
    style.surface.border_color = Some(Color::hexa(0xCBD5E1FF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(22.0).into());
    style
}

fn canvas_frame_style(ctx: &StyleContext<'_>) -> CanvasStyle {
    let mut style = CanvasStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hexa(0x0F172AFF).into());
    style.surface.border_color = Some(Color::hexa(0x334155FF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(20.0).into());
    style
}

fn info_chip_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hexa(0xE2E8F0FF).into());
    style.surface.border_radius = Some(dp(14.0).into());
    style.color = Color::hexa(0x0F172AFF).into();
    style
}

fn action_row_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hexa(0xEFF6FFFF).into());
    style.surface.border_radius = Some(dp(18.0).into());
    style
}

fn card_scene_background(canvas: &mut CanvasRecorder, color: Color) {
    canvas
        .set_fill(color)
        .fill_round_rect(0.0, 0.0, CARD_CANVAS_WIDTH, CARD_CANVAS_HEIGHT, dp(20.0));
}

fn showcase_title(canvas: &mut CanvasRecorder, x: f32, y: f32, label: &str) {
    canvas
        .set_text_style(CanvasTextStyle {
            color: Color::hexa(0xCBD5E1FF),
            font_size: sp(13.0),
            font_weight: FontWeight::Bold,
            ..Default::default()
        })
        .draw_text(Rect::new(x, y, 150.0, 18.0), label);
}

fn primitives_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0x020617FF));

        showcase_title(canvas, 26.0, 20.0, "Shortcuts");
        canvas
            .set_fill(Color::hexa(0x0EA5E9FF))
            .fill_rect(26.0, 44.0, 96.0, 54.0)
            .set_fill(Color::hexa(0x38BDF8FF))
            .fill_round_rect(136.0, 44.0, 96.0, 54.0, dp(18.0))
            .set_fill(Color::hexa(0x22C55EFF))
            .fill_circle(292.0, 72.0, 28.0)
            .set_fill(Color::hexa(0xF97316FF))
            .fill_ellipse(402.0, 72.0, 44.0, 26.0);

        showcase_title(canvas, 26.0, 116.0, "Stroke Alignment");
        canvas
            .set_stroke(CanvasStroke::new(dp(12.0), Color::hexa(0xF8FAFCFF)))
            .stroke_round_rect(28.0, 142.0, 86.0, 54.0, dp(16.0))
            .set_stroke(
                CanvasStroke::new(dp(12.0), Color::hexa(0x38BDF8FF))
                    .alignment(CanvasStrokeAlignment::Inside),
            )
            .stroke_round_rect(134.0, 142.0, 86.0, 54.0, dp(16.0))
            .set_stroke(
                CanvasStroke::new(dp(12.0), Color::hexa(0xF97316FF))
                    .alignment(CanvasStrokeAlignment::Outside),
            )
            .stroke_round_rect(240.0, 142.0, 86.0, 54.0, dp(16.0));

        showcase_title(canvas, 344.0, 116.0, "Fill Rule");
        canvas
            .set_fill(Color::hexa(0xE0F2FEFF))
            .set_fill_rule(CanvasFillRule::EvenOdd)
            .begin_path()
            .circle(420.0, 170.0, 44.0)
            .circle(420.0, 170.0, 18.0)
            .fill()
            .set_fill_rule(CanvasFillRule::NonZero)
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0xCBD5E1FF),
                font_size: sp(12.0),
                ..Default::default()
            })
            .draw_text(Rect::new(340.0, 214.0, 150.0, 20.0), "Even-odd donut");
    })
}

fn paths_scene() -> CanvasScene {
    let left = PathBuilder::new()
        .move_to(44.0, 178.0)
        .line_to(98.0, 126.0)
        .line_to(152.0, 178.0)
        .line_to(98.0, 220.0)
        .close();
    let right = PathBuilder::new()
        .move_to(108.0, 148.0)
        .line_to(176.0, 148.0)
        .line_to(176.0, 220.0)
        .line_to(108.0, 220.0)
        .close();
    let diff = right.difference(&left).expect("boolean path should succeed");

    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0x111827FF));

        showcase_title(canvas, 24.0, 18.0, "Curves");
        canvas
            .set_stroke(
                CanvasStroke::with_brush(
                    dp(10.0),
                    CanvasLinearGradient::new(
                        Point::new(24.0, 44.0),
                        Point::new(214.0, 112.0),
                        vec![
                            CanvasGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                            CanvasGradientStop::new(1.0, Color::hexa(0x22C55EFF)),
                        ],
                    ),
                )
                .line_cap(CanvasStrokeCap::Round)
                .line_join(CanvasStrokeJoin::Round),
            )
            .begin_path()
            .move_to(34.0, 98.0)
            .quad_to(74.0, 26.0, 118.0, 82.0)
            .cubic_to(146.0, 132.0, 180.0, 20.0, 222.0, 92.0)
            .stroke();

        showcase_title(canvas, 264.0, 18.0, "Arc / ArcTo");
        canvas
            .set_stroke(
                CanvasStroke::new(dp(8.0), Color::hexa(0xF8FAFCFF))
                    .line_cap(CanvasStrokeCap::Round)
                    .dash([dp(18.0), dp(10.0)]),
            )
            .begin_path()
            .move_to(274.0, 106.0)
            .arc_to(324.0, 34.0, 390.0, 98.0, dp(26.0))
            .arc(406.0, 98.0, dp(30.0), -1.2, 3.6)
            .stroke();

        showcase_title(canvas, 24.0, 126.0, "SVG Path");
        canvas
            .set_fill(Color::hexa(0xFDE68AFF))
            .draw_svg_path("M 280 0 L 360 160 L 200 52 L 360 52 L 200 160 Z")
            .expect("svg path should parse");

        showcase_title(canvas, 264.0, 126.0, "Boolean Difference");
        canvas
            .set_fill(CanvasLinearGradient::new(
                Point::new(334.0, 140.0),
                Point::new(468.0, 228.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0xF97316FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0xEF4444FF)),
                ],
            ))
            .draw_path(diff);
    })
}

fn text_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0xF8FAFCFF));
        canvas
            .set_fill(Color::hexa(0xDBEAFEFF))
            .fill_round_rect(24.0, 22.0, 230.0, 96.0, dp(20.0))
            .set_fill(Color::hexa(0xE0F2FEFF))
            .fill_round_rect(266.0, 22.0, 230.0, 96.0, dp(20.0))
            .set_fill(Color::hexa(0xE2E8F0FF))
            .fill_round_rect(24.0, 132.0, 472.0, 104.0, dp(20.0));

        showcase_title(canvas, 36.0, 30.0, "Wrap / Align");
        canvas
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0x0F172AFF),
                font_size: sp(15.0),
                line_height: Some(sp(20.0)),
                ..Default::default()
            })
            .set_paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Word,
                horizontal_align: CanvasTextHorizontalAlign::Center,
                vertical_align: CanvasTextVerticalAlign::Center,
                ..Default::default()
            })
            .draw_text(
                Rect::new(40.0, 48.0, 198.0, 58.0),
                "Word wrap centers the full block without splitting short phrases.",
            );

        showcase_title(canvas, 278.0, 30.0, "Rich Text");
        canvas
            .set_paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Glyph,
                vertical_align: CanvasTextVerticalAlign::Center,
                ..Default::default()
            })
            .draw_rich_text(
                Rect::new(282.0, 48.0, 196.0, 58.0),
                vec![
                    CanvasTextSpan::new("Recorder ")
                        .style(CanvasTextStyle {
                            color: Color::hexa(0x0F172AFF),
                            font_size: sp(16.0),
                            font_weight: FontWeight::Bold,
                            ..Default::default()
                        }),
                    CanvasTextSpan::new("supports ")
                        .style(CanvasTextStyle {
                            color: Color::hexa(0x1D4ED8FF),
                            font_size: sp(16.0),
                            ..Default::default()
                        }),
                    CanvasTextSpan::new("styled spans")
                        .style(CanvasTextStyle {
                            color: Color::hexa(0xF97316FF),
                            font_size: sp(16.0),
                            font_weight: FontWeight::Bold,
                            ..Default::default()
                        }),
                ],
            );

        showcase_title(canvas, 36.0, 140.0, "Ellipsis / Hit Text");
        canvas
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0x334155FF),
                font_size: sp(15.0),
                line_height: Some(sp(21.0)),
                ..Default::default()
            })
            .set_paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Word,
                overflow: CanvasTextOverflow::Ellipsis,
                vertical_align: CanvasTextVerticalAlign::Center,
                ..Default::default()
            })
            .draw_text(
                Rect::new(40.0, 164.0, 438.0, 52.0),
                "Hover the text item in the interaction lab to inspect CanvasTextHit utf8 ranges, line metrics, and cluster bounds.",
            );
    })
}

fn transforms_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0x111827FF));
        showcase_title(canvas, 26.0, 20.0, "Translate / Rotate / Scale / Matrix");

        canvas
            .set_fill(Color::hexa(0x0F172AFF))
            .fill_round_rect(28.0, 52.0, 140.0, 170.0, dp(24.0))
            .set_stroke(CanvasStroke::new(dp(2.0), Color::hexa(0x334155FF)))
            .stroke_round_rect(28.0, 52.0, 140.0, 170.0, dp(24.0));

        canvas
            .save()
            .translate(96.0, 134.0)
            .rotate(-0.22)
            .scale(0.82, 0.82)
            .translate(-52.0, -52.0)
            .draw_image(Rect::new(0.0, 0.0, 104.0, 104.0), logo_source())
            .restore();

        canvas
            .save()
            .transform(CanvasTransform2D::from_matrix([1.0, 0.12, -0.18, 1.0, 278.0, 48.0]))
            .set_fill(Color::hexa(0x0EA5E9FF))
            .fill_round_rect(0.0, 0.0, 166.0, 92.0, dp(18.0))
            .set_text_style(CanvasTextStyle {
                color: Color::WHITE,
                font_size: sp(18.0),
                font_weight: FontWeight::Bold,
                ..Default::default()
            })
            .draw_text(Rect::new(18.0, 18.0, 124.0, 30.0), "Matrix transform")
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0xE0F2FEFF),
                font_size: sp(14.0),
                line_height: Some(sp(18.0)),
                ..Default::default()
            })
            .draw_text(Rect::new(18.0, 54.0, 124.0, 32.0), "Local state stays isolated by save / restore.")
            .restore();
    })
}

fn clip_mask_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0xF8FAFCFF));

        showcase_title(canvas, 24.0, 18.0, "Clip");
        canvas
            .save()
            .rounded_rect(26.0, 42.0, 210.0, 184.0, dp(28.0))
            .clip()
            .set_fill(CanvasRadialGradient::new(
                Point::new(120.0, 128.0),
                dp(144.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0xFDE68AFF)),
                    CanvasGradientStop::new(0.5, Color::hexa(0xFB7185FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x7C3AEDFF)),
                ],
            ))
            .fill_ellipse(132.0, 136.0, 144.0, 104.0)
            .set_fill(Color::hexa(0xFFFFFFDD))
            .fill_circle(90.0, 98.0, 30.0)
            .restore();

        showcase_title(canvas, 276.0, 18.0, "Mask");
        canvas
            .save()
            .circle(384.0, 130.0, 86.0)
            .mask()
            .draw_image(
                Rect::new(294.0, 40.0, 180.0, 180.0),
                logo_source(),
            )
            .set_fill(CanvasLinearGradient::new(
                Point::new(276.0, 42.0),
                Point::new(472.0, 218.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0x0EA5E9CC)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8CC)),
                ],
            ))
            .fill_rect(276.0, 42.0, 208.0, 184.0)
            .restore();
    })
}

fn composite_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0x020617FF));
        showcase_title(canvas, 26.0, 18.0, "Blend / Alpha");

        canvas
            .set_fill(Color::hexa(0x14B8A6FF))
            .fill_circle(124.0, 110.0, 64.0)
            .set_opacity(0.88)
            .set_fill(Color::hexa(0xF43F5EFF))
            .fill_circle(180.0, 110.0, 64.0)
            .set_blend_mode(CanvasBlendMode::Screen)
            .set_fill(Color::hexa(0x38BDF8FF))
            .fill_circle(152.0, 78.0, 56.0)
            .set_fill(Color::hexa(0xFDE047FF))
            .fill_circle(206.0, 142.0, 56.0)
            .set_blend_mode(CanvasBlendMode::Normal)
            .set_opacity(1.0);

        showcase_title(canvas, 282.0, 18.0, "Effects / Isolation");
        canvas
            .save()
            .translate(286.0, 44.0)
            .set_isolation(true)
            .set_effects(vec![
                CanvasEffect::Blur(dp(6.0)),
                CanvasEffect::ColorFilter(CanvasColorFilter::tint(
                    Color::hexa(0x38BDF8FF),
                    0.28,
                )),
            ])
            .set_fill(Color::hexa(0x0F172AFF))
            .fill_round_rect(0.0, 0.0, 196.0, 86.0, dp(22.0))
            .clear_effects()
            .set_effects(vec![CanvasEffect::InnerShadow(CanvasInnerShadow::new(
                Color::hexa(0x020617CC),
                Point::new(0.0, 6.0),
                dp(16.0),
            ))])
            .set_fill(Color::hexa(0xE0F2FEFF))
            .fill_round_rect(0.0, 108.0, 196.0, 92.0, dp(22.0))
            .clear_effects()
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0x0F172AFF),
                font_size: sp(16.0),
                font_weight: FontWeight::Bold,
                ..Default::default()
            })
            .draw_text(Rect::new(18.0, 138.0, 158.0, 24.0), "Blur, tint, inner shadow")
            .restore();
    })
}

fn images_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0xF8FAFCFF));
        showcase_title(canvas, 24.0, 18.0, "Image Options");

        canvas
            .draw_image(Rect::new(28.0, 46.0, 136.0, 166.0), logo_source())
            .draw_image_with_options(
                Rect::new(192.0, 46.0, 136.0, 166.0),
                logo_source(),
                CanvasImageOptions::new()
                    .fit(ContentFit::Cover)
                    .corner_radius(dp(26.0)),
            )
            .draw_image_with_options(
                Rect::new(356.0, 46.0, 136.0, 166.0),
                logo_source(),
                CanvasImageOptions::new()
                    .fit(ContentFit::Fill)
                    .corner_radius(dp(26.0))
                    .source_rect(Rect::new(42.0, 32.0, 164.0, 164.0)),
            )
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0x334155FF),
                font_size: sp(13.0),
                ..Default::default()
            })
            .draw_text(Rect::new(48.0, 224.0, 90.0, 18.0), "Contain")
            .draw_text(Rect::new(220.0, 224.0, 80.0, 18.0), "Cover + radius")
            .draw_text(Rect::new(374.0, 224.0, 110.0, 18.0), "Fill + source rect");
    })
}

fn recorder_state_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0x111827FF));
        showcase_title(canvas, 24.0, 18.0, "Recorder State");

        canvas
            .next_item_name("gradient-badge")
            .set_fill(CanvasLinearGradient::new(
                Point::new(30.0, 42.0),
                Point::new(196.0, 130.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0x0EA5E9FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                ],
            ))
            .fill_round_rect(28.0, 42.0, 176.0, 84.0, dp(26.0))
            .set_shadow(CanvasShadow::new(
                Color::hexa(0x38BDF866),
                Point::new(0.0, 8.0),
                dp(18.0),
            ))
            .set_fill(Color::hexa(0xF8FAFCFF))
            .fill_circle(250.0, 86.0, 40.0)
            .clear_shadow()
            .set_stroke(
                CanvasStroke::new(dp(12.0), Color::hexa(0xF97316FF))
                    .line_cap(CanvasStrokeCap::Round)
                    .line_join(CanvasStrokeJoin::Round),
            )
            .draw_line(312.0, 86.0, 472.0, 86.0)
            .clear_stroke()
            .save()
            .translate(48.0, 158.0)
            .rotate(-0.14)
            .set_effects(vec![CanvasEffect::ColorFilter(CanvasColorFilter::multiply(
                Color::hexa(0x93C5FDFF),
            ))])
            .draw_image_with_options(
                Rect::new(0.0, 0.0, 114.0, 74.0),
                logo_source(),
                CanvasImageOptions::new().fit(ContentFit::Cover).corner_radius(dp(18.0)),
            )
            .clear_effects()
            .restore()
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0xCBD5E1FF),
                font_size: sp(14.0),
                line_height: Some(sp(18.0)),
                ..Default::default()
            })
            .draw_text(
                Rect::new(192.0, 156.0, 280.0, 64.0),
                "next_item_name, clear_shadow, clear_stroke, clear_effects, and save/restore all reuse the same public recorder state machine.",
            );
    })
}

fn interaction_scene() -> CanvasScene {
    CanvasRecorder::build(|canvas| {
        card_scene_background(canvas, Color::hexa(0xF8FAFCFF));
        canvas
            .next_item_id(801_u64)
            .next_item_name("click-target")
            .set_fill(Color::hexa(0x38BDF8FF))
            .fill_round_rect(42.0, 42.0, 130.0, 78.0, dp(18.0))
            .set_text_style(CanvasTextStyle {
                color: Color::WHITE,
                font_size: sp(18.0),
                font_weight: FontWeight::Bold,
                ..Default::default()
            })
            .draw_text(Rect::new(74.0, 68.0, 82.0, 28.0), "Click")
            .next_item_id(803_u64)
            .next_item_name("drag-target")
            .set_fill(Color::hexa(0x22C55EFF))
            .fill_round_rect(202.0, 40.0, 116.0, 82.0, dp(18.0))
            .draw_text(Rect::new(228.0, 68.0, 74.0, 28.0), "Drag")
            .next_item_id(805_u64)
            .next_item_name("hover-target")
            .set_fill(Color::hexa(0xF97316FF))
            .fill_circle(414.0, 82.0, 44.0)
            .draw_text(Rect::new(384.0, 72.0, 68.0, 22.0), "Hover")
            .next_item_id(807_u64)
            .next_item_name("text-target")
            .set_text_style(CanvasTextStyle {
                color: Color::hexa(0x334155FF),
                font_size: sp(15.0),
                line_height: Some(sp(21.0)),
                ..Default::default()
            })
            .set_paragraph_style(CanvasParagraphStyle {
                wrap: CanvasTextWrap::Word,
                ..Default::default()
            })
            .draw_text(
                Rect::new(40.0, 152.0, 438.0, 46.0),
                "Move across this text to inspect CanvasTextHit clusters, then use wheel / down / up / drag on the controls above.",
            );
    })
}

fn retained_overlay_item() -> CanvasItem {
    CanvasPath::new(
        590_u64,
        PathBuilder::new().rounded_rect(24.0, 18.0, 472.0, 228.0, dp(24.0)),
    )
    .name_item("selection-outline")
    .stroke(
        CanvasStroke::new(dp(6.0), Color::hexa(0x38BDF8FF))
            .alignment(CanvasStrokeAlignment::Outside),
    )
    .hit_test(false)
    .into()
}

fn retained_cluster_note() -> CanvasItem {
    CanvasText::new(
        526_u64,
        Rect::new(308.0, 168.0, 124.0, 28.0),
        "inline child",
    )
    .name_item("inline-note")
    .text_style(CanvasTextStyle {
        color: Color::hexa(0xE0F2FEFF),
        font_size: sp(13.0),
        font_weight: FontWeight::Bold,
        ..Default::default()
    })
    .into()
}

fn build_retained_scene() -> CanvasScene {
    let clipped_cluster = CanvasGroup::new(
        520_u64,
        CanvasGroupMode::Clip,
        CanvasGroupShape::path(PathBuilder::new().rounded_rect(278.0, 58.0, 176.0, 148.0, dp(30.0))),
        vec![
            CanvasPath::new(
                521_u64,
                PathBuilder::new().circle(360.0, 122.0, 88.0),
            )
            .name_item("cluster-fill")
            .fill(CanvasRadialGradient::new(
                Point::new(340.0, 102.0),
                dp(108.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0x67E8F9FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x2563EBFF)),
                ],
            ))
            .effects(vec![CanvasEffect::Blur(dp(2.0))])
            .into(),
            CanvasText::new(
                522_u64,
                Rect::new(302.0, 82.0, 128.0, 52.0),
                "Named group",
            )
            .name_item("cluster-title")
            .text_style(CanvasTextStyle {
                color: Color::WHITE,
                font_size: sp(22.0),
                font_weight: FontWeight::Bold,
                ..Default::default()
            })
            .paragraph_style(CanvasParagraphStyle {
                horizontal_align: CanvasTextHorizontalAlign::Center,
                ..Default::default()
            })
            .into(),
            CanvasImage::new(523_u64, Rect::new(320.0, 132.0, 92.0, 56.0), logo_source())
                .name_item("cluster-image")
                .options(CanvasImageOptions::new().fit(ContentFit::Cover).corner_radius(dp(16.0)))
                .opacity(0.92)
                .into(),
        ],
    )
    .name_item("cluster");

    let masked_logo = CanvasGroup::new(
        540_u64,
        CanvasGroupMode::Mask,
        CanvasGroupShape::path(PathBuilder::new().circle(132.0, 134.0, 68.0)),
        vec![
            CanvasImage::new(541_u64, Rect::new(60.0, 62.0, 144.0, 144.0), logo_source())
                .name_item("logo-mask-image")
                .options(CanvasImageOptions::new().fit(ContentFit::Cover))
                .into(),
            CanvasPath::new(
                542_u64,
                PathBuilder::new().rect(60.0, 62.0, 144.0, 144.0),
            )
            .name_item("logo-mask-tint")
            .fill(CanvasBrush::Solid(Color::hexa(0x0EA5E988)))
            .blend_mode(CanvasBlendMode::Screen)
            .into(),
        ],
    )
    .name_item("logo-mask");

    CanvasScene::from_items(vec![
        CanvasPath::new(
            510_u64,
            PathBuilder::new().rounded_rect(18.0, 18.0, 484.0, 228.0, dp(28.0)),
        )
        .name_item("board")
        .fill(Color::hexa(0x020617FF))
        .into(),
        masked_logo.into(),
        CanvasText::new(
            512_u64,
            Rect::new(220.0, 34.0, 242.0, 28.0),
            "Retained Scene Lab",
        )
        .name_item("title")
        .text_style(CanvasTextStyle {
            color: Color::hexa(0xE2E8F0FF),
            font_size: sp(22.0),
            font_weight: FontWeight::Bold,
            ..Default::default()
        })
        .into(),
        CanvasPath::new(
            514_u64,
            PathBuilder::new().rounded_rect(222.0, 76.0, 214.0, 124.0, dp(24.0)),
        )
        .name_item("note-card")
        .fill(Color::hexa(0x0F172AFF))
        .effects(vec![CanvasEffect::InnerShadow(CanvasInnerShadow::new(
            Color::hexa(0x020617CC),
            Point::new(0.0, 10.0),
            dp(18.0),
        ))])
        .into(),
        CanvasText::new(
            516_u64,
            Rect::new(244.0, 96.0, 170.0, 58.0),
            "Scene::find, visit, query_point, export_json and remove all point back to the same retained tree.",
        )
        .name_item("summary")
        .text_style(CanvasTextStyle {
            color: Color::hexa(0xCBD5E1FF),
            font_size: sp(14.0),
            line_height: Some(sp(20.0)),
            ..Default::default()
        })
        .paragraph_style(CanvasParagraphStyle {
            wrap: CanvasTextWrap::Word,
            overflow: CanvasTextOverflow::Ellipsis,
            ..Default::default()
        })
        .into(),
        clipped_cluster.into(),
        CanvasPath::new(
            560_u64,
            PathBuilder::new().rounded_rect(46.0, 46.0, 174.0, 176.0, dp(30.0)),
        )
        .name_item("guide")
        .stroke(CanvasStroke::new(dp(3.0), Color::hexa(0x93C5FD88)))
        .hit_test(false)
        .into(),
        retained_overlay_item(),
    ])
}

fn summarize_text_hit(hit: Option<CanvasTextHit>) -> String {
    match hit {
        Some(hit) => format!(
            " text={}..{} line={} cluster=({:.0},{:.0},{:.0},{:.0})",
            hit.utf8_start,
            hit.utf8_end,
            hit.line_index,
            hit.cluster_bounds.x,
            hit.cluster_bounds.y,
            hit.cluster_bounds.width,
            hit.cluster_bounds.height
        ),
        None => String::new(),
    }
}

fn summarize_scene_hit(hit: Option<CanvasSceneHit>) -> String {
    match hit {
        Some(hit) => {
            let name = hit.name.unwrap_or_else(|| "unnamed".to_string());
            format!(
                "id={} kind={:?} name={} depth={} local=({:.0},{:.0}){}",
                hit.item_id.get(),
                hit.kind,
                name,
                hit.depth,
                hit.local_position.x,
                hit.local_position.y,
                summarize_text_hit(hit.text_hit)
            )
        }
        None => "none".to_string(),
    }
}

fn format_bounds(bounds: Option<Rect>) -> String {
    match bounds {
        Some(bounds) => format!(
            "({:.0},{:.0},{:.0},{:.0})",
            bounds.x, bounds.y, bounds.width, bounds.height
        ),
        None => "none".to_string(),
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= max_chars {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

fn retained_scene_report(scene: &CanvasScene) -> String {
    let stats = scene.debug_info().stats;
    let contains_overlay = scene.contains_id(590_u64.into());
    let contains_cluster = scene.contains_name("cluster");
    let title = scene
        .find(512_u64.into())
        .and_then(|item| item.name())
        .unwrap_or("missing");
    let named = scene
        .find_named("cluster-title")
        .map(|item| format!("{:?}#{}", item.kind(), item.id().get()))
        .unwrap_or_else(|| "missing".to_string());
    let options = CanvasSceneQueryOptions::new().scale_factor(1.0).font_scale(1.0);
    let top_hit = scene.query_point_with(&options, Point::new(332.0, 116.0));
    let all_hits = scene.query_point_all(Point::new(332.0, 116.0));
    let mut visit_lines = Vec::new();
    scene.visit(|entry| {
        let name = entry.item.name().unwrap_or("unnamed");
        if visit_lines.len() < 5 {
            visit_lines.push(format!(
                "{}:{}@{:?}",
                entry.depth,
                name,
                entry.index_path
            ));
        }
    });

    let stable = scene.export_json();
    let debug_text = scene.export_debug_text();
    let debug_json = scene.export_debug_json();
    let mut report = String::new();
    let _ = writeln!(
        report,
        "Scene API: len={} empty={} bounds={} root={} total={} named={} depth={}",
        scene.len(),
        scene.is_empty(),
        format_bounds(scene.bounds()),
        stats.root_items,
        stats.total_items,
        stats.named_items,
        stats.max_depth
    );
    let _ = writeln!(
        report,
        "contains_id(590)={} contains_name(cluster)={} find(512)={} find_named(cluster-title)={}",
        contains_overlay,
        contains_cluster,
        title,
        named
    );
    let _ = writeln!(
        report,
        "query_point_with(332,116)={} query_point_all(332,116)={}",
        summarize_scene_hit(top_hit),
        all_hits.len()
    );
    let _ = writeln!(report, "visit={}", visit_lines.join(" | "));
    let _ = writeln!(
        report,
        "export_json={} bytes export_debug_text={} lines export_debug_json={} bytes",
        stable.len(),
        debug_text.lines().count(),
        debug_json.len()
    );
    let _ = writeln!(
        report,
        "stable preview={}",
        truncate(stable.as_str(), 108)
    );
    report
}

struct CanvasVm {
    selected_demo: State<usize>,
    hovered: State<String>,
    activity: State<String>,
    retained_probe: State<String>,
    retained_scene: State<CanvasScene>,
}

impl CanvasVm {
    fn sample_canvas(scene: CanvasScene) -> Canvas<Self> {
        Canvas::new(scene)
            .size(dp(CARD_CANVAS_WIDTH), dp(CARD_CANVAS_HEIGHT))
            .style_full(canvas_frame_style)
            .on_item_mouse_move(ValueCommand::new(Self::on_hover))
            .on_item_click(ValueCommand::new(Self::on_click))
            .on_item_drag(ValueCommand::new(Self::on_drag))
    }

    fn demo_scene(index: usize) -> CanvasScene {
        match index {
            0 => primitives_scene(),
            1 => paths_scene(),
            2 => text_scene(),
            3 => transforms_scene(),
            4 => clip_mask_scene(),
            5 => composite_scene(),
            6 => images_scene(),
            7 => recorder_state_scene(),
            8 => interaction_scene(),
            9 => build_retained_scene(),
            _ => primitives_scene(),
        }
    }

    fn demo_meta(index: usize) -> (&'static str, &'static str) {
        match index {
            0 => ("Primitives", "矩形、圆角矩形、圆、椭圆、描边对齐和 even-odd 填充规则。"),
            1 => ("Paths", "quad/cubic/arc/arc_to、SVG path 和布尔路径运算。"),
            2 => ("Text", "普通文本、富文本 span、对齐、换行、ellipsis 和 text hit 语义。"),
            3 => ("Transforms", "translate、scale、rotate、matrix transform 与 save/restore。"),
            4 => ("Clip / Mask", "录制式 group 会把后续命令包装成 clip 或 mask 组。"),
            5 => ("Composite", "opacity、blend mode、blur、color filter、inner shadow 和 isolation。"),
            6 => ("Images", "draw_image 与 draw_image_with_options 覆盖 contain / cover / fill、圆角与 source rect。"),
            7 => ("Recorder State", "next_item_name、clear_* 和统一 recorder 状态机。"),
            8 => ("Interaction Scene", "场景本身包含 hover / click / drag / text hit 的交互热点。"),
            9 => ("Retained Scene", "展示 CanvasScene 保留式结构生成出的命名分组与图元。"),
            _ => ("Primitives", "矩形、圆角矩形、圆、椭圆、描边对齐和 even-odd 填充规则。"),
        }
    }

    fn showcase_card(
        title: &'static str,
        description: &'static str,
        height: f32,
        body: Element<Self>,
    ) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .grow(1.0)
            .height(dp(height))
            .padding(Insets::all(dp(18.0)))
            .gap(dp(12.0))
            .style_full(card_style)
            .child(Text::new(title).style_full(|ctx| text_style(ctx, sp(22.0))))
            .child(Text::new(description).style_full(|ctx| muted_text_style(ctx, sp(14.0))))
            .child(body)
            .into()
    }

    fn static_scene_card(title: &'static str, description: &'static str, scene: CanvasScene) -> Element<Self> {
        Self::showcase_card(
            title,
            description,
            CARD_PANEL_HEIGHT,
            Self::sample_canvas(scene).into(),
        )
    }

    fn retained_lab_card(
        retained_scene: Signal<CanvasScene>,
        retained_probe: Signal<String>,
    ) -> Flex<Self> {
        Flex::new(Axis::Vertical)
            .height(dp(RETAINED_CARD_HEIGHT))
            .padding(Insets::all(dp(18.0)))
            .gap(dp(12.0))
            .style_full(card_style)
            .child(Text::new("Retained Scene Lab").style_full(|ctx| text_style(ctx, sp(22.0))))
            .child(
                Text::new("直接持有 CanvasScene，并用 names / visit / query / export / remove / insert / push / clear 驱动一个动态场景。")
                    .style_full(|ctx| muted_text_style(ctx, sp(14.0))),
            )
            .child(Flex::new(Axis::Vertical)
                .gap(dp(12.0))
                .child(
                    Flex::new(Axis::Horizontal)
                        .padding(Insets::all(dp(12.0)))
                        .gap(dp(10.0))
                        .style_full(action_row_style)
                        .child(
                            Button::new("重建 Scene")
                                .secondary()
                                .on_click(Command::new(Self::reset_retained_scene)),
                        )
                        .child(
                            Button::new("切换选框")
                                .secondary()
                                .on_click(Command::new(Self::toggle_retained_overlay)),
                        )
                        .child(
                            Button::new("插入批注")
                                .secondary()
                                .on_click(Command::new(Self::toggle_retained_note)),
                        )
                        .child(
                            Button::new("旋转层级")
                                .ghost()
                                .on_click(Command::new(Self::rotate_retained_order)),
                        )
                        .child(
                            Button::new("清空 Scene")
                                .danger()
                                .on_click(Command::new(Self::clear_retained_scene)),
                        ),
                )
                .child(
                    Canvas::new(retained_scene.clone())
                        .size(dp(CARD_CANVAS_WIDTH), dp(LAB_CANVAS_HEIGHT))
                        .style_full(canvas_frame_style)
                        .on_mouse_move(ValueCommand::new(Self::probe_retained_scene))
                        .on_mouse_leave(Command::new(Self::reset_probe))
                        .on_item_mouse_move(ValueCommand::new(Self::on_hover))
                        .on_item_click(ValueCommand::new(Self::on_click)),
                )
                .child(
                    Text::new(retained_probe)
                        .padding(Insets::all(dp(12.0)))
                        .style_full(info_chip_style),
                )
                .child(
                    Text::new(retained_scene.map(|scene| retained_scene_report(&scene)))
                        .padding(Insets::all(dp(12.0)))
                        .style_full(|ctx| muted_text_style(ctx, sp(13.0))),
                )
            )
    }

    fn event_lab_card() -> Flex<Self> {
        Flex::new(Axis::Vertical)
            .height(dp(EVENT_CARD_HEIGHT))
            .padding(Insets::all(dp(18.0)))
            .gap(dp(12.0))
            .style_full(card_style)
            .child(Text::new("Interaction Lab").style_full(|ctx| text_style(ctx, sp(22.0))))
            .child(
                Text::new("这里把 item 级 hover、enter、leave、down、up、click、double click、wheel、drag start、drag、drag end 和 text hit 都接起来了。")
                    .style_full(|ctx| muted_text_style(ctx, sp(14.0))),
            )
            .child(
                Canvas::new(interaction_scene())
                    .size(dp(CARD_CANVAS_WIDTH), dp(CARD_CANVAS_HEIGHT))
                    .style_full(canvas_frame_style)
                    .on_item_mouse_enter(ValueCommand::new(Self::on_mouse_enter))
                    .on_item_mouse_leave(ValueCommand::new(Self::on_mouse_leave))
                    .on_item_mouse_down(ValueCommand::new(Self::on_mouse_down))
                    .on_item_mouse_up(ValueCommand::new(Self::on_mouse_up))
                    .on_item_mouse_move(ValueCommand::new(Self::on_hover))
                    .on_item_click(ValueCommand::new(Self::on_click))
                    .on_item_double_click(ValueCommand::new(Self::on_double_click))
                    .on_item_wheel(ValueCommand::new(Self::on_wheel))
                    .on_item_drag_start(ValueCommand::new(Self::on_drag_start))
                    .on_item_drag(ValueCommand::new(Self::on_drag))
                    .on_item_drag_end(ValueCommand::new(Self::on_drag_end)),
            )
    }

    fn set_mouse_message(&self, prefix: &str, event: CanvasMouseEvent) {
        self.activity.set(format!(
            "{} item={} scene=({:.0},{:.0}) button={:?}{}",
            prefix,
            event.item_id.get(),
            event.scene_position.x,
            event.scene_position.y,
            event.button,
            summarize_text_hit(event.text_hit)
        ));
    }

    fn on_hover(&mut self, event: CanvasMouseEvent) {
        self.hovered.set(format!(
            "hover item={} canvas=({:.0},{:.0}) scene=({:.0},{:.0}) local=({:.0},{:.0}){}",
            event.item_id.get(),
            event.canvas_position.x,
            event.canvas_position.y,
            event.scene_position.x,
            event.scene_position.y,
            event.local_position.x,
            event.local_position.y,
            summarize_text_hit(event.text_hit)
        ));
    }

    fn on_click(&mut self, event: CanvasMouseEvent) {
        self.set_mouse_message("click", event);
    }

    fn on_double_click(&mut self, event: CanvasMouseEvent) {
        self.set_mouse_message("double-click", event);
    }

    fn on_mouse_enter(&mut self, event: CanvasMouseEvent) {
        self.set_mouse_message("enter", event);
    }

    fn on_mouse_leave(&mut self, event: CanvasMouseEvent) {
        self.set_mouse_message("leave", event);
    }

    fn on_mouse_down(&mut self, event: CanvasMouseEvent) {
        self.set_mouse_message("down", event);
    }

    fn on_mouse_up(&mut self, event: CanvasMouseEvent) {
        self.set_mouse_message("up", event);
    }

    fn on_wheel(&mut self, event: CanvasWheelEvent) {
        self.activity.set(format!(
            "wheel item={} delta=({:.0},{:.0}) scene=({:.0},{:.0}){}",
            event.item_id.get(),
            event.delta.x,
            event.delta.y,
            event.scene_position.x,
            event.scene_position.y,
            summarize_text_hit(event.text_hit)
        ));
    }

    fn on_drag_start(&mut self, event: CanvasDragEvent) {
        self.activity.set(format!(
            "drag-start item={} from=({:.0},{:.0}) button={:?}",
            event.item_id.get(),
            event.start_scene_position.x,
            event.start_scene_position.y,
            event.button
        ));
    }

    fn on_drag(&mut self, event: CanvasDragEvent) {
        self.activity.set(format!(
            "drag item={} delta=({:.0},{:.0}) scene=({:.0},{:.0}){}",
            event.item_id.get(),
            event.delta.x,
            event.delta.y,
            event.scene_position.x,
            event.scene_position.y,
            summarize_text_hit(event.text_hit)
        ));
    }

    fn on_drag_end(&mut self, event: CanvasDragEvent) {
        self.activity.set(format!(
            "drag-end item={} delta=({:.0},{:.0}) scene=({:.0},{:.0})",
            event.item_id.get(),
            event.delta.x,
            event.delta.y,
            event.scene_position.x,
            event.scene_position.y
        ));
    }

    fn reset_retained_scene(&mut self) {
        self.retained_scene.set(build_retained_scene());
        self.retained_probe
            .set("Retained scene rebuilt from CanvasScene::from_items(...).".to_string());
        self.activity
            .set("retained scene reset from a fresh retained tree".to_string());
    }

    fn toggle_retained_overlay(&mut self) {
        self.retained_scene.mutate(|scene| {
            if scene.contains_id(590_u64.into()) {
                scene.remove(590_u64.into());
            } else {
                scene.insert(scene.len(), retained_overlay_item());
            }
        });
        self.activity.set(
            "retained scene toggled selection-outline via contains_id + insert/remove".to_string(),
        );
    }

    fn toggle_retained_note(&mut self) {
        self.retained_scene.mutate(|scene| {
            if scene.is_empty() {
                *scene = build_retained_scene();
                return;
            }
            let Some(group) = scene.find_mut(520_u64.into()) else {
                return;
            };
            let Some(children) = group.children_mut() else {
                return;
            };
            if let Some(index) = children
                .iter()
                .position(|item| item.name() == Some("inline-note"))
            {
                children.remove(index);
            } else {
                children.push(retained_cluster_note());
            }
        });
        self.activity.set(
            "retained scene toggled an inline child through find_mut + children_mut".to_string(),
        );
    }

    fn rotate_retained_order(&mut self) {
        self.retained_scene.mutate(|scene| {
            if !scene.is_empty() {
                scene.items_mut().rotate_right(1);
            }
        });
        self.activity
            .set("retained scene rotated root order via items_mut()".to_string());
    }

    fn clear_retained_scene(&mut self) {
        self.retained_scene.mutate(CanvasScene::clear);
        self.retained_probe
            .set("Scene cleared. Rebuild it or use push/insert flows again.".to_string());
        self.activity
            .set("retained scene cleared with CanvasScene::clear()".to_string());
    }

    fn reset_probe(&mut self) {
        self.retained_probe
            .set("Move over the retained canvas to run query_point/query_point_all live.".to_string());
    }

    fn probe_retained_scene(&mut self, point: Point) {
        let message = self.retained_scene.read(|scene| {
            let options = CanvasSceneQueryOptions::new().scale_factor(1.0).font_scale(1.0);
            let top = scene.query_point_with(&options, point);
            let all = scene.query_point_all(point);
            format!(
                "probe ({:.0},{:.0}) => top={} all_hits={}",
                point.x,
                point.y,
                summarize_scene_hit(top),
                all.len()
            )
        });
        self.retained_probe.set(message);
    }
}

impl ViewModel for CanvasVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            selected_demo: ctx.state(0_usize),
            hovered: ctx.state("Move across any canvas to inspect item payloads.".to_string()),
            activity: ctx.state("Use the interaction lab to exercise every item-level event.".to_string()),
            retained_probe: ctx.state(
                "Move over the retained canvas to run query_point/query_point_all live."
                    .to_string(),
            ),
            retained_scene: ctx.state(build_retained_scene()),
        }
    }

    fn view(&self) -> Element<Self> {
        let selected_title = self.selected_demo.signal().map(|selected_demo| {
            let (title, _) = Self::demo_meta(selected_demo);
            title
        });
        let selected_description = self.selected_demo.signal().map(|selected_demo| {
            let (_, description) = Self::demo_meta(selected_demo);
            description
        });
        let selected_scene = self.selected_demo.signal().map({
            let retained_scene = self.retained_scene.signal();
            move |selected_demo| {
                if selected_demo == 9 {
                    retained_scene.get()
                } else {
                    Self::demo_scene(selected_demo)
                }
            }
        });

        Flex::new(Axis::Vertical)
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .gap(dp(16.0))
            .overflow_x(Overflow::Hidden)
            .overflow_y(Overflow::Scroll)
            .child(
                Flex::new(Axis::Vertical)
                    .padding(Insets::all(dp(20.0)))
                    .gap(dp(10.0))
                    .style_full(hero_style)
                    .child(Text::new("Canvas Capability Atlas").style_full(|ctx| text_style(ctx, sp(30.0))))
                    .child(
                        Text::new(
                            "这个示例把公开 Canvas API 尽量都串到一个地方：path、快捷图元、文字、富文本、图片、clip、mask、blend、effect、transform、retained scene、主动 query、命名、导出和完整 item 事件。",
                        )
                        .style_full(|ctx| muted_text_style(ctx, sp(15.0))),
                    ),
            )
            .child(
                Flex::new(Axis::Vertical)
                    .gap(dp(8.0))
                    .child(
                        Text::new(self.hovered.signal())
                            .padding(Insets::all(dp(12.0)))
                            .style_full(info_chip_style),
                    )
                    .child(
                        Text::new(self.activity.signal())
                            .padding(Insets::all(dp(12.0)))
                            .style_full(info_chip_style),
                    ),
            )
            .child(
                Flex::new(Axis::Horizontal)
                    .wrap(Wrap::Wrap)
                    .padding(Insets::all(dp(12.0)))
                    .gap(dp(10.0))
                    .style_full(action_row_style)
                    .child(el![
                        Button::new("Primitives")
                            .secondary()
                            .on_click(Command::new(|vm: &mut Self| vm.selected_demo.set(0))),
                        Button::new("Paths")
                            .secondary()
                            .on_click(Command::new(|vm: &mut Self| vm.selected_demo.set(1))),
                        Button::new("Text")
                            .secondary()
                            .on_click(Command::new(|vm: &mut Self| vm.selected_demo.set(2))),
                        Button::new("Transforms")
                            .secondary()
                            .on_click(Command::new(|vm: &mut Self| vm.selected_demo.set(3))),
                        Button::new("Clip / Mask")
                            .secondary()
                            .on_click(Command::new(|vm: &mut Self| vm.selected_demo.set(4))),
                        Button::new("Composite")
                            .secondary()
                            .on_click(Command::new(|vm: &mut Self| vm.selected_demo.set(5))),
                        Button::new("Images")
                            .secondary()
                            .on_click(Command::new(|vm: &mut Self| vm.selected_demo.set(6))),
                        Button::new("Recorder")
                            .secondary()
                            .on_click(Command::new(|vm: &mut Self| vm.selected_demo.set(7))),
                        Button::new("Interaction")
                            .secondary()
                            .on_click(Command::new(|vm: &mut Self| vm.selected_demo.set(8))),
                        Button::new("Retained")
                            .secondary()
                            .on_click(Command::new(|vm: &mut Self| vm.selected_demo.set(9))),
                    ]),
            )
            .child(
                Flex::new(Axis::Vertical)
                    .height(dp(CARD_PANEL_HEIGHT))
                    .padding(Insets::all(dp(18.0)))
                    .gap(dp(12.0))
                    .style_full(card_style)
                    .child(Text::new(selected_title).style_full(|ctx| text_style(ctx, sp(22.0))))
                    .child(
                        Text::new(selected_description)
                            .style_full(|ctx| muted_text_style(ctx, sp(14.0))),
                    )
                    .child(
                        Canvas::new(selected_scene)
                            .size(dp(CARD_CANVAS_WIDTH), dp(CARD_CANVAS_HEIGHT))
                            .style_full(canvas_frame_style)
                            .on_item_mouse_enter(ValueCommand::new(Self::on_mouse_enter))
                            .on_item_mouse_leave(ValueCommand::new(Self::on_mouse_leave))
                            .on_item_mouse_down(ValueCommand::new(Self::on_mouse_down))
                            .on_item_mouse_up(ValueCommand::new(Self::on_mouse_up))
                            .on_item_mouse_move(ValueCommand::new(Self::on_hover))
                            .on_item_click(ValueCommand::new(Self::on_click))
                            .on_item_double_click(ValueCommand::new(Self::on_double_click))
                            .on_item_wheel(ValueCommand::new(Self::on_wheel))
                            .on_item_drag_start(ValueCommand::new(Self::on_drag_start))
                            .on_item_drag(ValueCommand::new(Self::on_drag))
                            .on_item_drag_end(ValueCommand::new(Self::on_drag_end)),
                    ),
            )
            .into()
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .msaa(MsaaMode::X4)
        .theme_mode(ThemeMode::Light)
        .title("tgui Canvas Capability Atlas")
        .window_size(dp(1540.0), dp(1280.0))
        .with_view_model(CanvasVm::new)
        .root_view(CanvasVm::view)
        .run()
}
