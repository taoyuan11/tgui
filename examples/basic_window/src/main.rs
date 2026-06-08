use tgui::prelude::*;

struct AppVm;

fn card_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hexa(0x16233AFF).into());
    style.surface.border_color = Some(Color::hexa(0x33507DFF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(20.0).into());
    style
}

fn title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = sp(28.0);
    style.color = Color::hexa(0xF7FAFFFF).into();
    style
}

fn body_style(ctx: &StyleContext<'_>, size: Sp, color: Color) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = size;
    style.color = color.into();
    style
}

impl ViewModel for AppVm {
    fn new(_: &ViewModelContext) -> Self {
        Self
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(36.0)))
            .center()
            .child(
                Flex::new(Axis::Vertical)
                    .width(pct(100.0))
                    .padding(Insets::all(dp(28.0)))
                    .gap(dp(14.0))
                    .style_full(card_style)
                    .child(el![
                        Text::new("Hello, tgui").style_full(title_style),
                        Text::new(
                            "This example keeps things intentionally simple: one window, one card, and a small static widget tree.",
                        )
                        .style_full(|ctx| body_style(ctx, sp(16.0), Color::hexa(0xC2D3F1FF))),
                        Text::new(
                            "Use it as the smallest complete MVVM starting point before moving on to input, theming, and animation examples.",
                        )
                        .style_full(|ctx| body_style(ctx, sp(15.0), Color::hexa(0x9AB3D9FF))),
                    ]),
            )
            .into()
    }
}

fn main() -> Result<(), TguiError> {
    tgui::init_logging_from_cargo_toml!()?;

    let mut theme = Theme::dark();
    theme.colors.background = Color::hexa(0x0B1220FF);
    theme.colors.surface = Color::hexa(0x111B2EFF);
    theme.colors.surface_low = Color::hexa(0x1B2942FF);
    theme.colors.primary = Color::hexa(0x4F9CF9FF);
    let theme_set = ThemeSet::new(theme.clone(), theme);

    tgui_log(LogLevel::Info, "starting...");

    let result = Application::new()
        .title("tgui basic window")
        .window_size(dp(960.0), dp(640.0))
        .theme_set(theme_set)
        .theme_mode(ThemeMode::Dark)
        .with_view_model(AppVm::new)
        .root_view(AppVm::view)
        .run();
    tgui_log(LogLevel::Info, "stopping...");
    result
}
