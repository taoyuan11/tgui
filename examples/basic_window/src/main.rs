use tgui::{Align, Application, Color, Column, Insets, Justify, Stack, Text, TguiError, Theme, dp, sp, Command, LogLevel};

fn main() -> Result<(), TguiError> {
    let mut theme = Theme::dark();
    theme.palette.window_background = Color::hexa(0x0B1220FF);
    theme.palette.surface = Color::hexa(0x111B2EFF);
    theme.palette.surface_muted = Color::hexa(0x1B2942FF);
    theme.palette.accent = Color::hexa(0x4F9CF9FF);

    Application::new()
        .title("tgui basic window")
        .window_size(dp(960.0), dp(640.0))
        .theme(theme)
        .with_view_model(|_| ())
        .root_view(|_| {
            Stack::new()
                .fill_size()
                .padding(Insets::all(dp(36.0)))
                .align(Align::Center)
                .justify(Justify::Center)
                .child(
                    Column::new()
                        .width(dp(460.0))
                        .padding(Insets::all(dp(28.0)))
                        .gap(dp(14.0))
                        .background(Color::hexa(0x16233AFF))
                        .border(dp(1.0), Color::hexa(0x33507DFF))
                        .border_radius(dp(20.0))
                        .on_click(Command::new_with_context(|_, context| {
                            context
                                .log()
                                .log(LogLevel::Debug, "11111111111111111111111111111111");
                        }))
                        .child(
                            Text::new("Hello, tgui")
                                .font_size(sp(28.0))
                                .color(Color::hexa(0xF7FAFFFF)),
                        )
                        .child(
                            Text::new(
                                "This example keeps things intentionally simple: one window, one card, and a small static widget tree.",
                            )
                            .font_size(sp(16.0))
                            .color(Color::hexa(0xC2D3F1FF)),
                        )
                        .child(
                            Text::new(
                                "Use it as the smallest complete starting point before moving on to MVVM, input, theming, and animation examples.",
                            )
                            .font_size(sp(15.0))
                            .color(Color::hexa(0x9AB3D9FF)),
                        ),
                )
                .into()
        })
        .run()
}
