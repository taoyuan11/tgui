use crate::app::App;
use crate::styles;
use tgui::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DemoPage {
    Basics,
    Icons,
    Forms,
    Feedback,
    Overlays,
    Data,
    MediaCanvas,
}

impl DemoPage {
    pub(crate) fn content_key(self) -> &'static str {
        match self {
            Self::Basics => "page-content-basics",
            Self::Icons => "page-content-icons",
            Self::Forms => "page-content-forms",
            Self::Feedback => "page-content-feedback",
            Self::Overlays => "page-content-overlays",
            Self::Data => "page-content-data",
            Self::MediaCanvas => "page-content-media-canvas",
        }
    }
}

pub(crate) struct NavigationItem {
    pub page: DemoPage,
    pub title: &'static str,
    pub description: &'static str,
    pub badge: &'static str,
}

pub(crate) const NAV_ITEMS: [NavigationItem; 7] = [
    NavigationItem {
        page: DemoPage::Forms,
        title: "Forms",
        description: "Inputs and validation",
        badge: "F",
    },
    NavigationItem {
        page: DemoPage::Basics,
        title: "Basics",
        description: "Text, Button, Divider",
        badge: "B",
    },
    NavigationItem {
        page: DemoPage::Icons,
        title: "Icons",
        description: "Common built-in icons",
        badge: "I",
    },
    NavigationItem {
        page: DemoPage::Feedback,
        title: "Feedback",
        description: "Toast and notification",
        badge: "!",
    },
    NavigationItem {
        page: DemoPage::Overlays,
        title: "Overlays",
        description: "Floating UI layers",
        badge: "O",
    },
    NavigationItem {
        page: DemoPage::Data,
        title: "Data",
        description: "Lists, tabs, tables",
        badge: "D",
    },
    NavigationItem {
        page: DemoPage::MediaCanvas,
        title: "Media & Canvas",
        description: "Canvas and playback",
        badge: "M",
    },
];

pub(crate) fn sidebar(app: &App) -> Element<App> {
    let mut items: Vec<Element<App>> = Vec::new();

    items.push(
        Flex::vertical()
            .gap(dp(2.0))
            .child(Text::new("tgui").style_full(styles::section_title_style))
            .child(Text::new("Component gallery").style_full(styles::status_style))
            .into(),
    );
    items.push(Divider::new().into());

    for item in NAV_ITEMS {
        items.push(nav_item(app, item));
    }

    Flex::vertical()
        .width(dp(248.0))
        .height(pct(100.0))
        .gap(dp(6.0))
        .padding(Insets::all(dp(20.0)))
        .style_full(styles::sidebar_style)
        .child(items)
        .into()
}

fn nav_item(app: &App, item: NavigationItem) -> Element<App> {
    let page = item.page;
    let active = app
        .current_page
        .signal()
        .map(move |current| current == page);
    let active_for_item = active.clone();
    let active_for_badge = active.clone();
    let active_for_badge_text = active.clone();
    let active_for_title = active.clone();
    let active_for_description = active;

    Stack::new()
        .width(pct(100.0))
        .padding(Insets::symmetric(dp(12.0), dp(9.0)))
        .cursor(CursorStyle::Pointer)
        .style_full(move |ctx| styles::nav_item_style(ctx, active_for_item.get()))
        .on_click(Command::new_with_context(move |app: &mut App, ctx| {
            app.show_page_with_rebuild(page, ctx);
        }))
        .child(
            Flex::horizontal()
                .gap(dp(10.0))
                .align(Align::Center)
                .child(
                    Stack::new()
                        .size(dp(32.0), dp(32.0))
                        .center()
                        .style_full(move |ctx| styles::nav_badge_style(ctx, active_for_badge.get()))
                        .child(Text::new(item.badge).style_full(move |ctx| {
                            styles::nav_badge_text_style(ctx, active_for_badge_text.get())
                        })),
                )
                .child(
                    Flex::vertical()
                        .grow(1.0)
                        .gap(dp(3.0))
                        .child(Text::new(item.title).style_full(move |ctx| {
                            styles::nav_title_style(ctx, active_for_title.get())
                        }))
                        .child(Text::new(item.description).style_full(move |ctx| {
                            styles::nav_description_style(ctx, active_for_description.get())
                        })),
                ),
        )
        .into()
}
