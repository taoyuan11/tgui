pub mod pages;

use crate::pages::home_page::HomePage;
use crate::pages::settings_page::SettingsPage;
use std::sync::Arc;
use tgui::prelude::*;

fn root_style(ctx: &StyleContext<'_>, background: Signal<Color>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(background.into());
    style
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Settings,
}

struct RootVM {
    page: State<Page>,
    home: HomePage,
    settings: SettingsPage,
    themes: State<ThemeSet>,
    current_theme: State<ThemeMode>,
    background_color: Signal<Color>,
}

impl RootVM {
    fn theme_set(&self) -> Signal<ThemeSet> {
        self.themes.signal()
    }

    fn binding_theme(&self) -> Signal<ThemeMode> {
        self.current_theme.signal()
    }

    fn background_color(&self) -> Signal<Color> {
        self.background_color.clone()
    }

    fn toggle_theme_colors(&mut self) {
        self.themes.update(|themes| {
            let alternate = themes.light.colors.background == Color::hex(0xFF3333);
            *themes = multiple_vm_theme_set(alternate);
        });
    }

    fn show_home(&mut self) {
        self.page.set(Page::Home);
    }

    fn show_settings(&mut self) {
        self.page.set(Page::Settings);
    }
}

impl ViewModel for RootVM {
    fn new(context: &ViewModelContext) -> Self {
        let themes = context.state(multiple_vm_theme_set(false));
        let current_theme = context.state(ThemeMode::System);
        let theme_signal = themes.signal();
        let mode_signal = current_theme.signal();
        let background_color = context.signal(move || match mode_signal.get() {
            ThemeMode::Light => theme_signal.get().light.colors.background,
            ThemeMode::Dark | ThemeMode::System => theme_signal.get().dark.colors.background,
        });
        Self {
            page: context.state(Page::Home),
            home: HomePage::new(context),
            settings: SettingsPage::new(
                context,
                Some(Arc::new(|enabled| {
                    tgui_log(
                        LogLevel::Debug,
                        format_args!("Settings enabled: {}", enabled),
                    );
                })),
            ),
            themes,
            current_theme,
            background_color,
        }
    }

    fn view(&self) -> Element<Self> {
        let home = self.home.clone();
        let settings = self.settings.clone();
        let content = match self.page.get() {
            Page::Home => home.view().scope(|root: &mut Self| &mut root.home),
            Page::Settings => settings.view().scope(|root: &mut Self| &mut root.settings),
        };
        Flex::new(Axis::Vertical)
            .padding(Insets::all(dp(20.0)))
            .style_full({
                let background = self.background_color();
                move |ctx| root_style(ctx, background.clone())
            })
            .child(el![
                Text::new("根 VM：多页面应用"),
                Flex::new(Axis::Horizontal)
                    .gap(dp(10.0))
                    .padding(Insets::all(dp(10.0)))
                    .child(el![
                        Button::new("Home").on_click(Command::new(Self::show_home)),
                        Button::new("Settings").on_click(Command::new(Self::show_settings)),
                        Button::new("Change theme colors")
                            .on_click(Command::new(Self::toggle_theme_colors)),
                    ]),
            ])
            .child(content)
            .center()
            .into()
    }
}

fn multiple_vm_theme_set(alternate: bool) -> ThemeSet {
    let mut light = Theme::light();
    light.colors.background = if alternate {
        Color::hex(0xFFE066)
    } else {
        Color::hex(0xFF3333)
    };
    light.colors.primary = if alternate {
        Color::hex(0x35A853)
    } else {
        Color::hex(0xFF8A00)
    };

    let mut dark = Theme::dark();
    dark.colors.background = if alternate {
        Color::hex(0x4B0082)
    } else {
        Color::hex(0x0066FF)
    };
    dark.colors.primary = if alternate {
        Color::hex(0xE040FB)
    } else {
        Color::hex(0x00D1FF)
    };

    ThemeSet::new(light, dark)
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .with_view_model(RootVM::new)
        .root_view(RootVM::view)
        .bind_theme_set(RootVM::theme_set)
        .bind_theme_mode(RootVM::binding_theme)
        .run()
}
