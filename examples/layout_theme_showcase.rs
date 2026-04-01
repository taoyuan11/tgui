use tgui::{
    Align, Application, Axis, Button, Color, Column, Flex, FontWeight, Grid, Insets, Justify,
    Row, Stack, Text, Theme, Wrap,
};

fn main() -> Result<(), tgui::TguiError> {
    let mut theme = Theme::default();
    theme.palette.window_background = Color::hexa(0x0F141A00);
    theme.palette.surface = Color::hex(0x1F262E);
    theme.palette.surface_muted = Color::hex(0x2B3340);
    theme.palette.accent = Color::hex(0x217D94);
    theme.typography.font_size = 17.0;

    Application::new()
        .window_size(1080, 720)
        .theme(theme)
        .with_view_model(|_| ())
        .root_view(|_| {
            Column::new()
                .align(Align::Center)
                .fill_size()
                .background(Color::hex(0xFF1F26))
                .padding(Insets::all(20.0))
                .gap(16.0)
                .child(
                    Text::new("Milestone 4 Layout + Theme".to_string())
                        .font_size(22.0)
                        .font_weight(FontWeight::SEMIBOLD)
                        .background(Color::hex(0x293340)),
                )
                .child(
                    Row::new()
                        .fill_width()
                        .gap(12.0)
                        .align(Align::End)
                        .child(
                            Button::new(
                                Text::new("Primary".to_string())
                                    .font_size(18.0)
                                    .font_weight(FontWeight::MEDIUM),
                            )
                            .grow(1.0),
                        )
                        .child(
                            Button::new(Text::new("Secondary".to_string()).font_size(18.0))
                            .grow(1.0)
                            .background(Color::hex(0x475C75)),
                        )
                        .child(
                            Stack::new()
                                .width(220.0)
                                .height(96.0)
                                .background(Color::hex(0x362B21))
                                .align(Align::Center)
                                .child(
                                    Text::new("Stack".to_string())
                                        .font_weight(FontWeight::SEMIBOLD),
                                ),
                        ),
                )
                .child(
                    Row::new()
                        .child(
                            Grid::new(3)
                                .gap(12.0)
                                .child(card("Grid A"))
                                .child(card("Grid B"))
                                .child(card("Grid C"))
                                .child(card("Grid D"))
                                .child(card("Grid E"))
                                .child(card("Grid F"))
                        )
                        .child(
                            Flex::new(Axis::Vertical)
                                .gap(10.0)
                                .wrap(Wrap::Wrap)
                                .child(chip("Flex"))
                                .child(chip("Wrap"))
                                .child(chip("Spacing"))
                                .child(chip("Theme"))
                                .child(chip("Typography"))
                                .child(chip("Fallback"))
                                .child(chip("Grid"))
                                .child(chip("Row"))
                                .child(chip("Column")),
                        )
                )
                .into()
        })
        .run()
}

fn card(label: &str) -> tgui::Element<()> {
    Stack::new()
        .height(88.0)
        .background(Color::hex(0x29303B))
        .align(Align::Center)
        .justify(Justify::Center)
        .child(Text::new(label.to_string()).font_weight(FontWeight::MEDIUM))
        .into()
}

fn chip(label: &str) -> tgui::Element<()> {
    Button::new(Text::new(label.to_string()).font_size(15.0))
    .size(128.0, 42.0)
    .background(Color::hex(0x423828))
    .into()
}
