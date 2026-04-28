use std::path::PathBuf;

use tgui::{
    dp, el, pct, sp, Application, Axis, Button, Canvas, CanvasGradientStop, CanvasItem,
    CanvasLinearGradient, CanvasPath, CanvasStroke, Color, Element, Flex, Image, Input, Insets,
    Observable, Overflow, PathBuilder, Point, Stack, Switch, Text, TguiError,
    ValueCommand, ViewModel, ViewModelContext,
};

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
                            ]),
                        ),
                        component_card(
                            "Input",
                            Input::new(Text::new("输入框示例输入框示例输入框示例"))
                                .placeholder_with_str("请输入内容")
                                .width(dp(260.0)),
                        ),
                        component_card(
                            "Switch",
                            Switch::new(self.switch.binding()).on_change(ValueCommand::new(
                                |app: &mut App, enable| app.switch.set(enable),
                            )),
                        ),
                        component_card(
                            "Image",
                            Image::from_path(demo_image_path())
                                .size(dp(220.0), dp(120.0))
                                .border_radius(dp(12.0)),
                        ),
                        component_card("Canvas", demo_canvas()),
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

fn demo_image_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../background_effects/assets/juequling_shushu.jpg")
}

fn demo_canvas() -> Element<App> {
    let items = vec![
        CanvasItem::Path(
            CanvasPath::new(
                1_u64,
                PathBuilder::new()
                    .move_to(24.0, 20.0)
                    .line_to(208.0, 20.0)
                    .line_to(208.0, 128.0)
                    .line_to(24.0, 128.0)
                    .close(),
            )
            .fill(CanvasLinearGradient::new(
                Point::new(24.0, 20.0),
                Point::new(208.0, 128.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                ],
            ))
            .stroke(CanvasStroke::new(dp(3.0), Color::hexa(0xE0F2FEFF))),
        ),
        CanvasItem::Path(
            CanvasPath::new(
                2_u64,
                PathBuilder::new()
                    .move_to(44.0, 146.0)
                    .quad_to(116.0, 92.0, 188.0, 146.0)
                    .line_to(188.0, 188.0)
                    .line_to(44.0, 188.0)
                    .close(),
            )
            .fill(Color::hexa(0x22C55EFF))
            .stroke(CanvasStroke::new(dp(3.0), Color::hexa(0x14532DFF))),
        ),
    ];

    Canvas::new(items)
        .size(dp(232.0), dp(212.0))
        .background(Color::rgb(15, 23, 42))
        .border(dp(1.0), Color::rgb(51, 65, 85))
        .border_radius(dp(14.0))
        .into()
}

impl App {
    fn run() -> Result<(), TguiError> {
        Application::new()
            .with_view_model(App::new)
            .root_view(App::view)
            .run()
    }
}

fn main() -> Result<(), TguiError> {
    App::run()
}
