pub mod pages;

use crate::pages::home_page::HomePage;
use crate::pages::settings_page::SettingsPage;
use std::sync::Arc;
use tgui::{dp, el, tgui_log, Application, Axis, Binding, Button, Color, Command, Element, Flex, Insets, LogLevel, Observable, Text, TguiError, Theme, ThemeMode, ThemeSet, ViewModel, ViewModelContext};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Settings,
}

struct RootVM {
    page: Observable<Page>,
    home: HomePage,
    settings: SettingsPage,
    themes: Observable<ThemeSet>,
    current_theme: Observable<ThemeMode>,
}

impl RootVM {
    

    fn theme_set(&self) -> Binding<ThemeSet> {
        self.themes.binding()
    }

    fn binding_theme(&self) -> Binding<ThemeMode> {
        self.current_theme.binding()
    }

    fn background_color(&self) -> Binding<Color> {
        let themes = self.themes.binding();
        let mode = self.current_theme.binding();
        Binding::new(move || match mode.get() {
            ThemeMode::Light => themes.get().light.colors.background,
            ThemeMode::Dark | ThemeMode::System => themes.get().dark.colors.background,
        })
    }

    fn accent_color(&self) -> Binding<Color> {
        let themes = self.themes.binding();
        let mode = self.current_theme.binding();
        Binding::new(move || match mode.get() {
            ThemeMode::Light => themes.get().light.colors.primary,
            ThemeMode::Dark | ThemeMode::System => themes.get().dark.colors.primary,
        })
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
        Self {
            page: context.observable(Page::Home),
            home: HomePage::new(context),
            settings: SettingsPage::new(context, Some(Arc::new(|enabled| {
                tgui_log(LogLevel::Debug, format!("Settings enabled: {}", enabled));
            }))),
            themes: context.observable(multiple_vm_theme_set(false)),
            current_theme: context.observable(ThemeMode::System),
        }
    }

    fn view(&self) -> Element<Self> {
        let page = self.page.binding();
        let home = self.home.clone();
        let settings = self.settings.clone();
        Flex::new(Axis::Vertical)
            .padding(Insets::all(dp(20.0)))
            .background(self.background_color())
            .child(el![
                Text::new("根 VM：多页面应用"),
                Flex::new(Axis::Horizontal)
                    .gap(dp(10.0))
                    .padding(Insets::all(dp(10.0)))
                    .child(el![
                        Button::new(Text::new("Home")).on_click(Command::new(Self::show_home)),
                        Button::new(Text::new("Settings"))
                            .on_click(Command::new(Self::show_settings)),
                        Button::new(Text::new("Change theme colors"))
                            .on_click(Command::new(Self::toggle_theme_colors)),
                    ]),
            ])
            .child(page.map(move |page| match page {
                Page::Home => home.view().scope(|root: &mut Self| &mut root.home),
                Page::Settings => {
                    settings.view().scope(|root: &mut Self| &mut root.settings)
                }
            }))
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
