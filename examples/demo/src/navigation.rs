use crate::app::App;
use crate::styles;
use tgui::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DemoPage {
    Basics,
    Forms,
    Feedback,
    Overlays,
    Data,
    MediaCanvas,
}

pub(crate) struct NavigationItem {
    pub page: DemoPage,
    pub title: &'static str,
    pub description: &'static str,
    pub badge: &'static str,
    pub accent: u32,
}

pub(crate) const NAV_ITEMS: [NavigationItem; 6] = [
    NavigationItem {
        page: DemoPage::Basics,
        title: "Basics",
        description: "Text, Button, Divider",
        badge: "B",
        accent: 0x2563EBFF,
    },
    NavigationItem {
        page: DemoPage::Forms,
        title: "Forms",
        description: "Inputs and validation",
        badge: "F",
        accent: 0x14B8A6FF,
    },
    NavigationItem {
        page: DemoPage::Feedback,
        title: "Feedback",
        description: "Toast and notification",
        badge: "!",
        accent: 0xF59E0BFF,
    },
    NavigationItem {
        page: DemoPage::Overlays,
        title: "Overlays",
        description: "Floating UI layers",
        badge: "O",
        accent: 0x8B5CF6FF,
    },
    NavigationItem {
        page: DemoPage::Data,
        title: "Data",
        description: "Lists, tabs, tables",
        badge: "D",
        accent: 0x22C55EFF,
    },
    NavigationItem {
        page: DemoPage::MediaCanvas,
        title: "Media & Canvas",
        description: "Canvas and playback",
        badge: "M",
        accent: 0xEF4444FF,
    },
];

pub(crate) fn sidebar(_app: &App, current: DemoPage) -> Element<App> {
    let mut items: Vec<Element<App>> = Vec::new();

    items.push(
        Flex::vertical()
            .gap(dp(4.0))
            .child(Text::new("TGUI Demo").style(styles::section_title_style))
            .child(Text::new("组件文档式示例").style(styles::status_style))
            .into(),
    );

    for item in NAV_ITEMS {
        items.push(nav_item(item, current));
    }

    Flex::vertical()
        .width(dp(260.0))
        .height(pct(100.0))
        .gap(dp(14.0))
        .padding(Insets::all(dp(18.0)))
        .style(styles::sidebar_style)
        .child(items)
        .into()
}

fn nav_item(item: NavigationItem, current: DemoPage) -> Element<App> {
    let page = item.page;
    let active = current == page;
    let accent = item.accent;

    Stack::new()
        .width(pct(100.0))
        .padding(Insets::symmetric(dp(12.0), dp(10.0)))
        .cursor(CursorStyle::Pointer)
        .style(move |mode| styles::nav_item_style(mode, active, accent))
        .on_click(Command::new(move |app: &mut App| app.show_page(page)))
        .child(
            Flex::horizontal()
                .gap(dp(10.0))
                .align(Align::Center)
                .child(
                    Stack::new()
                        .size(dp(34.0), dp(34.0))
                        .center()
                        .style(move |mode| styles::nav_badge_style(mode, active, accent))
                        .child(
                            Text::new(item.badge)
                                .style(move |mode| styles::nav_badge_text_style(mode, active)),
                        ),
                )
                .child(
                    Flex::vertical()
                        .grow(1.0)
                        .gap(dp(3.0))
                        .child(
                            Text::new(item.title)
                                .style(move |mode| styles::nav_title_style(mode, active)),
                        )
                        .child(
                            Text::new(item.description)
                                .style(move |mode| styles::nav_description_style(mode, active)),
                        ),
                ),
        )
        .into()
}
