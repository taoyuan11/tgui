use crate::app::App;
use crate::demo_section::{self, UsageDemo};
use crate::styles;
use std::time::Duration;
use tgui::prelude::*;

const CODE_IDENTITY: &str = r#"Flex::horizontal().gap(dp(12.0)).child(el![
    Badge::count(128u32).max(99).attach(Icon::builtin(BuiltinIcon::Info)),
    AvatarGroup::new(vec![
        Avatar::name("Ada Lovelace"),
        Avatar::name("Mika Chen"),
        Avatar::initials("NP"),
    ]),
    Skeleton::lines(3),
])"#;

const CODE_CARD: &str = r#"Card::new()
    .header(Text::new("Release candidate"))
    .body(RichText::markdown("**Ready** for desktop QA."))
    .footer(Badge::text("P3").tone(BadgeTone::Primary))"#;

const CODE_NAVIGATION: &str = r#"Breadcrumb::new(vec![
    BreadcrumbItem::new("Workspace"),
    BreadcrumbItem::new("Components"),
    BreadcrumbItem::new("P3"),
])

Pagination::new(app.p3_page.signal(), 12usize)
    .page_size(app.p3_page_size.signal())
    .on_change(...)"#;

const CODE_DISCLOSURE: &str = r#"Collapse::new("Runtime notes", Text::new("内容区域"))
    .expanded(app.p3_collapse_open.signal())
    .on_change(ValueCommand::new(|app: &mut App, open| {
        app.p3_collapse_open.set(open);
    }))

Accordion::new(items, app.p3_accordion_key.signal())"#;

const CODE_SPLITTER: &str = r#"ResizablePanels::new(
    vec![Pane::new(left), Pane::new(right)],
    app.p3_splitter_sizes.signal(),
)
.on_resize(ValueCommand::new(|app: &mut App, resize| {
    app.p3_splitter_sizes.set(resize.sizes);
}))"#;

const CODE_RATING_CAROUSEL: &str = r#"Rating::new(app.p3_rating.signal())
    .half()
    .on_change(ValueCommand::new(|app: &mut App, change| {
        app.p3_rating.set(change.value);
    }))

Carousel::new(slides, app.p3_carousel.signal())
    .auto_play(Duration::from_secs(4))"#;

const CODE_COMBO_RICH: &str = r###"Combobox::new(app.p3_combobox_text.clone(), options)
    .open(app.p3_combobox_open.signal())
    .on_change(ValueCommand::new(|app: &mut App, change| {
        app.p3_combobox_selected.set(change.selected_key);
    }))

RichText::markdown("## Markdown\n- links\n- code")"###;

pub(crate) fn page(app: &App) -> Element<App> {
    demo_section::page(
        "P3 Widgets",
        "P3 页面集中展示 Badge、Avatar、Skeleton、Collapse、Splitter、Breadcrumb、Pagination、Card、Rating、Icon、RichText、Carousel 和 Combobox。",
        vec![
            identity_component(app),
            navigation_component(app),
            disclosure_component(app),
            input_component(app),
        ],
    )
}

fn identity_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Identity / Surface",
        "Badge、Avatar、Skeleton、Card 和 Icon 覆盖状态标识、占位骨架、内容卡片与小图标。",
        vec![
            UsageDemo::new(
                "p3/identity",
                "徽标、头像与骨架",
                "这些组件都是轻量组合组件，适合密集业务界面。",
                Flex::horizontal().gap(dp(16.0)).wrap(Wrap::Wrap).align(Align::Center).child(el![
                    Badge::count(128u32)
                        .max(99)
                        .tone(BadgeTone::Error)
                        .attach(Icon::builtin(BuiltinIcon::Info).size(dp(28.0), dp(28.0))),
                    Badge::text("NEW").tone(BadgeTone::Success),
                    AvatarGroup::new(vec![
                        Avatar::name("Ada Lovelace"),
                        Avatar::name("Mika Chen"),
                        Avatar::initials("NP"),
                    ])
                    .max_visible(2),
                    Flex::vertical().gap(dp(6.0)).child(el![
                        Skeleton::line().width(dp(180.0)),
                        Skeleton::lines(2),
                    ]),
                ]),
                CODE_IDENTITY,
            ),
            UsageDemo::new(
                "p3/card",
                "Card 与 RichText",
                "Card 可以承载 header / body / footer，内部可组合任意内容。",
                Card::new()
                    .width(dp(360.0))
                    .header(Text::new("Release candidate").style_full(styles::usage_title_style))
                    .body(RichText::markdown(
                        "**P3** 已接入公开 API，支持 `Markdown`、徽标和组合布局。",
                    ))
                    .footer(Badge::text("P3").tone(BadgeTone::Primary)),
                CODE_CARD,
            ),
        ],
    )
}

