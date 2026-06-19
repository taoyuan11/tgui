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
    children.push(Text::new(title).style_full(styles::title_style).into());
    children.push(
        Text::new(description)
            .style_full(|ctx| styles::muted_text_style(ctx, sp(15.0)))
            .into(),
    );
    children.extend(sections);

    Flex::vertical()
        .width(pct(100.0))
        .gap(dp(18.0))
        .padding(Insets::all(dp(24.0)))
        .child(children)
        .into()
}

pub(crate) fn virtual_page(
    title: &'static str,
    description: &'static str,
    sections: Vec<Element<App>>,
    estimate: Dp,
) -> Element<App> {
    let mut items = Vec::with_capacity(sections.len() + 2);
    items.push(
        Flex::vertical()
            .width(pct(100.0))
            .gap(dp(18.0))
            .padding(Insets::all(dp(24.0)))
            .child(Text::new(title).style_full(styles::title_style))
            .child(Text::new(description).style_full(|ctx| styles::muted_text_style(ctx, sp(15.0))))
            .into(),
    );
    items.extend(sections.into_iter().map(|section| {
        Flex::vertical()
            .width(pct(100.0))
            .padding(Insets::symmetric(dp(24.0), Dp::ZERO))
            .child(section)
            .into()
    }));
    items.push(Flex::vertical().height(dp(24.0)).into());

    VirtualList::new(items, |_index, item: &Element<App>| item.clone())
        .width(pct(100.0))
        .height(pct(100.0))
        .item_layout(ItemLayout::Measured {
            estimate,
            spacing: dp(18.0),
            overscan: 1,
        })
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
        .padding(Insets::all(dp(16.0)))
        .style_full(styles::component_card_style)
        .child(children)
        .into()
}

fn usage_demo(app: &App, demo: UsageDemo, layout: DemoLayout) -> Element<App> {
    let id = demo.id;
    let code = demo.code;

    let card = Flex::vertical().gap(dp(10.0));
    let card = match layout {
        DemoLayout::Wrapped => card.min_width(dp(340.0)).basis(dp(420.0)).grow(1.0),
        DemoLayout::Stacked => card.width(pct(100.0)),
    };

    card.padding(Insets::all(dp(12.0)))
        .style_full(styles::usage_card_style)
        .child(
            Flex::vertical()
                .gap(dp(4.0))
                .child(
                    Text::new(demo.title)
                        .style_full(styles::usage_title_style)
                        .user_select(true),
                )
                .child(Text::new(demo.description).style_full(styles::status_style)),
        )
        .child(
            Flex::vertical()
                .width(pct(100.0))
                .align(Align::Stretch)
                .padding(Insets::all(dp(14.0)))
                .style_full(styles::preview_style)
                .child(demo.preview),
        )
        .child(
            Button::new(app.code_toggle_label(id))
                .secondary()
                .on_click(Command::new(move |app: &mut App| app.toggle_code(id))),
        )
        .child(if app.code_expanded_signal(id).get() {
            code_block(code)
        } else {
            empty_code_block()
        })
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

fn empty_code_block() -> Element<App> {
    Flex::vertical().height(dp(0.0)).into()
}
