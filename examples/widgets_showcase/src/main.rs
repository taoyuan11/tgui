use tgui::{
    Align, Application, Axis, Binding, Button, Color, Column, Command, Flex, Grid, Input, Insets,
    Observable, Point, Row, Stack, Text, TguiError, ValueCommand, ViewModelContext, Wrap,
};

struct WidgetsVm {
    clicks: Observable<u32>,
    draft: Observable<String>,
    cursor: Observable<String>,
}

impl WidgetsVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            clicks: ctx.observable(0),
            draft: ctx.observable("Ship a polished widgets page".to_string()),
            cursor: ctx.observable("Move over the hero card".to_string()),
        }
    }

    fn title(&self) -> Binding<String> {
        let clicks = self.clicks.clone();
        Binding::new(move || format!("tgui widgets showcase - clicks: {}", clicks.get()))
    }

    fn summary(&self) -> Binding<String> {
        let clicks = self.clicks.clone();
        let draft = self.draft.clone();
        let cursor = self.cursor.clone();
        Binding::new(move || {
            format!(
                "Clicks: {}\nDraft: {}\nPointer: {}",
                clicks.get(),
                draft.get(),
                cursor.get()
            )
        })
    }

    fn increment(&mut self) {
        self.clicks.update(|clicks| *clicks += 1);
    }

    fn reset(&mut self) {
        self.clicks.set(0);
    }

    fn set_draft(&mut self, value: String) {
        self.draft.set(value);
    }

    fn remember_pointer(&mut self, point: Point) {
        self.cursor
            .set(format!("x: {:.0}, y: {:.0}", point.x, point.y));
    }

    fn view(&self) -> tgui::Element<Self> {
        Column::new()
            .fill_size()
            .padding(Insets::all(24.0))
            .gap(18.0)
            .background(Color::hexa(0x0F172AFF))
            .child(
                Stack::new()
                    .height(180.0)
                    .padding(Insets::all(24.0))
                    .background(Color::hexa(0x1D4ED8FF))
                    .border_radius(20.0)
                    .on_mouse_move(ValueCommand::new(Self::remember_pointer))
                    .child(
                        Column::new()
                            .gap(12.0)
                            .child(
                                Text::new("Widgets showcase")
                                    .font_size(30.0)
                                    .color(Color::hexa(0xF8FAFCFF)),
                            )
                            .child(
                                Text::new("This example mixes text, buttons, input, layout containers, and pointer events in one compact screen.")
                                    .font_size(16.0)
                                    .color(Color::hexa(0xDBEAFEFF)),
                            ),
                    ),
            )
            .child(
                Row::new()
                    .gap(18.0)
                    .child(
                        Column::new()
                            .grow(1.0)
                            .padding(Insets::all(18.0))
                            .gap(12.0)
                            .background(Color::hexa(0x111827FF))
                            .border(1.0, Color::hexa(0x334155FF))
                            .border_radius(16.0)
                            .child(
                                Text::new("Interactive widgets")
                                    .font_size(20.0)
                                    .color(Color::hexa(0xF8FAFCFF)),
                            )
                            .child(
                                Input::new(Text::new(self.draft.binding()))
                                    .fill_width()
                                    .background(Color::hexa(0x1E293BFF))
                                    .border(1.0, Color::hexa(0x475569FF))
                                    .border_radius(12.0)
                                    .placeholder_with_str("Write a short task")
                                    .on_change(ValueCommand::new(Self::set_draft)),
                            )
                            .child(
                                Row::new()
                                    .gap(10.0)
                                    .child(
                                        Button::new(Text::new("Click me"))
                                            .grow(1.0)
                                            .background(Color::hexa(0x0F766EFF))
                                            .border_radius(12.0)
                                            .on_click(Command::new(Self::increment)),
                                    )
                                    .child(
                                        Button::new(Text::new("Reset"))
                                            .grow(1.0)
                                            .background(Color::hexa(0x7C2D12FF))
                                            .border_radius(12.0)
                                            .on_click(Command::new(Self::reset)),
                                    ),
                            )
                            .child(
                                Text::new(self.summary())
                                    .padding(Insets::all(14.0))
                                    .background(Color::hexa(0x0B1120FF))
                                    .border(1.0, Color::hexa(0x1D4ED8FF))
                                    .border_radius(12.0)
                                    .color(Color::hexa(0xDBEAFEFF)),
                            ),
                    )
                    .child(
                        Column::new()
                            .grow(1.0)
                            .padding(Insets::all(18.0))
                            .gap(12.0)
                            .background(Color::hexa(0x111827FF))
                            .border(1.0, Color::hexa(0x334155FF))
                            .border_radius(16.0)
                            .child(
                                Text::new("Container widgets")
                                    .font_size(20.0)
                                    .color(Color::hexa(0xF8FAFCFF)),
                            )
                            .child(
                                Grid::new(2)
                                    .gap(10.0)
                                    .child(stat_card("Buttons", "Action surfaces"))
                                    .child(stat_card("Input", "Editable state"))
                                    .child(stat_card("Text", "Read-only content"))
                                    .child(stat_card("Stack", "Overlay layouts")),
                            )
                            .child(
                                Flex::new(Axis::Horizontal)
                                    .gap(10.0)
                                    .wrap(Wrap::Wrap)
                                    .child(tag("Observable"))
                                    .child(tag("Binding"))
                                    .child(tag("Command"))
                                    .child(tag("ValueCommand"))
                                    .child(tag("Pointer"))
                                    .child(tag("Layout")),
                            ),
                    ),
            )
            .into()
    }
}

fn stat_card(title: &str, subtitle: &str) -> tgui::Element<WidgetsVm> {
    Stack::new()
        .height(88.0)
        .padding(Insets::all(14.0))
        .background(Color::hexa(0x1E293BFF))
        .border_radius(14.0)
        .child(
            Column::new()
                .gap(6.0)
                .align(Align::Start)
                .child(Text::new(title).font_size(16.0).color(Color::WHITE))
                .child(
                    Text::new(subtitle)
                        .font_size(13.0)
                        .color(Color::hexa(0xCBD5E1FF)),
                ),
        )
        .into()
}

fn tag(label: &str) -> tgui::Element<WidgetsVm> {
    Stack::new()
        .padding(Insets::symmetric(12.0, 8.0))
        .background(Color::hexa(0x1D4ED8FF))
        .border_radius(999.0)
        .child(Text::new(label).color(Color::WHITE))
        .into()
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .window_size(1120, 820)
        .with_view_model(WidgetsVm::new)
        .bind_title(WidgetsVm::title)
        .root_view(WidgetsVm::view)
        .run()
}
