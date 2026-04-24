use crate::ShowcaseVm;
use tgui::{
    dp, sp, Align, Axis, Binding, Button, Color, Column, Command, Flex, Grid, Input, Insets, Row,
    Stack, Text, ValueCommand, Wrap,
};

pub(crate) fn view(vm: &ShowcaseVm) -> tgui::Element<ShowcaseVm> {
    Column::new()
        .fill_width()
        .gap(dp(18.0))
        .child(
            Stack::new()
                .padding(Insets::all(dp(20.0)))
                .background(Color::hexa(0x123552FF))
                .border_radius(dp(20.0))
                .child(
                    Column::new()
                        .gap(dp(10.0))
                        .child(
                            Text::new("Page 1: basic widgets")
                                .font_size(sp(26.0))
                                .color(Color::WHITE),
                        )
                        .child(
                            Text::new(
                                "This page collects the common building blocks you will usually reach for first: text, button, input, and layout containers.",
                            )
                            .font_size(sp(15.0))
                            .color(Color::hexa(0xD6EFFF)),
                        ),
                ),
        )
        .child(
            Row::new()
                .gap(dp(18.0))
                .child(Stack::new().grow(1.2).child(interactive_panel(vm)))
                .child(Stack::new().grow(1.0).child(layout_panel())),
        )
        .child(
            Column::new()
                .gap(dp(12.0))
                .child(
                    Text::new("Layout snippets")
                        .font_size(sp(20.0))
                        .color(Color::hexa(0xEAF6FFFF)),
                )
                .child(
                    Grid::new(3)
                        .gap(dp(12.0))
                        .child(layout_card("Row", "Line up content horizontally."))
                        .child(layout_card("Column", "Stack content vertically."))
                        .child(layout_card("Stack", "Overlay elements in one region."))
                        .child(layout_card("Grid", "Make dashboard-style matrices."))
                        .child(layout_card("Flex", "Wrap badges or fluid groups."))
                        .child(layout_card("Insets", "Control padding and spacing.")),
                ),
        )
        .into()
}

fn interactive_panel(vm: &ShowcaseVm) -> tgui::Element<ShowcaseVm> {
    let clicks = vm.clicks.clone();
    let draft = vm.draft.clone();
    let summary = Binding::new(move || {
        format!(
            "Clicks: {}\nDraft: {}\nWidgets here are backed by Observable + Binding.",
            clicks.get(),
            draft.get()
        )
    });

    Column::new()
        .padding(Insets::all(dp(18.0)))
        .gap(dp(14.0))
        .background(Color::hexa(0x0F2439FF))
        .border(dp(1.0), Color::hexa(0x264761FF))
        .border_radius(dp(18.0))
        .child(
            Text::new("Interactive panel")
                .font_size(sp(20.0))
                .color(Color::WHITE),
        )
        .child(
            Text::new("Edit the draft or click the actions below to watch the summary update.")
                .font_size(sp(14.0))
                .color(Color::hexa(0xBCD8ECFF)),
        )
        .child(
            Input::new(Text::new(vm.draft.binding()))
                .fill_width()
                .background(Color::hexa(0x091521FF))
                .border(dp(1.0), Color::hexa(0x315977FF))
                .border_radius(dp(12.0))
                .placeholder_with_str("Describe the page you want to build")
                .on_change(ValueCommand::new(ShowcaseVm::set_draft)),
        )
        .child(
            Row::new()
                .gap(dp(10.0))
                .child(
                    Button::new(Text::new("Increment"))
                        .grow(1.0)
                        .background(Color::hexa(0x34D399FF))
                        .border_radius(dp(12.0))
                        .on_click(Command::new(ShowcaseVm::increment_clicks)),
                )
                .child(
                    Button::new(Text::new("Reset"))
                        .grow(1.0)
                        .background(Color::hexa(0xF97316FF))
                        .border_radius(dp(12.0))
                        .on_click(Command::new(ShowcaseVm::reset_clicks)),
                ),
        )
        .child(
            Text::new(summary)
                .padding(Insets::all(dp(14.0)))
                .background(Color::hexa(0x08111BFF))
                .border(dp(1.0), Color::hexa(0x315977FF))
                .border_radius(dp(14.0))
                .color(Color::hexa(0xE0F2FEFF)),
        )
        .into()
}

fn layout_panel() -> tgui::Element<ShowcaseVm> {
    Column::new()
        .padding(Insets::all(dp(18.0)))
        .gap(dp(14.0))
        .background(Color::hexa(0x0F2439FF))
        .border(dp(1.0), Color::hexa(0x264761FF))
        .border_radius(dp(18.0))
        .child(
            Text::new("Container preview")
                .font_size(sp(20.0))
                .color(Color::WHITE),
        )
        .child(
            Grid::new(2)
                .gap(dp(10.0))
                .child(stat_card("Text", "Read-only copy"))
                .child(stat_card("Button", "Action surface"))
                .child(stat_card("Input", "Editable state"))
                .child(stat_card("Grid", "Two-dimensional layout")),
        )
        .child(
            Flex::new(Axis::Horizontal)
                .gap(dp(10.0))
                .wrap(Wrap::Wrap)
                .child(chip("Observable"))
                .child(chip("Binding"))
                .child(chip("Command"))
                .child(chip("ValueCommand"))
                .child(chip("Layout"))
                .child(chip("Theme")),
        )
        .into()
}

fn stat_card(title: &str, subtitle: &str) -> tgui::Element<ShowcaseVm> {
    Stack::new()
        .height(dp(88.0))
        .padding(Insets::all(dp(14.0)))
        .background(Color::hexa(0x17324CFF))
        .border_radius(dp(14.0))
        .child(
            Column::new()
                .gap(dp(6.0))
                .align(Align::Start)
                .child(Text::new(title).font_size(sp(16.0)).color(Color::WHITE))
                .child(
                    Text::new(subtitle)
                        .font_size(sp(13.0))
                        .color(Color::hexa(0xBDD7ECFF)),
                ),
        )
        .into()
}

fn chip(label: &str) -> tgui::Element<ShowcaseVm> {
    Stack::new()
        .padding(Insets::symmetric(dp(12.0), dp(8.0)))
        .background(Color::hexa(0x1E4F74FF))
        .border_radius(dp(999.0))
        .child(Text::new(label).color(Color::hexa(0xF4FBFFFF)))
        .into()
}

fn layout_card(title: &str, subtitle: &str) -> tgui::Element<ShowcaseVm> {
    Column::new()
        .padding(Insets::all(dp(16.0)))
        .gap(dp(8.0))
        .background(Color::hexa(0x0F2439FF))
        .border(dp(1.0), Color::hexa(0x264761FF))
        .border_radius(dp(16.0))
        .child(
            Text::new(title)
                .font_size(sp(18.0))
                .color(Color::hexa(0xF5FBFFFF)),
        )
        .child(
            Text::new(subtitle)
                .font_size(sp(14.0))
                .color(Color::hexa(0xBDD7ECFF)),
        )
        .into()
}