fn navigation_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Navigation",
        "Breadcrumb 和 Pagination 覆盖常见路径导航与分页控制。",
        vec![UsageDemo::new(
            "p3/navigation",
            "路径与分页",
            "分页事件由 ViewModel 回写当前页和 page size。",
            Flex::vertical().gap(dp(12.0)).child(el![
                Breadcrumb::new(vec![
                    BreadcrumbItem::new("Workspace").on_click(Command::new(|app: &mut App| {
                        app.p3_status.set("点击了 Workspace".to_string());
                    })),
                    BreadcrumbItem::new("Components"),
                    BreadcrumbItem::new("P3 Widgets"),
                ]),
                Pagination::new(app.p3_page.signal(), 12usize)
                    .page_size(app.p3_page_size.signal())
                    .on_change(ValueCommand::new(|app: &mut App, change: PaginationChange| {
                        app.p3_page.set(change.page);
                        app.p3_page_size.set(change.page_size);
                        app.p3_status.set(format!(
                            "分页: page={}, page_size={}",
                            change.page, change.page_size
                        ));
                    })),
                Text::new(app.p3_status.signal()).style_full(styles::status_style),
            ]),
            CODE_NAVIGATION,
        )],
    )
}

fn disclosure_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Disclosure / Layout",
        "Collapse、Accordion 和 Splitter 采用受控值 + 回调更新模式。",
        vec![
            UsageDemo::new(
                "p3/disclosure",
                "折叠与手风琴",
                "点击标题切换当前展开项。",
                Flex::vertical().gap(dp(10.0)).child(el![
                    Collapse::new(
                        "Runtime notes",
                        Text::new("Collapse 内容由调用方提供，可以放文本、表单或列表。")
                            .style_full(styles::status_style),
                    )
                    .expanded(app.p3_collapse_open.signal())
                    .on_change(ValueCommand::new(|app: &mut App, open| {
                        app.p3_collapse_open.set(open);
                    })),
                    Accordion::new(
                        vec![
                            AccordionItem::new(
                                "usage",
                                "Usage",
                                Text::new("Accordion 同一时间只展开一个 key。"),
                            ),
                            AccordionItem::new(
                                "theme",
                                "Theme",
                                Text::new("CollapseStyle 可通过 Theme components 覆盖。"),
                            ),
                        ],
                        app.p3_accordion_key.signal(),
                    )
                    .on_change(ValueCommand::new(|app: &mut App, key| {
                        app.p3_accordion_key.set(key);
                    })),
                ]),
                CODE_DISCLOSURE,
            ),
            UsageDemo::new(
                "p3/splitter",
                "ResizablePanels",
                "点击分隔条按步长调整尺寸，双击恢复均分。",
                ResizablePanels::new(
                    vec![
                        Pane::new(panel("Navigation pane", 0xE0F2FEFF)),
                        Pane::new(panel("Detail pane", 0xDCFCE7FF)),
                    ],
                    app.p3_splitter_sizes.signal(),
                )
                .height(dp(150.0))
                .on_resize(ValueCommand::new(|app: &mut App, resize: SplitterResize| {
                    app.p3_splitter_sizes.set(resize.sizes);
                })),
                CODE_SPLITTER,
            ),
        ],
    )
}

