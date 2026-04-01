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
            .map(|mode| format!("当前模式：{}", mode_label(mode)));

        Column::new()
            .fill_size()
            .padding(Insets::all(24.0))
            .gap(20.0)
            .align(Align::Center)
            .on_click(Command::new(|_| println!("点击了Column")))
            .on_double_click(Command::new(|_| println!("on_double_click Column")))
            .on_mouse_enter(Command::new(|_| println!("mouse_enter Column")))
            .on_mouse_leave(Command::new(|_| println!("mouse_leave Column")))
            .child(children![
                // 主题色的过渡现在由运行时内建处理，这里只需要切换 ThemeMode。
                Text::new("主题切换动画".to_string()).font_size(30.0),
                Text::new(current_mode).font_size(18.0),
                Row::new().gap(12.0).child(children![
                    Button::new(Text::new("浅色".to_string())).on_click(Command::new(
                        |app: &mut ThemeDemo| { app.set_theme(ThemeMode::Light) }
                    )).on_focus(Command::new(|_| println!("focus Button1")))
                    .on_blur(Command::new(|_| println!("blur Button1"))),
                    Button::new(Text::new("深色".to_string())).on_click(Command::new(
                        |app: &mut ThemeDemo| { app.set_theme(ThemeMode::Dark) }
                    )),
                    Button::new(Text::new("跟随系统".to_string())).on_click(Command::new(
                        |app: &mut ThemeDemo| { app.set_theme(ThemeMode::System) }
                    )).border_radius(6.0),
                ]),
            ])
            .into()
    }
}

fn mode_label(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Light => "浅色",
        ThemeMode::Dark => "深色",
        ThemeMode::System => "跟随系统",
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
