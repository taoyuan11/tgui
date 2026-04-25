use tgui::{
    el, Align, Application, Axis, Binding, Button, Color, Command, Element, Flex, Insets,
    Observable, Stack, Text, TguiError, Theme, ThemeMode, ViewModelContext, dp, pct, sp,
};
#[cfg(target_os = "android")]
use tgui::platform::android::activity::AndroidApp;

fn themed_app() -> Application {
    let mut theme = Theme::dark();
    theme.palette.window_background = Color::hexa(0x09111EFF);
    theme.palette.surface = Color::hexa(0x132238FF);
    theme.palette.surface_muted = Color::hexa(0x1C3150FF);
    theme.palette.accent = Color::hexa(0x54A6FFFF);

    Application::new()
        .title("tgui android")
        .theme(theme)
}

struct AndroidApplication {
    current_theme: Observable<String>,
    theme: Observable<ThemeMode>
}

impl AndroidApplication {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            current_theme: context.observable("System".to_string()),
            theme: context.observable(ThemeMode::System),
        }
    }

    fn get_theme(&self) -> Binding<ThemeMode> {
        self.theme.binding()
    }

    fn set_theme(&mut self) {
        let mode = self.theme.get();
        if mode == ThemeMode::System {
            self.theme.set(ThemeMode::Light);
            self.current_theme.set("Light".to_string());
        } else if mode == ThemeMode::Light {
            self.theme.set(ThemeMode::Dark);
            self.current_theme.set("Dark".to_string());
        } else {
            self.theme.set(ThemeMode::System);
            self.current_theme.set("System".to_string());
        }
    }

    fn view(&self) -> Element<Self> {
        let title = Text::new("当前主题/CurrentTheme")
            .font_size(sp(30.0));

        let text = Text::new(self.current_theme.binding());

        let button = Button::new(
            Text::new("change theme")
        ).on_click(Command::new(Self::set_theme));

        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .align(Align::Center)
            .child(
                Flex::new(Axis::Vertical)
                    .width(pct(100.0))
                    .padding(Insets::all(dp(24.0)))
                    .gap(dp(12.0))
                    .background(Color::hex(0x0099FF))
                    .border_radius(dp(28.0))
                    .child(el![
                        title,
                        text,
                        button,
                    ])
            )
            .into()
    }
}

#[cfg(target_os = "android")]
fn run_android_entry(app: AndroidApp) -> Result<(), TguiError> {
    themed_app()
        .with_view_model(AndroidApplication::new)
        .root_view(AndroidApplication::view)
        .bind_theme_mode(AndroidApplication::get_theme)
        .run_android(app)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: AndroidApp) {
    if let Err(error) = run_android_entry(app) {
        panic!("failed to run android_basic_window: {error}");
    }
}
