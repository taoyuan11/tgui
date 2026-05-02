use tgui::prelude::*;

struct AppVm {
    confirm_exit_open: Observable<bool>,
    app_open: Observable<bool>,
}

impl AppVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            confirm_exit_open: ctx.observable(false),
            app_open: ctx.observable(true),
        }
    }

    fn main_title(&self) -> Binding<String> {
        self.confirm_exit_open.binding().map(|open| {
            if open {
                "tgui frameless window (modal confirmation open)".to_string()
            } else {
                "tgui frameless window".to_string()
            }
        })
    }

    fn request_exit(&mut self) {
        self.confirm_exit_open.set(true);
    }

    fn cancel_exit(&mut self) {
        self.confirm_exit_open.set(false);
    }

    fn confirm_exit(&mut self) {
        self.confirm_exit_open.set(false);
        self.app_open.set(false);
    }

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

    fn window_button(label: &'static str, background: Color, command: Command<Self>) -> Button<Self> {
        Button::new(
            Text::new(label)
                .font_size(sp(15.0))
                .color(Color::hexa(0xF8FAFCFF)),
        )
        .size(dp(38.0), dp(30.0))
        .padding(Insets::all(dp(0.0)))
        .background(background)
        .border_radius(dp(6.0))
        .on_click(command)
    }

    fn main_title_bar(&self) -> Element<Self> {
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
                Command::new_with_context(|_: &mut Self, context| {
                    context.window().minimize();
                }),
            ))
            .child(Self::window_button(
                "[]",
                Color::hexa(0x1F2937FF),
                Command::new_with_context(|_: &mut Self, context| {
                    context.window().toggle_maximize();
                }),
            ))
            .child(Self::window_button(
                "x",
                Color::hexa(0x7F1D1DFF),
                Command::new(Self::request_exit),
            ))
            .into()
    }

    fn modal_title_bar(&self) -> Element<Self> {
        Flex::new(Axis::Horizontal)
            .height(dp(44.0))
            .width(pct(100.0))
            .align(Align::Center)
            .padding(Insets::symmetric(dp(16.0), dp(0.0)))
            .gap(dp(8.0))
            .background(Color::hexa(0x1A2440FF))
            .on_click(Command::new_with_context(|_: &mut Self, context| {
                context.window().drag_window();
            }))
            .child(
                Text::new("Confirm exit")
                    .font_size(sp(14.0))
                    .color(Color::hexa(0xE2E8F0FF))
                    .grow(1.0),
            )
            .child(Self::window_button(
                "x",
                Color::hexa(0x312E81FF),
                Command::new(Self::cancel_exit),
            ))
            .into()
    }

    fn main_view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .child(
                Flex::new(Axis::Vertical)
                    .size(pct(100.0), pct(100.0))
                    .background(Color::hexa(0x0B1020FF))
                    .border_radius(dp(18.0))
                    .child(self.main_title_bar())
                    .child(
                        Stack::new()
                            .size(pct(100.0), pct(100.0))
                            .padding(Insets::all(dp(28.0)))
                            .center()
                            .child(
                                Flex::new(Axis::Vertical)
                                    .width(dp(580.0))
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
                                            "This example runs with native decorations disabled and opens a custom modal confirmation window before exit.",
                                        )
                                        .font_size(sp(16.0))
                                        .color(Color::hexa(0xCBD5E1FF)),
                                    )
                                    .child(
                                        Text::new(
                                            "On supported platforms the confirmation window is also wired up as a native owned/modal child, while tgui still gates main-window input everywhere else.",
                                        )
                                        .font_size(sp(15.0))
                                        .color(Color::hexa(0x93C5FDFF)),
                                    )
                                    .child(
                                        Text::new(
                                            "The custom edges and corners still start native resize drags, and the top bar remains regular tgui UI.",
                                        )
                                        .font_size(sp(15.0))
                                        .color(Color::hexa(0x93C5FDFF)),
                                    )
                                    .child(
                                        Button::new(Text::new("Open modal confirmation"))
                                            .height(dp(42.0))
                                            .background(Color::hexa(0x0F766EFF))
                                            .border_radius(dp(12.0))
                                            .on_click(Command::new(Self::request_exit)),
                                    ),
                            ),
                    ),
            )
            .child(Self::resize_grips())
            .into()
    }

    fn confirm_exit_view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .overflow_x(Overflow::Scroll)
            .child(
                Flex::new(Axis::Vertical)
                    .size(pct(100.0), pct(100.0))
                    .background(Color::hexa(0x101827FF))
                    .border_radius(dp(18.0))
                    .child(self.modal_title_bar())
                    .child(
                        Flex::new(Axis::Vertical)
                            .size(pct(100.0), pct(100.0))
                            .padding(Insets::all(dp(22.0)))
                            .gap(dp(12.0))
                            .child(
                                Text::new("Exit the frameless demo?")
                                    .font_size(sp(24.0))
                                    .color(Color::hexa(0xF8FAFCFF)),
                            )
                            .child(
                                Text::new(
                                    "This confirmation window is also undecorated. Try clicking the main window behind it while this dialog is open.",
                                )
                                .font_size(sp(15.0))
                                .color(Color::hexa(0xCBD5E1FF)),
                            )
                            .child(
                                Text::new(
                                    "Cancel keeps the app alive. Confirm removes the main window from the runtime and exits the application.",
                                )
                                .font_size(sp(14.0))
                                .color(Color::hexa(0x93C5FDFF)),
                            )
                            .child(
                                Flex::new(Axis::Horizontal)
                                    .gap(dp(10.0))
                                    .padding(Insets::top(dp(8.0)))
                                    .child(
                                        Button::new(Text::new("Cancel"))
                                            .grow(1.0)
                                            .height(dp(40.0))
                                            .background(Color::hexa(0x1F2937FF))
                                            .border_radius(dp(10.0))
                                            .on_click(Command::new(Self::cancel_exit)),
                                    )
                                    .child(
                                        Button::new(Text::new("Exit app"))
                                            .grow(1.0)
                                            .height(dp(40.0))
                                            .background(Color::hexa(0x991B1BFF))
                                            .border_radius(dp(10.0))
                                            .on_click(Command::new(Self::confirm_exit)),
                                    ),
                            ),
                    ),
            )
            .child(Self::resize_grips())
            .into()
    }

    fn windows(&self) -> Vec<WindowSpec<Self>> {
        if !self.app_open.get() {
            return Vec::new();
        }

        let mut windows = vec![WindowSpec::main("main")
            .title("tgui frameless window")
            .window_size(dp(900.0), dp(620.0))
            .min_window_size(dp(760.0), dp(520.0))
            .bind_title(Self::main_title)
            .root_view(Self::main_view)];

        if self.confirm_exit_open.get() {
            windows.push(
                WindowSpec::child("confirm-exit")
                    .title("Confirm exit")
                    .window_size(dp(440.0), dp(250.0))
                    .min_window_size(dp(400.0), dp(230.0))
                    .max_window_size(dp(520.0), dp(280.0))
                    .decorations(false)
                    .blocks_main_window(true)
                    .root_view(Self::confirm_exit_view),
            );
        }

        windows
    }
}

impl ViewModel for AppVm {
    fn new(ctx: &ViewModelContext) -> Self {
        AppVm::new(ctx)
    }

    fn view(&self) -> Element<Self> {
        AppVm::main_view(self)
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("tgui frameless window")
        .window_size(dp(900.0), dp(620.0))
        .decorations(false)
        .clear_color(Color::TRANSPARENT)
        .with_view_model(AppVm::new)
        .windows(AppVm::windows)
        .run()
}
