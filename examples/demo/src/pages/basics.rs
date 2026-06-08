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

pub(crate) fn page(app: &App) -> Element<App> {
    demo_section::page(
        "Basics",
        "基础页面展示文本、按钮、分隔线和阴影等低层组合能力。",
        vec![
            text_component(app),
            button_component(app),
            divider_component(app),
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
