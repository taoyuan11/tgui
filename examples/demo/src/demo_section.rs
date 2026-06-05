use crate::app::App;
use crate::styles;
use tgui::prelude::*;

pub(crate) struct UsageDemo {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub preview: Element<App>,
    pub code: &'static str,
}

impl UsageDemo {
    pub(crate) fn new(
        id: &'static str,
        title: &'static str,
        description: &'static str,
        preview: impl Into<Element<App>>,
        code: &'static str,
    ) -> Self {
        Self {
            id,
            title,
            description,
            preview: preview.into(),
            code,
        }
    }
}

pub(crate) fn page(
    title: &'static str,
    description: &'static str,
    sections: Vec<Element<App>>,
) -> Element<App> {
    let mut children = Vec::new();
    children.push(Text::new(title).style(styles::title_style).into());
    children.push(
        Text::new(description)
            .style(|mode| styles::muted_text_style(mode, sp(15.0)))
            .into(),
    );
    children.extend(sections);

    Flex::vertical()
        .gap(dp(18.0))
        .padding(Insets::all(dp(24.0)))
        .child(children)
        .into()
}

pub(crate) fn component_doc(
    app: &App,
    title: &'static str,
    intro: &'static str,
    demos: Vec<UsageDemo>,
) -> Element<App> {
    let mut children = Vec::new();
    children.push(Text::new(title).style(styles::section_title_style).into());
    children.push(Text::new(intro).style(styles::status_style).into());
    for demo in demos {
        children.push(usage_demo(app, demo));
    }

    Flex::vertical()
        .gap(dp(14.0))
        .padding(Insets::all(dp(16.0)))
        .style(styles::component_card_style)
        .child(children)
        .into()
}

fn usage_demo(app: &App, demo: UsageDemo) -> Element<App> {
    let id = demo.id;
    let code = demo.code;

    Flex::vertical()
        .gap(dp(10.0))
        .padding(Insets::all(dp(12.0)))
        .style(styles::usage_card_style)
        .child(
            Flex::vertical()
                .gap(dp(4.0))
                .child(Text::new(demo.title).style(styles::usage_title_style))
                .child(Text::new(demo.description).style(styles::status_style)),
        )
        .child(
            Flex::vertical()
                .padding(Insets::all(dp(14.0)))
                .style(styles::preview_style)
                .child(demo.preview),
        )
        .child(
            Button::new(app.code_toggle_label(id))
                .secondary()
                .on_click(Command::new(move |app: &mut App| app.toggle_code(id))),
        )
        .child(app.code_expanded_signal(id).map(move |expanded| {
            if expanded {
                code_block(code)
            } else {
                empty_code_block()
            }
        }))
        .into()
}

fn code_block(code: &'static str) -> Element<App> {
    ScrollView::new()
        .height(dp(180.0))
        .overflow_x(Overflow::Scroll)
        .overflow_y(Overflow::Scroll)
        .padding(Insets::all(dp(12.0)))
        .style(styles::code_block_style)
        .child(
            Text::new(code)
                .style(styles::code_text_style)
                .user_select(true),
        )
        .into()
}

fn empty_code_block() -> Element<App> {
    Flex::vertical().height(dp(0.0)).into()
}
