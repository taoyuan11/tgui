use tgui::{
    Align, Application, Axis, Button, Color, Column, Flex, FontWeight, Grid, Insets, Justify, Row,
    Stack, Text, TguiError, Theme, Wrap,
};

fn main() -> Result<(), TguiError> {
    let mut theme = Theme::dark();
    theme.palette.window_background = Color::hexa(0x081019FF);
    theme.palette.surface = Color::hexa(0x101C2BFF);
    theme.palette.surface_muted = Color::hexa(0x173047FF);
    theme.palette.accent = Color::hexa(0x4FD1C5FF);
    theme.typography.font_size = 16.0;

    Application::new()
        .title("tgui layout + theme showcase")
        .window_size(1180, 760)
        .theme(theme)
        .with_view_model(|_| ())
        .root_view(|_| {
            Column::new()
                .fill_size()
                .padding(Insets::all(24.0))
                .gap(18.0)
                .child(hero())
                .child(
                    Row::new()
                        .gap(18.0)
                        .child(metric_grid().grow(1.4))
                        .child(sidebar().grow(1.0)),
                )
                .into()
        })
        .run()
}

fn hero() -> tgui::Element<()> {
    Row::new()
        .padding(Insets::all(22.0))
        .gap(18.0)
        .background(Color::hexa(0x0F1B2BFF))
        .border(1.0, Color::hexa(0x23435FFF))
        .border_radius(22.0)
        .child(
            Column::new()
                .grow(1.0)
                .gap(12.0)
                .child(
                    Text::new("Layout + theme showcase")
                        .font_size(30.0)
                        .font_weight(FontWeight::SEMIBOLD)
                        .color(Color::hexa(0xF0FDFAFF)),
                )
                .child(
                    Text::new("A dashboard-like example that combines custom theme colors with nested layout containers.")
                        .font_size(16.0)
                        .color(Color::hexa(0xB8E6E1FF)),
                )
                .child(
                    Row::new()
                        .gap(10.0)
                        .child(
                            Button::new(Text::new("Primary action"))
                                .background(Color::hexa(0x0F766EFF))
                                .border_radius(12.0),
                        )
                        .child(
                            Button::new(Text::new("Secondary"))
                                .background(Color::hexa(0x1E3A5FFF))
                                .border_radius(12.0),
                        ),
                ),
        )
        .child(
            Stack::new()
                .width(220.0)
                .height(140.0)
                .background(Color::hexa(0x103B43FF))
                .border_radius(20.0)
                .align(Align::Center)
                .justify(Justify::Center)
                .child(
                    Text::new("Custom theme")
                        .font_size(20.0)
                        .color(Color::hexa(0xE6FFFBFF)),
                ),
        )
        .into()
}

fn metric_grid() -> Column<()> {
    Column::new()
        .padding(Insets::all(18.0))
        .gap(14.0)
        .background(Color::hexa(0x0E1826FF))
        .border(1.0, Color::hexa(0x223B58FF))
        .border_radius(18.0)
        .child(
            Text::new("Metrics")
                .font_size(22.0)
                .color(Color::hexa(0xF8FAFCFF)),
        )
        .child(
            Grid::new(2)
                .gap(12.0)
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
        .padding(Insets::all(16.0))
        .gap(8.0)
        .background(Color::hexa(0x132235FF))
        .border(1.0, Color::hexa(0x27425FFF))
        .border_radius(16.0)
        .child(
            Text::new(title)
                .font_size(15.0)
                .color(Color::hexa(0xB8C7D9FF)),
        )
        .child(
            Text::new(value)
                .font_size(28.0)
                .font_weight(FontWeight::SEMIBOLD)
                .color(accent),
        )
        .into()
}

fn sidebar() -> Column<()> {
    Column::new()
        .padding(Insets::all(18.0))
        .gap(14.0)
        .background(Color::hexa(0x0E1826FF))
        .border(1.0, Color::hexa(0x223B58FF))
        .border_radius(18.0)
        .child(
            Text::new("Signals")
                .font_size(22.0)
                .color(Color::hexa(0xF8FAFCFF)),
        )
        .child(
            Flex::new(Axis::Horizontal)
                .gap(10.0)
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
                .height(220.0)
                .background(Color::hexa(0x10263AFF))
                .border_radius(18.0)
                .align(Align::Center)
                .child(
                    Text::new("Space for charts or activity feeds")
                        .font_size(18.0)
                        .color(Color::hexa(0xD6E9FFFF)),
                ),
        )
}

fn signal(label: &str) -> tgui::Element<()> {
    Stack::new()
        .padding(Insets::symmetric(12.0, 8.0))
        .background(Color::hexa(0x163754FF))
        .border_radius(999.0)
        .child(Text::new(label).color(Color::hexa(0xCFFAFEFF)))
        .into()
}
