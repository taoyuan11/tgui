pub mod pages;

use crate::pages::home_page::HomePage;
use crate::pages::settings_page::SettingsPage;
use std::sync::Arc;
use tgui::{dp, el, tgui_log, Application, Axis, Button, Color, Command, Element, Flex, Insets, LogLevel, Observable, Text, TguiError, ViewModelContext};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Settings,
}

struct RootVM {
    page: Observable<Page>,
    home: HomePage,
    settings: SettingsPage,
}

impl RootVM {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            page: context.observable(Page::Home),
            home: HomePage::new(context),
            settings: SettingsPage::new(context, Some(Arc::new(|enabled| {
                tgui_log(LogLevel::Debug, format!("Settings enabled: {}", enabled));
            }))),
        }
    }

    fn show_home(&mut self) {
        self.page.set(Page::Home);
    }

    fn show_settings(&mut self) {
        self.page.set(Page::Settings);
    }

    fn view(&self) -> Element<Self> {
        let page = self.page.binding();
        let home = self.home.clone();
        let settings = self.settings.clone();
        Flex::new(Axis::Vertical)
            .padding(Insets::all(dp(20.0)))
            .background(Color::hex(0x202124))
            .child(el![
                Text::new("根 VM：多页面应用"),
                Flex::new(Axis::Horizontal).gap(dp(10.0)).child(el![
                    Button::new(Text::new("Home")).on_click(Command::new(Self::show_home)),
                    Button::new(Text::new("Settings")).on_click(Command::new(Self::show_settings)),
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

fn main() -> Result<(), TguiError> {
    Application::new()
        .with_view_model(RootVM::new)
        .root_view(RootVM::view)
        .run()
}
