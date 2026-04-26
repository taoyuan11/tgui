use tgui::{
    dp, sp, Application, Canvas, CanvasGradientStop, CanvasItem, CanvasLinearGradient, CanvasPath,
    CanvasPointerEvent, CanvasRadialGradient, CanvasShadow, CanvasStroke, Color, Flex, Insets,
    Observable, Overflow, PathBuilder, Point, ScrollbarStyle, Stack, Text, ValueCommand, Axis,
    ViewModel, ViewModelContext, pct,
};

struct CanvasVm {
    hovered: Observable<String>,
    clicked: Observable<u64>,
}

impl CanvasVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            hovered: ctx.observable("Move over a path".to_string()),
            clicked: ctx.observable(0u64),
        }
    }

    fn on_hover(&mut self, event: CanvasPointerEvent) {
        self.hovered.set(format!(
            "hover item={} canvas=({:.0}, {:.0}) local=({:.0}, {:.0})",
            event.item_id.get(),
            event.canvas_position.x,
            event.canvas_position.y,
            event.local_position.x,
            event.local_position.y
        ));
    }

    fn on_click(&mut self, event: CanvasPointerEvent) {
        self.clicked.set(event.item_id.get());
    }

    fn view(&self) -> tgui::Element<Self> {
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

        let items = vec![
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
        ];

        Flex::new(Axis::Vertical)
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .gap(dp(16.0))
            .background(Color::hexa(0x0B1120FF))
            .child(
                Text::new("Canvas: gradients, dashed strokes, shadows, boolean ops")
                    .font_size(sp(26.0))
                    .color(Color::WHITE),
            )
            .child(
                Text::new(
                    "The surface below mixes linear/radial gradients, dashed stroke geometry, drop shadows, and a boolean-difference path.",
                )
                .font_size(sp(15.0))
                .color(Color::hexa(0xBFDBFEFF)),
            )
            .child(
                Stack::new()
                    .height(dp(560.0))
                    .padding(Insets::all(dp(16.0)))
                    .background(Color::hexa(0x020617FF))
                    .border_radius(dp(20.0))
                    .overflow_x(Overflow::Scroll)
                    .overflow_y(Overflow::Scroll)
                    .scrollbar_style(
                        ScrollbarStyle::default()
                            .thumb_color(Color::hexa(0x60A5FAE0))
                            .hover_thumb_color(Color::hexa(0x93C5FDFF)),
                    )
                    .child(
                        Canvas::new(items)
                            .size(dp(840.0), dp(560.0))
                            .background(Color::hexa(0x111827FF))
                            .border(dp(1.0), Color::hexa(0x334155FF))
                            .border_radius(dp(18.0))
                            .on_item_mouse_move(ValueCommand::new(Self::on_hover))
                            .on_item_click(ValueCommand::new(Self::on_click)),
                    ),
            )
            .child(
                Flex::new(Axis::Vertical)
                    .gap(dp(8.0))
                    .child(
                        Text::new(self.hovered.binding())
                            .padding(Insets::all(dp(12.0)))
                            .background(Color::hexa(0x111827FF))
                            .border_radius(dp(12.0))
                            .color(Color::hexa(0xDBEAFEFF)),
                    )
                    .child(
                        Text::new(
                            self.clicked
                                .binding()
                                .map(|item_id| format!("clicked item={item_id}")),
                        )
                        .padding(Insets::all(dp(12.0)))
                        .background(Color::hexa(0x111827FF))
                        .border_radius(dp(12.0))
                        .color(Color::hexa(0xFDE68AFF)),
                    ),
            )
            .into()
    }
}

impl ViewModel for CanvasVm {}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .title("tgui Canvas")
        .window_size(dp(1160.0), dp(920.0))
        .with_view_model(CanvasVm::new)
        .root_view(CanvasVm::view)
        .run()
}
