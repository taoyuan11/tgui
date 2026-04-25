use crate::ShowcaseVm;
use tgui::{
    dp, pct, sp, Canvas, CanvasGradientStop, CanvasItem, CanvasLinearGradient, CanvasPath,
    CanvasRadialGradient, CanvasShadow, CanvasStroke, Color, Axis, Flex, Insets, Overflow,
    PathBuilder, Point, ScrollbarStyle, Stack, Text, ValueCommand,
};

pub(crate) fn view(vm: &ShowcaseVm) -> tgui::Element<ShowcaseVm> {
    Flex::new(Axis::Vertical)
        .width(pct(100.0))
        .gap(dp(18.0))
        .child(
            Stack::new()
                .padding(Insets::all(dp(20.0)))
                .background(Color::hexa(0x123552FF))
                .border_radius(dp(20.0))
                .child(
                    Flex::new(Axis::Vertical)
                        .gap(dp(10.0))
                        .child(
                            Text::new("Page 3: canvas")
                                .font_size(sp(26.0))
                                .color(Color::WHITE),
                        )
                        .child(
                            Text::new(
                                "This page demonstrates Canvas with gradient fills, a dashed stroke, a boolean-difference path, shadow rendering, and pointer hit feedback.",
                            )
                            .font_size(sp(15.0))
                            .color(Color::hexa(0xD6EFFF)),
                        ),
                ),
        )
        .child(
            Stack::new()
                .padding(Insets::all(dp(16.0)))
                .background(Color::hexa(0x08111BFF))
                .border_radius(dp(20.0))
                .overflow_x(Overflow::Scroll)
                .overflow_y(Overflow::Scroll)
                .scrollbar_style(
                    ScrollbarStyle::default()
                        .thumb_color(Color::hexa(0x4EA8DECC))
                        .hover_thumb_color(Color::hexa(0x89C2D9FF)),
                )
                .child(
                    Canvas::new(canvas_items())
                        .size(dp(860.0), dp(560.0))
                        .background(Color::hexa(0x102131FF))
                        .border(dp(1.0), Color::hexa(0x315977FF))
                        .border_radius(dp(18.0))
                        .on_item_mouse_move(ValueCommand::new(ShowcaseVm::note_canvas_hover))
                        .on_item_click(ValueCommand::new(ShowcaseVm::note_canvas_click)),
                ),
        )
        .child(
            Flex::new(Axis::Horizontal)
                .gap(dp(12.0))
                .child(
                    Stack::new()
                        .grow(1.0)
                        .child(status_card("Hover state", vm.canvas_hover.binding())),
                )
                .child(
                    Stack::new()
                        .grow(1.0)
                        .child(status_card("Click state", vm.canvas_clicked.binding())),
                ),
        )
        .into()
}

fn canvas_items() -> Vec<CanvasItem> {
    let boolean_base = PathBuilder::new()
        .move_to(560.0, 110.0)
        .line_to(760.0, 110.0)
        .line_to(760.0, 300.0)
        .line_to(560.0, 300.0)
        .close();
    let boolean_cutout = PathBuilder::new()
        .move_to(620.0, 150.0)
        .line_to(700.0, 150.0)
        .line_to(700.0, 260.0)
        .line_to(620.0, 260.0)
        .close();
    let boolean_shape = boolean_base
        .difference(&boolean_cutout)
        .expect("boolean difference should produce a path");

    vec![
        CanvasItem::Path(
            CanvasPath::new(
                1_u64,
                PathBuilder::new()
                    .move_to(40.0, 60.0)
                    .line_to(260.0, 60.0)
                    .line_to(260.0, 220.0)
                    .line_to(40.0, 220.0)
                    .close(),
            )
            .fill(CanvasLinearGradient::new(
                Point::new(40.0, 60.0),
                Point::new(260.0, 220.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                    CanvasGradientStop::new(0.55, Color::hexa(0x2563EBFF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                ],
            ))
            .stroke(CanvasStroke::new(dp(4.0), Color::hexa(0xE0F2FEFF)))
            .shadow(CanvasShadow::new(
                Color::hexa(0x0F172A99),
                Point::new(12.0, 12.0),
                dp(10.0),
            )),
        ),
        CanvasItem::Path(
            CanvasPath::new(
                2_u64,
                PathBuilder::new()
                    .move_to(320.0, 90.0)
                    .cubic_to(380.0, 10.0, 520.0, 210.0, 620.0, 90.0),
            )
            .stroke(
                CanvasStroke::with_brush(
                    dp(10.0),
                    CanvasLinearGradient::new(
                        Point::new(320.0, 90.0),
                        Point::new(620.0, 90.0),
                        vec![
                            CanvasGradientStop::new(0.0, Color::hexa(0xF59E0BFF)),
                            CanvasGradientStop::new(1.0, Color::hexa(0xEF4444FF)),
                        ],
                    ),
                )
                .dash([dp(18.0), dp(14.0)])
                .dash_offset(dp(6.0)),
            ),
        ),
        CanvasItem::Path(
            CanvasPath::new(
                3_u64,
                PathBuilder::new()
                    .move_to(90.0, 320.0)
                    .quad_to(180.0, 230.0, 270.0, 320.0)
                    .quad_to(360.0, 410.0, 450.0, 320.0)
                    .line_to(450.0, 500.0)
                    .line_to(90.0, 500.0)
                    .close(),
            )
            .fill(CanvasRadialGradient::new(
                Point::new(270.0, 360.0),
                dp(160.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0xDCFCE7FF)),
                    CanvasGradientStop::new(0.55, Color::hexa(0x4ADE80FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x166534FF)),
                ],
            ))
            .stroke(CanvasStroke::new(dp(3.0), Color::hexa(0x14532DFF))),
        ),
        CanvasItem::Path(
            CanvasPath::new(4_u64, boolean_shape)
                .fill(Color::hexa(0xF8FAFCFF))
                .stroke(CanvasStroke::new(dp(5.0), Color::hexa(0x8B5CF6FF)))
                .shadow(CanvasShadow::new(
                    Color::hexa(0x312E8199),
                    Point::new(0.0, 10.0),
                    dp(8.0),
                )),
        ),
    ]
}

fn status_card(title: &str, value: tgui::Binding<String>) -> tgui::Element<ShowcaseVm> {
    Flex::new(Axis::Vertical)
        .padding(Insets::all(dp(16.0)))
        .gap(dp(10.0))
        .background(Color::hexa(0x0F2439FF))
        .border(dp(1.0), Color::hexa(0x264761FF))
        .border_radius(dp(16.0))
        .child(Text::new(title).font_size(sp(18.0)).color(Color::WHITE))
        .child(
            Text::new(value)
                .padding(Insets::all(dp(12.0)))
                .background(Color::hexa(0x102131FF))
                .border_radius(dp(12.0))
                .color(Color::hexa(0xE0F2FEFF)),
        )
        .into()
}
