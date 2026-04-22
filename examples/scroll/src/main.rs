use tgui::{
    Application, Color, Column, Insets, Overflow, Row, ScrollbarStyle, Stack, Text, TguiError,
    dp, sp,
};

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("tgui overflow + scroll")
        .window_size(dp(1120.0), dp(760.0))
        .with_view_model(|_| ())
        .root_view(|_| {
            Column::new()
                .fill_size()
                .padding(Insets::all(dp(24.0)))
                .gap(dp(20.0))
                .background(Color::hexa(0x0F172AFF))
                .child(
                    Text::new("Overflow + Scroll")
                        .font_size(sp(28.0))
                        .color(Color::hexa(0xF8FAFCFF)),
                )
                .child(
                    Text::new(
                        "Both panels clip overflow by default. The left panel scrolls vertically, and the right panel scrolls in both axes. You can drag the scrollbar thumb directly, and Hold Shift while using a vertical mouse wheel to drive horizontal scrolling.",
                    )
                    .font_size(sp(15.0))
                    .color(Color::hexa(0xCBD5E1FF)),
                )
                .child(
                    Row::new()
                        .fill_height()
                        .gap(dp(20.0))
                        .child(vertical_scroll_panel().grow(1.0))
                        .child(canvas_scroll_panel().grow(1.0)),
                )
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
        .border_radius(dp(18.0))
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

fn vertical_scroll_panel() -> Column<()> {
    let mut list = Column::new()
        .height(dp(460.0))
        .padding(Insets::all(dp(16.0)))
        .gap(dp(12.0))
        .background(Color::hexa(0x020617FF))
        .border_radius(dp(16.0))
        .overflow_y(Overflow::Scroll)
        .scrollbar_style(
            ScrollbarStyle::default()
                .thickness(dp(10.0))
                .track_color(Color::hexa(0xFFFFFF1A))
                .thumb_color(Color::hexa(0x38BDF8D9))
                .hover_thumb_color(Color::hexa(0x67E8F9F2))
                .active_thumb_color(Color::hexa(0xA5F3FCFF))
                .insets(Insets::all(dp(8.0))),
        );

    for index in 0..18 {
        list = list.child(
            Stack::new()
                .height(dp(84.0))
                .background(if index % 2 == 0 {
                    Color::hexa(0x1D4ED8FF)
                } else {
                    Color::hexa(0x0F766EFF)
                })
                .border_radius(dp(14.0))
                .padding(Insets::all(dp(18.0)))
                .child(
                    Text::new(format!("Feed card {}", index + 1))
                        .font_size(sp(18.0))
                        .color(Color::WHITE),
                ),
        );
    }

    panel(
        "Vertical Scroll",
        "The content exceeds the viewport height, so the panel scrolls on the Y axis while clipping overflow.",
    )
    .child(list)
}

fn canvas_scroll_panel() -> Column<()> {
    panel(
        "Bi-Directional Scroll",
        "This panel is narrower than the canvas inside it. Trackpads can use native X/Y deltas, and regular mouse wheels can hold Shift for horizontal scrolling.",
    )
    .child(
        Stack::new()
            .height(dp(460.0))
            .padding(Insets::all(dp(16.0)))
            .background(Color::hexa(0x020617FF))
            .border_radius(dp(16.0))
            .overflow_x(Overflow::Scroll)
            .overflow_y(Overflow::Scroll)
            .scrollbar_thickness(dp(12.0))
            .scrollbar_radius(dp(6.0))
            .scrollbar_insets(Insets::all(dp(10.0)))
            .scrollbar_track_color(Color::hexa(0xFFFFFF16))
            .scrollbar_thumb_color(Color::hexa(0xF97316E0))
            .scrollbar_hover_thumb_color(Color::hexa(0xFDBA74F0))
            .scrollbar_active_thumb_color(Color::hexa(0xFFEDD5FF))
            .child(
                Stack::new()
                    .size(dp(900.0), dp(780.0))
                    .background(Color::hexa(0x172554FF))
                    .border_radius(dp(20.0))
                    .child(
                        Text::new("Large canvas")
                            .font_size(sp(22.0))
                            .color(Color::hexa(0xDBEAFEFF))
                            .offset(tgui::Point::new(dp(28.0), dp(24.0))),
                    )
                    .child(
                        Stack::new()
                            .size(dp(220.0), dp(120.0))
                            .offset(tgui::Point::new(dp(560.0), dp(140.0)))
                            .background(Color::hexa(0xF97316FF))
                            .border_radius(dp(18.0))
                            .align(tgui::Align::Center)
                            .child(Text::new("Drag the scrollbar thumb").color(Color::WHITE)),
                    )
                    .child(
                        Stack::new()
                            .size(dp(280.0), dp(160.0))
                            .offset(tgui::Point::new(dp(120.0), dp(480.0)))
                            .background(Color::hexa(0x22C55EFF))
                            .border_radius(dp(18.0))
                            .align(tgui::Align::Center)
                            .child(Text::new("Bottom-left cluster").color(Color::WHITE)),
                    )
                    .child(
                        Stack::new()
                            .size(dp(180.0), dp(180.0))
                            .offset(tgui::Point::new(dp(670.0), dp(520.0)))
                            .background(Color::hexa(0xA855F7FF))
                            .border_radius(dp(999.0))
                            .align(tgui::Align::Center)
                            .child(Text::new("Far corner").color(Color::WHITE)),
                    ),
            ),
    )
}
