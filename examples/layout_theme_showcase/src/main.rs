use tgui::{
    Align, Application, Axis, Button, Color, Column, Flex, FontWeight, Grid, Insets, Row,
    Stack, Text, TguiError, Theme, Wrap, dp, sp,
};

fn main() -> Result<(), TguiError> {
    let mut theme = Theme::dark();
    theme.palette.window_background = Color::hexa(0x081019FF);
    theme.palette.surface = Color::hexa(0x101C2BFF);
    theme.palette.surface_muted = Color::hexa(0x173047FF);
    theme.palette.accent = Color::hexa(0x4FD1C5FF);
    theme.typography.font_size = sp(16.0);

    Application::new()
        .title("tgui layout + theme showcase")
        .window_size(dp(1180.0), dp(760.0))
        .theme(theme)
        .with_view_model(|_| ())
        .root_view(|_| {
            Column::new()
                .fill_size()
                .padding(Insets::all(dp(24.0)))
                .gap(dp(18.0))
                .child(hero())
                .child(
                    Row::new()
                        .gap(dp(18.0))
                        .child(metric_grid().grow(1.4))
                        .child(sidebar().grow(1.0)),
                )
                .into()
        })
        .run()
}

fn hero() -> tgui::Element<()> {
    Row::new()
        .padding(Insets::all(dp(22.0)))
        .gap(dp(18.0))
        .background(Color::hexa(0x0F1B2BFF))
        .border(dp(1.0), Color::hexa(0x23435FFF))
        .border_radius(dp(22.0))
        .child(
            Column::new()
                .grow(1.0)
                .gap(dp(12.0))
                .child(
                    Text::new("Layout + theme showcase")
                        .font_size(sp(30.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .color(Color::hexa(0xF0FDFAFF)),
                )
                .child(
                    Text::new("A dashboard-like example that combines custom theme colors with nested layout containers.")
                        .font_size(sp(16.0))
                        .color(Color::hexa(0xB8E6E1FF)),
                )
                .child(
                    Row::new()
                        .gap(dp(10.0))
                        .child(
                            Button::new(Text::new("Primary action"))
                                .background(Color::hexa(0x0F766EFF))
                                .border_radius(dp(12.0)),
                        )
                        .child(
                            Button::new(Text::new("Secondary"))
                                .background(Color::hexa(0x1E3A5FFF))
                                .border_radius(dp(12.0)),
                        ),
                ),
        )
        .child(
            Stack::new()
                .width(dp(220.0))
                .height(dp(140.0))
                .background(Color::hexa(0x103B43FF))
                .border_radius(dp(20.0))
                .align(Align::Center)
                .child(
                    Text::new("Custom theme")
                        .font_size(sp(20.0))
                        .color(Color::hexa(0xE6FFFBFF)),
                ),
        )
        .into()
}

fn metric_grid() -> Column<()> {
    Column::new()
        .padding(Insets::all(dp(18.0)))
        .gap(dp(14.0))
        .background(Color::hexa(0x0E1826FF))
        .border(dp(1.0), Color::hexa(0x223B58FF))
        .border_radius(dp(18.0))
        .child(
            Text::new("Metrics")
                .font_size(sp(22.0))
                .color(Color::hexa(0xF8FAFCFF)),
        )
        .child(
            Grid::new(2)
                .gap(dp(12.0))
                .child(metric_card(
                    "Active users",
                    "14,280",
                    Color::hexa(0x0F766EFF),
                ))
                .child(metric_card("Conversion", "8.4%", Color::hexa(0x2563EBFF)))
                .child(metric_card("Net growth", "+21%", Color::hexa(0x9333EAFF)))
                .child(metric_card("Satisfaction", "94%", Color::hexa(0xEA580CFF))),
        )
}

fn metric_card(title: &str, value: &str, accent: Color) -> tgui::Element<()> {
    Column::new()
        .padding(Insets::all(dp(16.0)))
        .gap(dp(8.0))
        .background(Color::hexa(0x132235FF))
        .border(dp(1.0), Color::hexa(0x27425FFF))
        .border_radius(dp(16.0))
        .child(
            Text::new(title)
                .font_size(sp(15.0))
                .color(Color::hexa(0xB8C7D9FF)),
        )
        .child(
            Text::new(value)
                .font_size(sp(28.0))
                .font_weight(FontWeight::SEMIBOLD)
                .color(accent),
        )
        .into()
}

fn sidebar() -> Column<()> {
    Column::new()
        .padding(Insets::all(dp(18.0)))
        .gap(dp(14.0))
        .background(Color::hexa(0x0E1826FF))
        .border(dp(1.0), Color::hexa(0x223B58FF))
        .border_radius(dp(18.0))
        .child(
            Text::new("Signals")
                .font_size(sp(22.0))
                .color(Color::hexa(0xF8FAFCFF)),
        )
        .child(
            Flex::new(Axis::Horizontal)
                .gap(dp(10.0))
                .wrap(Wrap::Wrap)
                .child(signal("Design"))
                .child(signal("Quality"))
                .child(signal("Speed"))
                .child(signal("Theming"))
                .child(signal("Layouts"))
                .child(signal("Reusable")),
        )
        .child(
            Stack::new()
                .height(dp(220.0))
                .background(Color::hexa(0x10263AFF))
                .border_radius(dp(18.0))
                .align(Align::Center)
                .child(
                    Text::new("Space for charts or activity feeds")
                        .font_size(sp(18.0))
                        .color(Color::hexa(0xD6E9FFFF)),
                ),
        )
}

fn signal(label: &str) -> tgui::Element<()> {
    Stack::new()
        .padding(Insets::symmetric(dp(12.0), dp(8.0)))
        .background(Color::hexa(0x163754FF))
        .border_radius(dp(999.0))
        .child(Text::new(label).color(Color::hexa(0xCFFAFEFF)))
        .into()
}
