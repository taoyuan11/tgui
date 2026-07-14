use tgui::prelude::*;

struct AppVm;

fn root_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(ctx.theme.colors.background.into());
    style
}

fn starter_card_style(ctx: &StyleContext<'_>) -> CardStyle {
    let mut style = CardStyle::default_for_theme(ctx.theme);
    style.background = ctx.theme.colors.surface.into();
    style.border = ctx.theme.colors.outline_muted.into();
    style.border_width = ctx.theme.border.thin;
    style.radius = ctx.theme.radius.xl;
    style.shadow = ctx.theme.elevation.md.clone();
    style.padding = Insets::all(ctx.theme.spacing.lg);
    style.gap = ctx.theme.spacing.md;
    style
}

fn title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography = ctx.theme.typography.headline.clone();
    style.color = ctx.theme.colors.on_surface.into();
    style
}

fn body_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography = ctx.theme.typography.body.clone();
    style.color = ctx.theme.colors.on_surface_muted.into();
    style
}

fn supporting_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography = ctx.theme.typography.body_small.clone();
    style.color = ctx.theme.colors.on_surface_muted.into();
    style
}

impl ViewModel for AppVm {
    fn new(_: &ViewModelContext) -> Self {
        Self
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(32.0)))
            .center()
            .style_full(root_style)
            .child(
                Card::new()
                    .width(pct(100.0))
                    .max_width(dp(640.0))
                    .style_full(starter_card_style)
                    .header(
                        Flex::vertical()
                            .gap(dp(8.0))
                            .child(Badge::text("STARTER").tone(BadgeTone::Primary))
                            .child(Text::new("Build your first tgui window").style_full(title_style)),
                    )
                    .body(
                        Text::new(
                            "A focused MVVM starter with theme-aware components and responsive layout.",
                        )
                        .width(pct(100.0))
                        .style_full(body_style),
                    )
                    .footer(
                        Flex::vertical()
                            .gap(dp(12.0))
                            .child(Divider::new())
                            .child(
                                Text::new("Rust · wgpu · MVVM")
                                    .style_full(supporting_style),
                            ),
                    ),
            )
            .into()
    }
}

fn main() -> Result<(), TguiError> {
    tgui::init_logging_from_cargo_toml!()?;
    tgui_log(LogLevel::Info, "starting...");

    let result = Application::new()
        .title("tgui basic window")
        .window_size(dp(960.0), dp(640.0))
        .theme_mode(ThemeMode::System)
        .with_view_model(AppVm::new)
        .root_view(AppVm::view)
        .run();

    tgui_log(LogLevel::Info, "stopping...");
    result
}
