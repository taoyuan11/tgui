use crate::app::App;
use crate::demo_section::{self, UsageDemo};
use crate::styles;
use tgui::prelude::*;

const CODE_TEXT_BASIC: &str = r#"Text::new("这是一段基础文本")
    .style(|style, _ctx| {
        style.typography.size = sp(16.0);
    })"#;

const CODE_TEXT_SELECTABLE: &str = r#"Text::new("这段文本允许用户选择和复制")
    .user_select(true)
    .style(|style, _ctx| {
        style.typography.size = sp(14.0);
        style.color = Color::hexa(0x64748BFF).into();
    })"#;

const CODE_RICH_TEXT: &str = r###"RichText::markdown("## Markdown\n- links\n- code")
    .on_link_click(ValueCommand::new(|app: &mut App, link| {
        app.component_status.set(format!("RichText link: {}", link.href));
    }))"###;

const CODE_BUTTON_VARIANTS: &str = r#"Flex::horizontal().gap(dp(8.0)).child(el![
    Button::new("主按钮").primary(),
    Button::new("次按钮").secondary(),
    Button::new("幽灵按钮").ghost(),
    Button::new("危险按钮").danger(),
    Button::new("禁用").disable(true),
])"#;

const CODE_BUTTON_COMMAND: &str = r#"Button::new("触发命令")
    .on_click(Command::new(|app: &mut App| {
        app.toast_status.set("按钮已点击".to_string());
    }))"#;

const CODE_SURFACE_IDENTITY: &str = r#"Flex::horizontal().gap(dp(12.0)).child(el![
    Badge::count(128u32).max(99).attach(Icon::builtin(BuiltinIcon::Info)),
    AvatarGroup::new(vec![
        Avatar::name("Ada Lovelace"),
        Avatar::name("Mika Chen"),
        Avatar::initials("NP"),
    ]),
])"#;

const CODE_CARD: &str = r#"Card::new()
    .header(Text::new("Release candidate"))
    .body(RichText::markdown("**Ready** for desktop QA."))
    .footer(Badge::text("UI").tone(BadgeTone::Primary))"#;

const CODE_DIVIDER_BASIC: &str = r#"Flex::vertical().gap(dp(8.0)).child(el![
    Text::new("上方内容"),
    Divider::new().width(dp(280.0)),
    Text::new("下方内容"),
])"#;

const CODE_DIVIDER_VARIANTS: &str = r#"Flex::vertical().gap(dp(10.0)).child(el![
    Divider::new().width(dp(280.0)).label("或"),
    Divider::new()
        .width(dp(280.0))
        .dashed(true)
        .thickness(dp(2.0))
        .color(Color::hexa(0x2563EBFF)),
])"#;

const CODE_SHADOW_SURFACE: &str = r#"Stack::new()
    .size(dp(100.0), dp(100.0))
    .style_full(shadow_showcase_style)"#;

const CODE_SHADOW_LAYOUT: &str = r#"Flex::horizontal().gap(dp(18.0)).child(el![
    Stack::new().size(dp(72.0), dp(72.0)).style_full(shadow_showcase_style),
    Stack::new().size(dp(100.0), dp(100.0)).style_full(shadow_showcase_style),
])"#;

const CODE_DISCLOSURE: &str = r#"Collapse::new("Runtime notes", Text::new("内容区域"))
    .expanded(app.collapse_open.signal().animated(Transition::ease_in_out(
        std::time::Duration::from_millis(180),
    )))
    .on_change(ValueCommand::new(|app: &mut App, open| {
        app.collapse_open.set(open);
    }))

Accordion::new(
    items,
    app.accordion_key.signal().animated(Transition::ease_in_out(
        std::time::Duration::from_millis(180),
    )),
)"#;

const CODE_SPLITTER: &str = r#"ResizablePanels::new(
    vec![Pane::new(left), Pane::new(right)],
    app.splitter_sizes.signal(),
)
.on_resize(ValueCommand::new(|app: &mut App, resize| {
    app.splitter_sizes.set(resize.sizes);
}))"#;

