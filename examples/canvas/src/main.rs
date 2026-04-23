use tgui::{dp, sp, Application, Binding, Canvas, CanvasItem, CanvasPath, CanvasPointerEvent, CanvasStroke, Color, Column, Insets, Observable, Overflow, PathBuilder, ScrollbarStyle, Stack, Text, ValueCommand, ViewModelContext};

struct CanvasVm {
    hovered: Observable<String>,
    clicked: Observable<u64>,
}

impl CanvasVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            hovered: ctx.observable("Move over a shape".to_string()),
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
        self.clicked
            .set(event.item_id.get());
    }
    
    fn get_color(&self, id: u64) -> Binding<Color> {
        self.clicked.binding().map(move |i| {
            return if i == id {
                Color::hexa(0x2563EBFF)
            } else {
                Color::hexa(0xF9FAFBFF)
            }
        })
    }

    fn view(&self) -> tgui::Element<Self> {
        let items = vec![
            CanvasItem::Path(
                CanvasPath::new(
                    1_u64,
                    PathBuilder::new()
                        .move_to(40.0, 40.0)
                        .line_to(260.0, 40.0)
                        .line_to(260.0, 160.0)
                        .line_to(40.0, 160.0)
                        .close(),
                )
                .fill(self.get_color(1_u64))
                .stroke(CanvasStroke::new(dp(4.0), Color::hexa(0xDBEAFEFF))),
            ),
            CanvasItem::Path(
                CanvasPath::new(
                    2_u64,
                    PathBuilder::new()
                        .move_to(320.0, 70.0)
                        .cubic_to(420.0, 0.0, 520.0, 210.0, 640.0, 120.0)
                        .line_to(640.0, 320.0)
                        .line_to(320.0, 320.0)
                        .close(),
                )
                    .fill(self.get_color(2_u64))
                .stroke(CanvasStroke::new(dp(6.0), Color::hexa(0xFFEDD5FF))),
            ),
            CanvasItem::Path(
                CanvasPath::new(
                    3_u64,
                    PathBuilder::new()
                        .move_to(120.0, 240.0)
                        .quad_to(220.0, 160.0, 320.0, 240.0)
                        .quad_to(420.0, 320.0, 520.0, 240.0)
                        .line_to(520.0, 420.0)
                        .line_to(120.0, 420.0)
                        .close(),
                )
                    .fill(self.get_color(3_u64))
                .stroke(CanvasStroke::new(dp(4.0), Color::hexa(0xDCFCE7FF))),
            ),
        ];

        Column::new()
            .fill_size()
            .padding(Insets::all(dp(24.0)))
            .gap(dp(16.0))
            .background(Color::hexa(0x0F172AFF))
            .child(
                Text::new("Declarative Canvas")
                    .font_size(sp(28.0))
                    .color(Color::WHITE),
            )
            .child(
                Text::new(
                    "The canvas below is scrollable and each path reports its own hover/click payload.",
                )
                .font_size(sp(15.0))
                .color(Color::hexa(0xCBD5E1FF)),
            )
            .child(
                Stack::new()
                    .height(dp(520.0))
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
                            .size(dp(760.0), dp(520.0))
                            .background(Color::hexa(0x111827FF))
                            .border(dp(1.0), Color::hexa(0x334155FF))
                            .border_radius(dp(18.0))
                            .on_item_mouse_move(ValueCommand::new(Self::on_hover))
                            .on_item_click(ValueCommand::new(Self::on_click)),
                    ),
            )
            .child(
                Column::new()
                    .gap(dp(8.0))
                    .child(
                        Text::new(self.hovered.binding())
                            .padding(Insets::all(dp(12.0)))
                            .background(Color::hexa(0x111827FF))
                            .border_radius(dp(12.0))
                            .color(Color::hexa(0xDBEAFEFF)),
                    )
                    .child(
                        Text::new(self.clicked.binding().map(|i| {format!("clicked item={}", i)}))
                            .padding(Insets::all(dp(12.0)))
                            .background(Color::hexa(0x111827FF))
                            .border_radius(dp(12.0))
                            .color(Color::hexa(0xFED7AAFF)),
                    ),
            )
            .into()
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .title("tgui Canvas")
        .window_size(dp(1100.0), dp(820.0))
        .with_view_model(CanvasVm::new)
        .root_view(CanvasVm::view)
        .run()
}
