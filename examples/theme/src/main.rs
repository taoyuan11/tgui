use tgui::{
    Align, Application, Binding, Button, Color, Column, Command, Input, Insets, Observable, Row,
    Stack, Text, TguiError, ThemeMode, ValueCommand, ViewModelContext,
};

struct ThemeDemoVm {
    mode: Observable<ThemeMode>,
    search: Observable<String>,
}

impl ThemeDemoVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            mode: ctx.observable(ThemeMode::System),
            search: ctx.observable("Theme preview".to_string()),
        }
    }

    fn title(&self) -> Binding<String> {
        self.mode
            .binding()
            .map(|mode| format!("tgui theme demo - {}", mode_label(mode)))
    }

    fn theme_mode(&self) -> Binding<ThemeMode> {
        self.mode.binding()
    }

    fn set_light(&mut self) {
        self.mode.set(ThemeMode::Light);
    }

    fn set_dark(&mut self) {
        self.mode.set(ThemeMode::Dark);
    }

    fn set_system(&mut self) {
        self.mode.set(ThemeMode::System);
    }

    fn set_search(&mut self, value: String) {
        self.search.set(value);
    }

    fn view(&self) -> tgui::Element<Self> {
        Column::new()
            .fill_size()
            .padding(Insets::all(24.0))
            .gap(18.0)
            .child(
                Text::new("Theme mode binding")
                    .font_size(28.0)
                    .color(Color::hexa(0xF8FAFCFF)),
            )
            .child(
                Text::new(
                    self.mode
                        .binding()
                        .map(|mode| format!("Current mode: {}", mode_label(mode))),
                )
                .font_size(16.0)
                .color(Color::hexa(0xCBD5E1FF)),
            )
            .child(
                Row::new()
                    .gap(10.0)
                    .child(
                        Button::new(Text::new("Light"))
                            .grow(1.0)
                            .border_radius(12.0)
                            .on_click(Command::new(Self::set_light)),
                    )
                    .child(
                        Button::new(Text::new("Dark"))
                            .grow(1.0)
                            .border_radius(12.0)
                            .on_click(Command::new(Self::set_dark)),
                    )
                    .child(
                        Button::new(Text::new("System"))
                            .grow(1.0)
                            .border_radius(12.0)
                            .on_click(Command::new(Self::set_system)),
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
                            .background(Color::hexa(0x0F172A88))
                            .border(1.0, Color::hexa(0x334155FF))
                            .border_radius(16.0)
                            .child(
                                Text::new("Surface preview")
                                    .font_size(20.0)
                                    .color(Color::hexa(0xF8FAFCFF)),
                            )
                            .child(
                                Text::new("Switch the runtime theme and watch the window palette animate.")
                                    .font_size(15.0)
                                    .color(Color::hexa(0xCBD5E1FF)),
                            )
                            .child(
                                Input::new(Text::new(self.search.binding()))
                                    .fill_width()
                                    .border_radius(12.0)
                                    .placeholder_with_str("Type anything here")
                                    .on_change(ValueCommand::new(Self::set_search)),
                            )
                            .child(
                                Button::new(Text::new("Sample action"))
                                    .fill_width()
                                    .border_radius(12.0),
                            ),
                    )
                    .child(
                        Stack::new()
                            .grow(1.0)
                            .padding(Insets::all(18.0))
                            .background(Color::hexa(0x111827AA))
                            .border(1.0, Color::hexa(0x475569FF))
                            .border_radius(16.0)
                            .align(Align::Center)
                            .child(
                                Text::new("Theme transitions are handled by the runtime.")
                                    .font_size(18.0)
                                    .color(Color::hexa(0xE2E8F0FF)),
                            ),
                    ),
            )
            .into()
    }
}

fn mode_label(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Light => "Light",
        ThemeMode::Dark => "Dark",
        ThemeMode::System => "System",
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .window_size(980, 700)
        .with_view_model(ThemeDemoVm::new)
        .bind_title(ThemeDemoVm::title)
        .bind_theme_mode(ThemeDemoVm::theme_mode)
        .root_view(ThemeDemoVm::view)
        .run()
}
