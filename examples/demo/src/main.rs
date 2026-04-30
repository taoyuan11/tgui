use std::path::PathBuf;
use tgui::prelude::*;

struct App {
    content: Observable<String>,
    switch: Observable<bool>,
    checkbox: Observable<bool>,
    radio: Observable<bool>,
    contact_method: Observable<String>,
    select_action: Observable<Option<String>>,
    notification_status: Observable<String>,
}

impl ViewModel for App {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            content: context.observable(String::from("输入框示例输入框示例输入框示例")),
            switch: context.observable(false),
            checkbox: context.observable(false),
            radio: context.observable(false),
            contact_method: context.observable(String::from("email")),
            select_action: context.observable(None),
            notification_status: context.observable(String::from("尚未发送通知")),
        }
    }

    fn view(&self) -> Element<Self> {
        Flex::horizontal()
            .wrap(Wrap::Wrap)
            .padding(Insets::all(dp(20.0)))
            .gap(dp(10.0))
            .overflow_y(Overflow::Scroll)
            .child(el![
                Text::new("TGUI 组件列表示例")
                    .font_size(sp(28.0))
                    .width(pct(100.0))
                    .color(Color::WHITE),
                component_card(
                    "Text",
                    Text::new("这是一段可直接渲染、可复制的文本组件")
                        .user_select(true)
                        .font_size(sp(16.0))
                        .color(Color::rgb(240, 244, 255)),
                ),
                component_card(
                    "Input",
                    Input::new(Text::new(self.content.binding()))
                        .placeholder_with_str("请输入内容")
                        .width(dp(260.0))
                        .on_change(ValueCommand::new(|app: &mut App, text| {
                            app.content.set(text)
                        }))
                ),
                component_card(
                    "Button",
                    Flex::new(Axis::Horizontal).gap(dp(10.0)).child(el![
                        Button::new(Text::new("普通按钮")).primary(),
                        Button::new(Text::new("次要按钮")).secondary(),
                        Button::new(Text::new("幽灵按钮")).ghost(),
                        Button::new(Text::new("危险按钮")).danger(),
                        Button::new(Text::new("禁用按钮")).disable(true),
                    ]),
                ),
                component_card(
                    "Switch",
                    Switch::new(self.switch.binding()).on_change(ValueCommand::new(
                        |app: &mut App, enable| app.switch.set(enable),
                    )),
                ),
                component_card(
                    "Checkbox",
                    Checkbox::new(self.checkbox.binding())
                        .label(Text::new("接收通知"))
                        .on_change(ValueCommand::new(|app: &mut App, checked| {
                            app.checkbox.set(checked)
                        })),
                ),
                component_card(
                    "Radio",
                    Radio::new(self.radio.binding())
                        .label(Text::new("单个单选框"))
                        .on_change(ValueCommand::new(|app: &mut App, checked| {
                            app.radio.set(checked)
                        })),
                ),
                component_card(
                    "RadioGroup",
                    RadioGroup::new(
                        vec![
                            RadioOption::new("email".to_string(), "邮件".to_string()).disable(true),
                            RadioOption::new("sms".to_string(), "短信".to_string()),
                            RadioOption::new("phone".to_string(), "电话".to_string()),
                        ],
                        self.contact_method.binding(),
                    )
                    .horizontal()
                    .on_change(ValueCommand::new(|app: &mut App, (key, _label)| {
                        app.contact_method.set(key)
                    })),
                ),
                component_card(
                    "Select",
                    Select::new(
                        vec![
                            SelectOption::new("archive".to_string(), "归档".to_string()),
                            SelectOption::new("delete".to_string(), "删除".to_string())
                                .disable(true),
                            SelectOption::new("share".to_string(), "分享".to_string()),
                        ],
                        self.select_action.binding(),
                    )
                    .placeholder_with_str("请选择操作")
                    .width(dp(220.0))
                    .on_change(ValueCommand::new(|app: &mut App, (key, _label)| {
                        app.select_action.set(Some(key))
                    })),
                ),
                component_card(
                    "Notification",
                    Flex::vertical().gap(dp(10.0)).child(el![
                        Flex::horizontal().gap(dp(10.0)).wrap(Wrap::Wrap).child(el![
                            Button::new(Text::new("请求通知权限")).on_click(
                                Command::new_with_context(|_: &mut App, ctx| {
                                    App::request_notification_permission(ctx)
                                }),
                            ),
                            Button::new(Text::new("发送普通通知")).on_click(
                                Command::new_with_context(|app: &mut App, ctx| {
                                    app.send_plain_notification(ctx)
                                }),
                            ),
                            Button::new(Text::new("发送动作通知")).on_click(
                                Command::new_with_context(|app: &mut App, ctx| {
                                    app.send_action_notification(ctx)
                                }),
                            ),
                        ]),
                        Text::new(self.notification_status.binding())
                            .font_size(sp(14.0))
                            .color(Color::rgb(203, 213, 225)),
                    ]),
                ),
                component_card(
                    "Image",
                    Image::from_path(demo_image_path())
                        .size(dp(220.0), dp(120.0))
                        .border_radius(dp(12.0)),
                ),
                component_card("Canvas", demo_canvas()),
            ])
            .into()
    }
}

fn component_card(title: &str, content: impl Into<Element<App>>) -> Element<App> {
    Flex::vertical()
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
    fn request_notification_permission(ctx: &CommandContext<Self>) {
        let _ = ctx.notifications().request_permission(ValueCommand::new(
            |app: &mut App, result| {
                app.notification_status.set(match result {
                    Ok(permission) => format!("通知权限: {permission:?}"),
                    Err(error) => format!("通知权限请求失败: {error}"),
                });
            },
        ));
    }

    fn send_plain_notification(&mut self, ctx: &CommandContext<Self>) {
        let result = ctx.notifications().send(
            NotificationOptions::new("TGUI Demo")
                .body("这是一条来自 demo 示例的系统通知。")
                .app_name("TGUI Demo"),
        );
        self.notification_status.set(match result {
            Ok(id) => format!("已发送普通通知: {id}"),
            Err(error) => format!("发送普通通知失败: {error}"),
        });
    }

    fn send_action_notification(&mut self, ctx: &CommandContext<Self>) {
        let result = ctx.notifications().send_with_actions(
            NotificationOptions::new("TGUI Demo")
                .body("请选择一个动作，结果会回到 ViewModel。")
                .app_name("TGUI Demo")
                .action(NotificationAction::new("accept", "接受"))
                .action(NotificationAction::new("dismiss", "忽略")),
            ValueCommand::new(
                |app: &mut App,
                 result: Result<NotificationActionEvent, NotificationError>| {
                app.notification_status.set(match result {
                    Ok(event) => format!(
                        "通知动作: notification_id={}, action_id={}",
                        event.notification_id, event.action_id
                    ),
                    Err(error) => format!("通知动作失败: {error}"),
                });
            }),
        );
        self.notification_status.set(match result {
            Ok(id) => format!("已发送动作通知: {id}"),
            Err(error) => format!("发送动作通知失败: {error}"),
        });
    }

    fn run() -> Result<(), TguiError> {
        Application::new()
            .window_icon(include_bytes!("../../background_effects/assets/juequling_shushu.jpg"))
            .app_id("com.tgui.demo")
            .with_view_model(App::new)
            .root_view(App::view)
            .run()
    }
}

fn main() -> Result<(), TguiError> {
    App::run()
}
