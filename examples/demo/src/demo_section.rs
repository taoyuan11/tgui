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
    let mut children = Vec::with_capacity(sections.len() + 1);
    children.push(
        Flex::vertical()
            .width(pct(100.0))
            .gap(dp(6.0))
            .child(Text::new(title).style_full(styles::title_style))
            .child(Text::new(description).style_full(styles::page_description_style))
            .into(),
    );
    children.extend(sections);

    ScrollView::new()
        .width(pct(100.0))
        .height(pct(100.0))
        .overflow_x(Overflow::Hidden)
        .child(
            Flex::vertical()
                .width(pct(100.0))
                .gap(dp(20.0))
                .padding(Insets::all(dp(24.0)))
                .child(children),
        )
        .into()
}

pub(crate) fn component_doc(
    app: &App,
    title: &'static str,
    intro: &'static str,
    demos: Vec<UsageDemo>,
) -> Element<App> {
    component_doc_with_layout(app, title, intro, demos, DemoLayout::Wrapped)
}

pub(crate) fn component_doc_stacked(
    app: &App,
    title: &'static str,
    intro: &'static str,
    demos: Vec<UsageDemo>,
) -> Element<App> {
    component_doc_with_layout(app, title, intro, demos, DemoLayout::Stacked)
}

#[derive(Clone, Copy)]
enum DemoLayout {
    Wrapped,
    Stacked,
}

fn component_doc_with_layout(
    app: &App,
    title: &'static str,
    intro: &'static str,
    demos: Vec<UsageDemo>,
    layout: DemoLayout,
) -> Element<App> {
    let mut children: Vec<Element<App>> = Vec::new();
    children.push(
        Text::new(title)
            .style_full(styles::section_title_style)
            .user_select(true)
            .into(),
    );
    children.push(Text::new(intro).style_full(styles::status_style).into());
    let mut demo_cards: Vec<Element<App>> = Vec::new();
    for demo in demos {
        demo_cards.push(usage_demo(app, demo, layout));
    }
    match layout {
        DemoLayout::Wrapped => children.push(
            Flex::horizontal()
                .width(pct(100.0))
                .gap(dp(12.0))
                .wrap(Wrap::Wrap)
                .child(demo_cards)
                .into(),
        ),
        DemoLayout::Stacked => children.extend(demo_cards),
    }

    Flex::vertical()
        .width(pct(100.0))
        .gap(dp(14.0))
        .child(children)
        .into()
}

fn usage_demo(app: &App, demo: UsageDemo, layout: DemoLayout) -> Element<App> {
    let id = demo.id;
    let code = demo.code;

    let card = Card::new();
    let card = match layout {
        DemoLayout::Wrapped => card.min_width(dp(340.0)).basis(dp(420.0)).grow(1.0),
        DemoLayout::Stacked => card.width(pct(100.0)),
    };

    card.style_full(styles::usage_card_style)
        .header(
            Flex::vertical()
                .gap(dp(4.0))
                .child(
                    Text::new(demo.title)
                        .style_full(styles::usage_title_style)
                        .user_select(true),
                )
                .child(Text::new(demo.description).style_full(styles::status_style)),
        )
        .body(
            Flex::vertical()
                .width(pct(100.0))
                .align(Align::Stretch)
                .padding(Insets::all(dp(14.0)))
                .style_full(styles::preview_style)
                .child(demo.preview),
        )
        .footer(
            Flex::vertical()
                .width(pct(100.0))
                .gap(dp(8.0))
                .child(
                    Button::new(app.code_toggle_label(id))
                        .secondary()
                        .on_click(Command::new(move |app: &mut App| app.toggle_code(id))),
                )
                .child(Show::new(app.code_expanded_signal(id), code_block(code))),
        )
        .into()
}

fn code_block(code: &'static str) -> Element<App> {
    ScrollView::new()
        .width(pct(100.0))
        .max_height(dp(180.0))
        .overflow_x(Overflow::Scroll)
        .overflow_y(Overflow::Scroll)
        .padding(Insets::all(dp(12.0)))
        .style_full(styles::code_block_style)
        .child(
            Text::new(code)
                .style_full(styles::code_text_style)
                .user_select(true),
        )
        .into()
}