pub(crate) fn page(app: &App) -> Element<App> {
    demo_section::page(
        "Basics",
        "基础页面展示文本、富文本、按钮、表面元素、布局和阴影等低层组合能力。",
        vec![
            text_component(app),
            button_component(app),
            surface_component(app),
            divider_component(app),
            disclosure_layout_component(app),
            shadow_component(app),
        ],
    )
}

fn text_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Text",
        "Text 支持响应式内容、主题样式和用户选择，适合普通文案与状态展示。",
        vec![
            UsageDemo::new(
                "text/basic",
                "基础文本",
                "使用主题默认文字样式并调整字号。",
                Text::new("这是一段可直接渲染的文本组件。")
                    .style_full(|ctx| styles::text_style(ctx, sp(16.0))),
                CODE_TEXT_BASIC,
            ),
            UsageDemo::new(
                "text/selectable",
                "可选择文本",
                "开启 user_select 后，用户可以复制文本内容。",
                Text::new("这段文本允许选择和复制，适合路径、日志、诊断信息。")
                    .user_select(true)
                    .style_full(styles::status_style),
                CODE_TEXT_SELECTABLE,
            ),
            UsageDemo::new(
                "text/rich",
                "Markdown 富文本",
                "RichText 使用 Markdown 解析器渲染段落、列表、代码和链接。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    RichText::markdown(
                        "### Markdown sample\n- **Bold** text\n- Inline `code`\n- [Link action](https://example.com)\n\n```rust\nRichText::markdown(markdown)\n```",
                    )
                    .on_link_click(ValueCommand::new(|app: &mut App, link: RichTextLinkClick| {
                        app.component_status.set(format!("RichText link: {}", link.href));
                    })),
                    Text::new(app.component_status.signal()).style_full(styles::status_style),
                ]),
                CODE_RICH_TEXT,
            ),
        ],
    )
}

fn button_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Button",
        "Button 提供主次、幽灵、危险和禁用状态，并通过 Command 连接 ViewModel。",
        vec![
            UsageDemo::new(
                "button/variants",
                "按钮变体",
                "常见语义按钮可直接通过 builder 切换。",
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("主按钮").primary(),
                    Button::new("次按钮").secondary(),
                    Button::new("幽灵按钮").ghost(),
                    Button::new("危险按钮").danger(),
                    Button::new("禁用").disable(true),
                ]),
                CODE_BUTTON_VARIANTS,
            ),
            UsageDemo::new(
                "button/command",
                "命令绑定",
                "点击按钮会更新下方状态文本。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Button::new("触发命令").on_click(Command::new(|app: &mut App| {
                        app.toast_status.set("按钮命令已触发".to_string());
                    })),
                    Text::new(app.toast_status.signal()).style_full(styles::status_style),
                ]),
                CODE_BUTTON_COMMAND,
            ),
        ],
    )
}

fn surface_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Badge / Avatar / Card / Icon",
        "这些展示组件覆盖状态标识、身份缩略图、小图标和结构化内容面。",
        vec![
            UsageDemo::new(
                "surface/identity",
                "徽标、头像和图标",
                "轻量视觉组件适合密集业务界面中的状态和身份信息。",
                Flex::horizontal()
                    .gap(dp(16.0))
                    .wrap(Wrap::Wrap)
                    .align(Align::Center)
                    .child(el![
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
                        Icon::builtin(BuiltinIcon::Search).size(dp(28.0), dp(28.0)),
                    ]),
                CODE_SURFACE_IDENTITY,
            ),
            UsageDemo::new(
                "surface/card",
                "Card 与内容组合",
                "Card 可以承载 header / body / footer，内部可组合任意内容。",
                Card::new()
                    .width(dp(360.0))
                    .header(Text::new("Release candidate").style_full(styles::usage_title_style))
                    .body(RichText::markdown(
                        "**Card** 已接入公开 API，支持徽标、富文本和组合布局。",
                    ))
                    .footer(Badge::text("UI").tone(BadgeTone::Primary)),
                CODE_CARD,
            ),
        ],
    )
}

