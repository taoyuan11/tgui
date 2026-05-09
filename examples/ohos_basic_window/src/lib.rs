#[cfg(target_env = "ohos")]
use tgui::platform::ohos::export_ohos_winit_app;
use tgui::prelude::*;
#[cfg(target_env = "ohos")]
use winit_core::application::ApplicationHandler;
use tgui::application::Application;

fn title_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.typography.size = sp(28.0);
    style
}

fn card_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.background = Some(Color::hex(0x1188DD).into());
    style.surface.border_radius = Some(dp(28.0).into());
    style
}

fn themed_app() -> Application {
    let mut theme = Theme::dark();
    theme.colors.background = Color::hexa(0x07111EFF);
    theme.colors.surface = Color::hexa(0x13263BFF);
    theme.colors.surface_low = Color::hexa(0x1A3658FF);
    theme.colors.primary = Color::hexa(0x5ED0FAFF);
    let theme_set = ThemeSet::new(theme.clone(), theme);

    Application::new()
        .title("tgui ohos")
        .theme_set(theme_set)
        .theme_mode(ThemeMode::Dark)
}

struct OhosApplication {
    current_theme: State<String>,
    theme: State<ThemeMode>,
}

impl OhosApplication {
    

    fn theme_mode(&self) -> Signal<ThemeMode> {
        self.theme.signal()
    }

    fn toggle_theme(&mut self) {
        let next = match self.theme.get() {
            ThemeMode::System => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::System,
        };
        let label = match next {
            ThemeMode::System => "System",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        };
        self.theme.set(next);
        self.current_theme.set(label.to_string());
    }

    
}

impl ViewModel for OhosApplication {

    fn new(context: &ViewModelContext) -> Self {
        Self {
            current_theme: context.state("System".to_string()),
            theme: context.state(ThemeMode::System),
        }
    }
    
    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .center()
            .child(el![
                Flex::new(Axis::Vertical)
                    .width(pct(100.0))
                    .padding(Insets::all(dp(24.0)))
                    .gap(dp(12.0))
                    .style(card_style)
                    .child(el![
                        Text::new("当前主题 / Current Theme").style(title_style),
                        Text::new(self.current_theme.signal()),
                        Button::new("toggle theme")
                            .on_click(Command::new(Self::toggle_theme)),
                    ])
            ])
            .into()
    }
}

#[cfg(target_env = "ohos")]
fn create_ohos_app() -> impl ApplicationHandler + Send {
    themed_app()
        .with_view_model(OhosApplication::new)
        .root_view(OhosApplication::view)
        .bind_theme_mode(OhosApplication::theme_mode)
        .into_ohos_handler()
}

#[cfg(target_env = "ohos")]
export_ohos_winit_app!(create_ohos_app);

#[cfg(not(target_env = "ohos"))]
pub fn host_build_placeholder() {}
