use tgui::prelude::*;

struct AppVm;

impl AppVm {
    fn resize_grips() -> Vec<Element<Self>> {
        vec![
            Self::resize_grip(WindowResizeDirection::North, CursorStyle::NsResize),
            Self::resize_grip(WindowResizeDirection::South, CursorStyle::NsResize),
            Self::resize_grip(WindowResizeDirection::West, CursorStyle::EwResize),
            Self::resize_grip(WindowResizeDirection::East, CursorStyle::EwResize),
            Self::resize_grip(WindowResizeDirection::NorthWest, CursorStyle::NwseResize),
            Self::resize_grip(WindowResizeDirection::NorthEast, CursorStyle::NeswResize),
            Self::resize_grip(WindowResizeDirection::SouthWest, CursorStyle::NeswResize),
            Self::resize_grip(WindowResizeDirection::SouthEast, CursorStyle::NwseResize),
        ]
    }

    fn resize_grip(direction: WindowResizeDirection, cursor: CursorStyle) -> Element<Self> {
        let edge = dp(6.0);
        let corner = dp(14.0);
        let grip = Stack::new()
            .position_absolute()
            .background(Color::hexa(0x00000000))
            .cursor(cursor)
            .on_click(Command::new_with_context(move |_: &mut Self, context| {
                context.window().drag_resize_window(direction);
            }));

        match direction {
            WindowResizeDirection::North => grip
                .height(edge)
                .left(corner)
                .right(corner)
                .top(dp(0.0))
                .into(),
            WindowResizeDirection::South => grip
                .height(edge)
                .left(corner)
                .right(corner)
                .bottom(dp(0.0))
                .into(),
            WindowResizeDirection::West => grip
                .width(edge)
                .left(dp(0.0))
                .top(corner)
                .bottom(corner)
                .into(),
            WindowResizeDirection::East => grip
                .width(edge)
                .right(dp(0.0))
                .top(corner)
                .bottom(corner)
                .into(),
            WindowResizeDirection::NorthWest => grip
                .size(corner, corner)
                .left(dp(0.0))
                .top(dp(0.0))
                .into(),
            WindowResizeDirection::NorthEast => grip
                .size(corner, corner)
                .right(dp(0.0))
                .top(dp(0.0))
                .into(),
            WindowResizeDirection::SouthWest => grip
                .size(corner, corner)
                .left(dp(0.0))
                .bottom(dp(0.0))
                .into(),
            WindowResizeDirection::SouthEast => grip
                .size(corner, corner)
                .right(dp(0.0))
                .bottom(dp(0.0))
                .into(),
        }
    }

    fn title_bar(&self) -> Element<Self> {
        Flex::new(Axis::Horizontal)
            .height(dp(48.0))
            .width(pct(100.0))
            .align(Align::Center)
            .padding(Insets::symmetric(dp(18.0), dp(0.0)))
            .gap(dp(8.0))
            .on_click(Command::new_with_context(|_: &mut Self, context| {
                context.window().drag_window();
            }))
            .child(
                Text::new("tgui frameless")
                    .font_size(sp(15.0))
                    .color(Color::hexa(0xF8FAFCFF))
                    .grow(1.0),
            )
            .child(Self::window_button(
                "-",
                Color::hexa(0x1F2937FF),
                |context| context.window().minimize(),
            ))
            .child(Self::window_button(
                "[]",
                Color::hexa(0x1F2937FF),
                |context| context.window().toggle_maximize(),
            ))
            .child(Self::window_button(
                "x",
                Color::hexa(0x7F1D1DFF),
                |context| context.window().close(),
            ))
            .into()
    }

    fn window_button(
        label: &'static str,
        background: Color,
        action: impl Fn(&CommandContext<Self>) + Send + Sync + 'static,
    ) -> Button<Self> {
        Button::new(
            Text::new(label)
                .font_size(sp(15.0))
                .color(Color::hexa(0xF8FAFCFF)),
        )
        .size(dp(38.0), dp(30.0))
        .padding(Insets::all(dp(0.0)))
        .background(background)
        .border_radius(dp(6.0))
        .on_click(Command::new_with_context(move |_: &mut Self, context| {
            action(context);
        }))
    }
}

impl ViewModel for AppVm {
    fn new(_: &ViewModelContext) -> Self {
        Self
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .child(
                Flex::new(Axis::Vertical)
                    .size(pct(100.0), pct(100.0))
                    .background(Color::hexa(0x0B1020FF))
                    .border(dp(1.0), Color::hexa(0x334155FF))
                    .border_radius(dp(18.0))
                    .child(self.title_bar())
                    .child(
                        Stack::new()
                            .size(pct(100.0), pct(100.0))
                            .padding(Insets::all(dp(28.0)))
                            .center()
                            .child(
                                Flex::new(Axis::Vertical)
                                    .width(dp(560.0))
                                    .padding(Insets::all(dp(26.0)))
                                    .gap(dp(14.0))
                                    .background(Color::hexa(0x162033EE))
                                    .border(dp(1.0), Color::hexa(0x334155FF))
                                    .border_radius(dp(16.0))
                                    .child(
                                        Text::new("Custom chrome")
                                            .font_size(sp(28.0))
                                            .color(Color::hexa(0xF8FAFCFF)),
                                    )
                                    .child(
                                        Text::new(
                                            "This window is created with native decorations disabled.",
                                        )
                                        .font_size(sp(16.0))
                                        .color(Color::hexa(0xCBD5E1FF)),
                                    )
                                    .child(
                                        Text::new(
                                            "The custom edges and corners start native resize drags.",
                                        )
                                        .font_size(sp(15.0))
                                        .color(Color::hexa(0x93C5FDFF)),
                                    )
                                    .child(
                                        Text::new(
                                            "The top bar is regular tgui UI wired to runtime window controls.",
                                        )
                                        .font_size(sp(15.0))
                                        .color(Color::hexa(0x93C5FDFF)),
                                    ),
                            ),
                    ),
            )
            .child(Self::resize_grips())
            .into()
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("tgui frameless window")
        .window_size(dp(900.0), dp(620.0))
        .decorations(false)
        .clear_color(Color::TRANSPARENT)
        .with_view_model(AppVm::new)
        .root_view(AppVm::view)
        .run()
}
