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
}

pub(crate) const NAV_ITEMS: [NavigationItem; 6] = [
    NavigationItem {
        page: DemoPage::Basics,
        title: "Basics",
        description: "Text, Button, Divider, Shadow",
    },
    NavigationItem {
        page: DemoPage::Forms,
        title: "Forms",
        description: "Input, Select, Slider, Form",
    },
    NavigationItem {
        page: DemoPage::Feedback,
        title: "Feedback",
        description: "Progress, Toast, Notification",
    },
    NavigationItem {
        page: DemoPage::Overlays,
        title: "Overlays",
        description: "Tooltip, Popover, Menu, Modal, Drawer",
    },
    NavigationItem {
        page: DemoPage::Data,
        title: "Data",
        description: "Tabs, List, VirtualList, DataGrid",
    },
    NavigationItem {
        page: DemoPage::MediaCanvas,
        title: "Media & Canvas",
        description: "Image, Canvas, Audio, Video",
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
        let page = item.page;
        let button = Button::new(item.title)
            .width(pct(100.0))
            .on_click(Command::new(move |app: &mut App| app.show_page(page)));
        let button = if current == page {
            button.primary()
        } else {
            button.ghost()
        };

        items.push(
            Flex::vertical()
                .gap(dp(4.0))
                .child(button)
                .child(Text::new(item.description).style(styles::status_style))
                .into(),
        );
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
