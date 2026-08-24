//! Component pages shown in the main content area.

mod actions;
mod basics;
mod data;
mod media;
mod native;

use tgui::layout::Display;
use tgui::widget::{BuildContext, Widget, WidgetNode};
use tgui::widgets::{Container, Text};

use crate::app::GalleryModel;
use crate::layout::{fixed, scroll_column, sized};
use crate::navigation::Page;

pub fn page(
    context: &mut BuildContext,
    selected: Page,
    model: &GalleryModel,
) -> tgui::Result<WidgetNode> {
    let mut pages = Vec::with_capacity(Page::ALL.len());
    for page in Page::ALL {
        pages.push(component_page(context, page, page == selected, model)?);
    }

    Container::new()
        .with_key("component-pages")
        .with_children(pages)
        .build(context)
        .map(|node| node.with_layout_style(sized(726.0, 720.0)))
}

fn component_page(
    context: &mut BuildContext,
    page: Page,
    visible: bool,
    model: &GalleryModel,
) -> tgui::Result<WidgetNode> {
    let mut children = vec![
        fixed(
            Text::new(page.label())
                .with_key(format!("page-title-{}", page.key()))
                .build(context)?,
            690.0,
            44.0,
        ),
        fixed(
            Text::new(page.description())
                .with_key(format!("page-description-{}", page.key()))
                .build(context)?,
            690.0,
            32.0,
        ),
    ];

    children.extend(match page {
        Page::Basics => basics::build(context)?,
        Page::Actions => actions::build(context, model)?,
        Page::Media => media::build(context, model)?,
        Page::Data => data::build(context, model)?,
        Page::Native => native::build(context, model)?,
    });

    let mut style = scroll_column(726.0, 720.0, 14.0, 18.0);
    if !visible {
        style.display = Display::None;
    }
    Container::new()
        .with_key(format!("page-{}", page.key()))
        .with_children(children)
        .build(context)
        .map(|node| node.with_layout_style(style))
}

pub(super) fn usage(
    context: &mut BuildContext,
    title: &str,
    summary: &str,
    code: &str,
    example: WidgetNode,
) -> tgui::Result<WidgetNode> {
    Container::new()
        .with_key(format!("usage-{title}"))
        .with_children([
            fixed(Text::new(title).build(context)?, 660.0, 30.0),
            fixed(Text::new(summary).build(context)?, 660.0, 40.0),
            example,
            fixed(Text::new(code).build(context)?, 660.0, 56.0),
        ])
        .build(context)
        .map(|node| node.with_layout_style(scroll_column(682.0, 326.0, 8.0, 10.0)))
}
