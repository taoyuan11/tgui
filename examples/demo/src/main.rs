use std::path::PathBuf;

use tgui::{dp, el, fr, pct, sp, Application, Axis, Button, Color, Element, Flex, Grid, Image, Input, Insets, Observable, Overflow, Stack, Switch, Text, TguiError, Theme, ValueCommand, ViewModel, ViewModelContext};

struct App {
    switch: Observable<bool>,
}

impl ViewModel for App {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            switch: context.observable(false),
        }
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(20.0)))
            .overflow_y(Overflow::Scroll)
            .child(
                Flex::new(Axis::Vertical)
                    .width(pct(100.0))
                    .gap(dp(14.0))
                    .child(el![
                        Text::new("TGUI 组件列表示例")
                            .font_size(sp(28.0))
                            .color(Color::WHITE),
                        component_card(
                            "Text",
                            Text::new("这是一段可直接渲染的文本组件")
                                .font_size(sp(16.0))
                                .color(Color::rgb(240, 244, 255)),
                        ),
                        component_card(
                            "Button",
                            Flex::new(Axis::Horizontal).gap(dp(10.0)).child(el![
                                Button::new(Text::new("普通按钮")).primary(),
                                Button::new(Text::new("次要按钮")).secondary(),
                                Button::new(Text::new("危险按钮")).danger(),
                                Button::new(Text::new("幽灵按钮")).ghost(),
                            ]),
                        ),
                        component_card(
                            "Input",
                            Input::new(Text::new("输入框示例输入框示例输入框示例输入框示例输入框示例输入框示例输入框示例输入框示例输入框示例输入框示例输入框示例输入框示例输入框示例输入框示例输入框示例输入框示例"))
                                .placeholder_with_str("请输入内容")
                                .width(dp(260.0)),
                        ),
                        component_card(
                            "Switch",
                            Switch::new(self.switch.binding())
                            .on_change(ValueCommand::new(|app: &mut App, enable| {
                                app.switch.set(enable)
                            }))
                        ),
                        component_card(
                            "Stack",
                            Stack::new()
                                .width(dp(260.0))
                                .height(dp(80.0))
                                .padding(Insets::all(dp(12.0)))
                                .background(Color::rgb(36, 44, 58))
                                .border_radius(dp(12.0))
                                .center()
                                .child(
                                    Text::new("Stack 容器")
                                        .font_size(sp(16.0))
                                        .color(Color::WHITE),
                                ),
                        ),
                        component_card(
                            "Flex",
                            Flex::new(Axis::Horizontal).gap(dp(10.0)).child(el![
                                pill("Rust"),
                                pill("Desktop"),
                                pill("GPU"),
                                pill("MVVM"),
                            ]),
                        ),
                        component_card(
                            "Grid",
                            Grid::columns([fr(1.0), fr(1.0)])
                                .gap(dp(10.0))
                                .width(dp(260.0))
                                .child(el![
                                    grid_block("A", Color::rgb(58, 86, 140)),
                                    grid_block("B", Color::rgb(67, 116, 89)),
                                    grid_block("C", Color::rgb(125, 82, 48)),
                                    grid_block("D", Color::rgb(124, 66, 102)),
                                ]),
                        ),
                        component_card(
                            "Image",
                            Image::from_path(demo_image_path())
                                .size(dp(220.0), dp(120.0))
                                .border_radius(dp(12.0)),
                        ),
                        component_card(
                            "其他组件",
                            Flex::new(Axis::Vertical).gap(dp(6.0)).child(el![
                                Text::new("Canvas：当前示例接入后会触发栈溢出，暂不启用")
                                    .color(Color::rgb(190, 198, 214)),
                                Text::new("VideoSurface：启用 video feature 后可用")
                                    .color(Color::rgb(190, 198, 214)),
                            ]),
                        ),
                    ]),
            )
            .into()
    }
}

fn component_card(title: &str, content: impl Into<Element<App>>) -> Element<App> {
    Flex::new(Axis::Vertical)
        .gap(dp(10.0))
        .padding(Insets::all(dp(14.0)))
        .background(Color::rgb(23, 28, 38))
        .border(dp(1.0), Color::rgb(48, 58, 76))
        .border_radius(dp(14.0))
        .child(el![
            Text::new(title)
                .font_size(sp(18.0))
                .color(Color::rgb(255, 255, 255)),
            content.into(),
        ])
        .into()
}

fn pill(label: &str) -> Element<App> {
    Text::new(label)
        .padding(Insets::symmetric(dp(10.0), dp(6.0)))
        .background(Color::rgb(38, 50, 68))
        .border_radius(dp(999.0))
        .color(Color::rgb(226, 234, 246))
        .into()
}

fn grid_block(label: &str, color: Color) -> Element<App> {
    Stack::new()
        .height(dp(52.0))
        .width(dp(52.0))
        .background(color)
        .border_radius(dp(10.0))
        .center()
        .child(Text::new(label).color(Color::WHITE).background(Color::RED))
        .into()
}

fn demo_image_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../background_effects/assets/juequling_shushu.jpg")
}

impl App {
    fn run() -> Result<(), TguiError> {
        let mut theme = Theme::dark();
        theme.colors.primary = Color::rgb(0, 120, 212);
        theme.refresh_components();

        Application::new()
            .theme(theme)
            .with_view_model(App::new)
            .root_view(App::view)
            .run()
    }
}

fn main() -> Result<(), TguiError> {
    App::run()
}
