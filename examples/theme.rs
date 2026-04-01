use tgui::{
    children, Align, Application, Binding, Button, Column, Command, Element, Insets, Row, Text,
    ThemeMode, ViewModelContext,
};

struct ThemeDemo {
    mode: tgui::Observable<ThemeMode>,
}

impl ThemeDemo {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            mode: context.observable(ThemeMode::System),
        }
    }

    fn title(&self) -> Binding<String> {
        self.mode
            .binding()
            .map(|mode| format!("theme demo - {}", mode_label(mode)))
    }

    fn theme_mode(&self) -> Binding<ThemeMode> {
        self.mode.binding()
    }

    fn set_theme(&mut self, mode: ThemeMode) {
        self.mode.set(mode);
    }

    fn view(&self) -> Element<Self> {
        let current_mode = self
            .mode
            .binding()
            .map(|mode| format!("Current mode: {}", mode_label(mode)));

        Column::new()
            .fill_size()
            .padding(Insets::all(24.0))
            .gap(20.0)
            .align(Align::Center)
            .child(children![
                Text::new("Theme Switcher".to_string()).font_size(30.0),
                Text::new(current_mode).font_size(18.0),
                Row::new().gap(12.0).child(children![
                    Button::new(Text::new("light".to_string())).on_click(Command::new(
                        |app: &mut ThemeDemo| { app.set_theme(ThemeMode::Light) }
                    )),
                    Button::new(Text::new("dark".to_string())).on_click(Command::new(
                        |app: &mut ThemeDemo| { app.set_theme(ThemeMode::Dark) }
                    )),
                    Button::new(Text::new("system".to_string())).on_click(Command::new(
                        |app: &mut ThemeDemo| { app.set_theme(ThemeMode::System) }
                    )),
                ]),
            ])
            .into()
    }
}

fn mode_label(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
        ThemeMode::System => "system",
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .window_size(960, 640)
        .with_view_model(ThemeDemo::new)
        .bind_title(ThemeDemo::title)
        .bind_theme_mode(ThemeDemo::theme_mode)
        .root_view(ThemeDemo::view)
        .run()
}