fn input_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Input / Rich Content",
        "Rating、Carousel、Combobox 和 RichText 覆盖评分、轮播、本地搜索选择与 Markdown 内容。",
        vec![
            UsageDemo::new(
                "p3/rating-carousel",
                "评分与轮播",
                "Rating 支持半星步长，Carousel 支持受控 index。",
                Flex::vertical().gap(dp(12.0)).child(el![
                    Rating::new(app.p3_rating.signal())
                        .half()
                        .on_change(ValueCommand::new(|app: &mut App, change: RatingChange| {
                            app.p3_rating.set(change.value);
                            app.p3_status.set(format!("评分: {:.1}", change.value));
                        })),
                    Carousel::new(
                        vec![
                            slide("Overview", "Badge / Avatar / Card"),
                            slide("Forms", "Combobox / Rating"),
                            slide("Content", "RichText / Carousel"),
                        ],
                        app.p3_carousel.signal(),
                    )
                    .auto_play(Duration::from_secs(4))
                    .on_change(ValueCommand::new(|app: &mut App, index| {
                        app.p3_carousel.set(index);
                    })),
                ]),
                CODE_RATING_CAROUSEL,
            ),
            UsageDemo::new(
                "p3/combo-rich",
                "Combobox 与 Markdown",
                "Combobox 仅使用本地 options 做大小写不敏感过滤。",
                Flex::vertical().gap(dp(12.0)).child(el![
                    Combobox::new(app.p3_combobox_text.clone(), combo_options())
                        .open(app.p3_combobox_open.signal())
                        .selected_key(app.p3_combobox_selected.signal())
                        .placeholder("Search component")
                        .on_open_change(ValueCommand::new(|app: &mut App, open| {
                            app.p3_combobox_open.set(open);
                        }))
                        .on_change(ValueCommand::new(|app: &mut App, change: ComboboxChange| {
                            app.p3_combobox_selected.set(change.selected_key.clone());
                            app.p3_status.set(format!("Combobox: {}", change.text));
                        })),
                    RichText::markdown(
                        "### Markdown sample\n- **Bold** text\n- Inline `code`\n- [Link action](https://example.com)\n\n```rust\nRichText::markdown(markdown)\n```",
                    )
                    .on_link_click(ValueCommand::new(|app: &mut App, link: RichTextLinkClick| {
                        app.p3_status.set(format!("RichText link: {}", link.href));
                    })),
                ]),
                CODE_COMBO_RICH,
            ),
        ],
    )
}

fn panel(title: &'static str, color: u32) -> Element<App> {
    Stack::new()
        .height(pct(100.0))
        .center()
        .style_full(move |ctx| {
            let mut style = ContainerStyle::default_for_theme(ctx.theme);
            style.surface.background = Some(Color::hexa(color).into());
            style.surface.border_radius = Some(dp(6.0).into());
            style
        })
        .child(Text::new(title).style_full(styles::status_style))
        .into()
}

fn slide(title: &'static str, subtitle: &'static str) -> Element<App> {
    Stack::new()
        .height(dp(110.0))
        .center()
        .style_full(|ctx| {
            let mut style = ContainerStyle::default_for_theme(ctx.theme);
            style.surface.background = Some(ctx.theme.colors.surface_high.into());
            style.surface.border_radius = Some(ctx.theme.radius.lg.into());
            style
        })
        .child(
            Flex::vertical()
                .gap(dp(4.0))
                .align(Align::Center)
                .child(Text::new(title).style_full(styles::usage_title_style))
                .child(Text::new(subtitle).style_full(styles::status_style)),
        )
        .into()
}

fn combo_options() -> Vec<ComboboxOption> {
    vec![
        ComboboxOption::new("badge", "Badge"),
        ComboboxOption::new("avatar", "Avatar"),
        ComboboxOption::new("rich-text", "RichText"),
        ComboboxOption::new("combobox", "Combobox"),
        ComboboxOption::new("splitter", "Splitter"),
        ComboboxOption::new("carousel", "Carousel"),
    ]
}
