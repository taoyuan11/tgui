use tgui::{
    Align, Application, Axis, Button, Column, Flex, FontWeight, Grid, Insets, Justify, Row, Stack,
    Text, Theme, Wrap,
};

fn main() -> Result<(), tgui::TguiError> {
    let mut theme = Theme::default();
    theme.palette.window_background = wgpu::Color {
        r: 0.06,
        g: 0.08,
        b: 0.10,
        a: 1.0,
    };
    theme.palette.surface = wgpu::Color {
        r: 0.12,
        g: 0.15,
        b: 0.18,
        a: 1.0,
    };
    theme.palette.surface_muted = wgpu::Color {
        r: 0.17,
        g: 0.20,
        b: 0.25,
        a: 1.0,
    };
    theme.palette.accent = wgpu::Color {
        r: 0.13,
        g: 0.49,
        b: 0.58,
        a: 1.0,
    };
    theme.typography.font_size = 17.0;

    Application::new()
        .window_size(1080, 720)
        .theme(theme)
        .with_view_model(|_| ())
        .root_view(|_| {
            Column::new()
                .background(wgpu::Color {
                    r: 0.10,
                    g: 0.12,
                    b: 0.15,
                    a: 1.0,
                })
                .padding(Insets::all(20.0))
                .gap(16.0)
                .child(
                    Text::new("Milestone 4 Layout + Theme".to_string())
                        .font_size(22.0)
                        .font_weight(FontWeight::SEMIBOLD)
                        .background(wgpu::Color {
                            r: 0.16,
                            g: 0.20,
                            b: 0.25,
                            a: 1.0,
                        }),
                )
                .child(
                    Row::new()
                        .gap(12.0)
                        .align(Align::Stretch)
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
                            .background(wgpu::Color {
                                r: 0.28,
                                g: 0.36,
                                b: 0.46,
                                a: 1.0,
                            }),
                        )
                        .child(
                            Stack::new()
                                .width(220.0)
                                .height(96.0)
                                .background(wgpu::Color {
                                    r: 0.21,
                                    g: 0.17,
                                    b: 0.13,
                                    a: 1.0,
                                })
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
        .background(wgpu::Color {
            r: 0.16,
            g: 0.18,
            b: 0.23,
            a: 1.0,
        })
        .align(Align::Center)
        .justify(Justify::Center)
        .child(Text::new(label.to_string()).font_weight(FontWeight::MEDIUM))
        .into()
}

fn chip(label: &str) -> tgui::Element<()> {
    Button::new(Text::new(label.to_string()).font_size(15.0))
    .size(128.0, 42.0)
    .background(wgpu::Color {
        r: 0.26,
        g: 0.22,
        b: 0.16,
        a: 1.0,
    })
    .into()
}
