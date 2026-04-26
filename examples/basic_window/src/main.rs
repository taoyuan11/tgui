use tgui::{dp, el, pct, sp, tgui_log, Application, Axis, Color, Flex, Insets, LogLevel, Stack, Text, TguiError, Theme};

fn main() -> Result<(), TguiError> {
    let mut theme = Theme::dark();
    theme.palette.window_background = Color::hexa(0x0B1220FF);
    theme.palette.surface = Color::hexa(0x111B2EFF);
    theme.palette.surface_muted = Color::hexa(0x1B2942FF);
    theme.palette.accent = Color::hexa(0x4F9CF9FF);

    tgui_log(LogLevel::Info, "starting...");

    let result = Application::new()
        .title("tgui basic window")
        .window_size(dp(960.0), dp(640.0))
        .theme(theme)
        .with_view_model(|_| ())
        .root_view(|_| {
            Stack::new()
                .size(pct(100.0), pct(100.0))
                .padding(Insets::all(dp(36.0)))
                .center()
                .child(
                    Flex::new(Axis::Vertical)
                        .width(pct(100.0))
                        .padding(Insets::all(dp(28.0)))
                        .gap(dp(14.0))
                        .background(Color::hexa(0x16233AFF))
                        .border(dp(1.0), Color::hexa(0x33507DFF))
                        .border_radius(dp(20.0))
                        .child(el![
                            Text::new("Hello, tgui")
                                .font_size(sp(28.0))
                                .color(Color::hexa(0xF7FAFFFF)),
                            Text::new(
                                "This example keeps things intentionally simple: one window, one card, and a small static widget tree.",
                            )
                                .font_size(sp(16.0))
                                .color(Color::hexa(0xC2D3F1FF)),
                            Text::new(
                                "Use it as the smallest complete starting point before moving on to MVVM, input, theming, and animation examples.",
                            )
                                .font_size(sp(15.0))
                                .color(Color::hexa(0x9AB3D9FF)),
                        ])
                )
                .into()
        })
        .run();
    tgui_log(LogLevel::Info, "stopping...");
    result
}