fn divider_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Divider",
        "Divider 用于分隔内容区，可添加标签、虚线样式和垂直方向。",
        vec![
            UsageDemo::new(
                "divider/basic",
                "基础分隔",
                "水平分隔线保持内容层次清晰。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Text::new("上方内容").style_full(styles::status_style),
                    Divider::new().width(dp(280.0)),
                    Text::new("下方内容").style_full(styles::status_style),
                ]),
                CODE_DIVIDER_BASIC,
            ),
            UsageDemo::new(
                "divider/variants",
                "标签和虚线",
                "同一组件可以表达分组、替代路径或轻量视觉边界。",
                Flex::vertical().gap(dp(10.0)).child(el![
                    Divider::new().width(dp(280.0)).label("或"),
                    Divider::new()
                        .width(dp(280.0))
                        .dashed(true)
                        .thickness(dp(2.0))
                        .color(Color::hexa(0x2563EBFF)),
                    Flex::horizontal().gap(dp(12.0)).child(el![
                        Text::new("左侧").style_full(styles::status_style),
                        Divider::new().vertical().height(dp(24.0)),
                        Text::new("右侧").style_full(styles::status_style),
                    ]),
                ]),
                CODE_DIVIDER_VARIANTS,
            ),
        ],
    )
}

fn disclosure_layout_component(app: &App) -> Element<App> {
    let disclosure_transition = Transition::ease_in_out(std::time::Duration::from_millis(180));

    demo_section::component_doc(
        app,
        "Disclosure / Resizable Layout",
        "Collapse、Accordion 和 Splitter 采用受控值 + 回调更新模式。",
        vec![
            UsageDemo::new(
                "layout/disclosure",
                "折叠与手风琴",
                "点击标题切换当前展开项。",
                Flex::vertical().gap(dp(10.0)).child(el![
                    Collapse::new(
                        "Runtime notes",
                        Text::new("Collapse 内容由调用方提供，可以放文本、表单或列表。")
                            .style_full(styles::status_style),
                    )
                    .expanded(app.collapse_open.signal().animated(disclosure_transition))
                    .on_change(ValueCommand::new(|app: &mut App, open| {
                        app.collapse_open.set(open);
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
                        app.accordion_key.signal().animated(disclosure_transition),
                    )
                    .on_change(ValueCommand::new(|app: &mut App, key| {
                        app.accordion_key.set(key);
                    })),
                ]),
                CODE_DISCLOSURE,
            ),
            UsageDemo::new(
                "layout/splitter",
                "ResizablePanels",
                "点击分隔条按步长调整尺寸，双击恢复均分。",
                ResizablePanels::new(
                    vec![
                        Pane::new(panel("Navigation pane", 0xE0F2FEFF)),
                        Pane::new(panel("Detail pane", 0xDCFCE7FF)),
                    ],
                    app.splitter_sizes.signal(),
                )
                .height(dp(150.0))
                .on_resize(ValueCommand::new(
                    |app: &mut App, resize: SplitterResize| {
                        app.splitter_sizes.set(resize.sizes);
                    },
                )),
                CODE_SPLITTER,
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

fn shadow_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Shadow",
        "容器 surface 可以配置阴影，用于浮层、卡片或强调面。",
        vec![
            UsageDemo::new(
                "shadow/surface",
                "单个阴影面",
                "圆形 surface 展示阴影半径、模糊和偏移。",
                Stack::new().size(dp(180.0), dp(150.0)).center().child(
                    Stack::new()
                        .size(dp(100.0), dp(100.0))
                        .style_full(styles::shadow_showcase_style),
                ),
                CODE_SHADOW_SURFACE,
            ),
            UsageDemo::new(
                "shadow/layout",
                "多尺寸组合",
                "不同尺寸的阴影元素可并排用于视觉对比。",
                Flex::horizontal().gap(dp(18.0)).child(el![
                    Stack::new()
                        .size(dp(72.0), dp(72.0))
                        .style_full(styles::shadow_showcase_style),
                    Stack::new()
                        .size(dp(100.0), dp(100.0))
                        .style_full(styles::shadow_showcase_style),
                ]),
                CODE_SHADOW_LAYOUT,
            ),
        ],
    )
}
