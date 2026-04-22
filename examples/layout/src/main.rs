use tgui::{
    Align, Application, Axis, Color, Column, Flex, Grid, Insets, Row, Stack, Text, TguiError, Wrap,
    dp, sp,
};

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("tgui layout overview")
        .window_size(dp(1100.0), dp(760.0))
        .with_view_model(|_| ())
        .root_view(|_| {
            Column::new()
                .fill_size()
                .padding(Insets::all(dp(24.0)))
                .gap(dp(18.0))
                .background(Color::hexa(0x0F172AFF))
                .child(
                    Text::new("Layout containers")
                        .font_size(sp(28.0))
                        .color(Color::hexa(0xF8FAFCFF)),
                )
                .child(
                    Text::new("This example shows how Row, Column, Grid, Flex, and Stack can be combined into one responsive page.")
                        .font_size(sp(15.0))
                        .color(Color::hexa(0xCBD5E1FF)),
                )
                .child(
                    Row::new()
                        .gap(dp(18.0))
                        .child(row_panel().grow(1.0))
                        .child(column_panel().grow(1.0)),
                )
                .child(
                    Row::new()
                        .gap(dp(18.0))
                        .child(grid_panel().grow(1.0))
                        .child(flex_panel().grow(1.0)),
                )
                .child(stack_panel())
                .into()
        })
        .run()
}

fn panel(title: &str, subtitle: &str) -> Column<()> {
    Column::new()
        .padding(Insets::all(dp(18.0)))
        .gap(dp(12.0))
        .background(Color::hexa(0x111827FF))
        .border(dp(1.0), Color::hexa(0x334155FF))
        .border_radius(dp(16.0))
        .child(
            Text::new(title)
                .font_size(sp(20.0))
                .color(Color::hexa(0xF8FAFCFF)),
        )
        .child(
            Text::new(subtitle)
                .font_size(sp(14.0))
                .color(Color::hexa(0x94A3B8FF)),
        )
}

fn block(label: &str, color: Color) -> Stack<()> {
    Stack::new()
        .height(dp(56.0))
        .background(color)
        .border_radius(dp(12.0))
        .align(Align::Center)
        .child(Text::new(label).color(Color::WHITE))
}

fn chip(label: &str) -> Stack<()> {
    Stack::new()
        .padding(Insets::symmetric(dp(14.0), dp(10.0)))
        .background(Color::hexa(0x1D4ED8FF))
        .border_radius(dp(999.0))
        .child(Text::new(label).color(Color::WHITE))
}

fn row_panel() -> Column<()> {
    panel("Row", "Horizontal layout with shared spacing.").child(
        Row::new()
            .gap(dp(10.0))
            .child(block("Left", Color::hexa(0x0F766EFF)).grow(1.0))
            .child(block("Center", Color::hexa(0x0369A1FF)).grow(1.0))
            .child(block("Right", Color::hexa(0x7C3AEDFF)).grow(1.0)),
    )
}

fn column_panel() -> Column<()> {
    panel(
        "Column",
        "Vertical stacking is great for forms and dashboards.",
    )
    .child(block("Header", Color::hexa(0x1D4ED8FF)))
    .child(block("Body", Color::hexa(0x334155FF)))
    .child(block("Footer", Color::hexa(0x475569FF)))
}

fn grid_panel() -> Column<()> {
    panel(
        "Grid",
        "Regular cells work well for galleries and analytics.",
    )
    .child(
        Grid::new(3)
            .gap(dp(10.0))
            .child(block("A1", Color::hexa(0x1E3A8AFF)))
            .child(block("A2", Color::hexa(0x1D4ED8FF)))
            .child(block("A3", Color::hexa(0x2563EBFF)))
            .child(block("B1", Color::hexa(0x0F766EFF)))
            .child(block("B2", Color::hexa(0x0891B2FF)))
            .child(block("B3", Color::hexa(0x7C3AEDFF))),
    )
}

fn flex_panel() -> Column<()> {
    panel(
        "Flex + Wrap",
        "Wrap long chip lists without building a manual grid.",
    )
    .child(
        Flex::new(Axis::Horizontal)
            .gap(dp(10.0))
            .wrap(Wrap::Wrap)
            .child(chip("Search"))
            .child(chip("Billing"))
            .child(chip("Settings"))
            .child(chip("Workspace"))
            .child(chip("Integrations"))
            .child(chip("Automation"))
            .child(chip("Release notes"))
            .child(chip("Support")),
    )
}

fn stack_panel() -> Column<()> {
    panel("Stack", "Overlay a badge on top of a base surface.").child(
        Stack::new()
            .height(dp(120.0))
            .background(Color::hexa(0x172554FF))
            .border_radius(dp(16.0))
            .child(
                Stack::new()
                    .width(dp(160.0))
                    .height(dp(40.0))
                    .margin(Insets::all(dp(12.0)))
                    .background(Color::hexa(0xF97316FF))
                    .border_radius(dp(999.0))
                    .align(Align::Center)
                    .child(Text::new("Overlay badge").color(Color::WHITE)),
            )
            .align(Align::Center)
            .child(
                Text::new("Base layer")
                    .font_size(sp(18.0))
                    .color(Color::hexa(0xDBEAFEFF)),
            ),
    )
}
