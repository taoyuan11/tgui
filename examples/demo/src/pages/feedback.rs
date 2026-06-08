use std::time::Duration;

use crate::app::App;
use crate::demo_section::{self, UsageDemo};
use crate::styles;
use tgui::prelude::*;

const CODE_PROGRESS_VALUE: &str = r#"ProgressBar::new(app.slider_value.signal().map(|value| value / 100.0))
    .show_label(true)
    .label(app.slider_value.signal().map(|value| format!("{value:.0}%")))"#;

const CODE_PROGRESS_INDETERMINATE: &str = r#"ProgressBar::indeterminate(true)
    .show_label(true)
    .label("处理中")"#;

const CODE_SPINNER_BASIC: &str = r#"Flex::horizontal().gap(dp(12.0)).child(el![
    Spinner::new(),
    Spinner::new().size(dp(32.0), dp(32.0)),
])"#;

const CODE_SPINNER_REDUCED: &str = r#"Switch::new(app.reduced_motion.signal())
    .on_change(ValueCommand::new(|app: &mut App, enabled| {
        app.reduced_motion.set(enabled);
    }))"#;

const CODE_SKELETON_LINES: &str = r#"Flex::vertical().gap(dp(6.0)).child(el![
    Skeleton::line().width(dp(180.0)),
    Skeleton::lines(2),
])"#;

const CODE_SKELETON_CARD: &str = r#"Flex::vertical().gap(dp(10.0)).child(el![
    Skeleton::line().width(dp(220.0)),
    Skeleton::lines(3),
])"#;

const CODE_TOAST_KINDS: &str = r#"app.toast_queue.push(
    Toast::new("文件已成功保存到云端")
        .title("保存成功")
        .kind(ToastKind::Success),
);"#;

const CODE_TOAST_ACTION: &str = r#"Toast::new("文件已移入回收站")
    .title("Snackbar")
    .kind(ToastKind::Info)
    .action(ToastAction::new("撤销", Command::new(|app: &mut App| {
        app.toast_status.set("点击了撤销".to_string());
    })))"#;

const CODE_TOAST_PLACEMENT: &str = r#"ToastHost::new(queue)
    .placement(ToastPlacement::TopCenter)
    .style_full(modern_toast_style)"#;

const CODE_NOTIFICATION_PERMISSION: &str = r#"ctx.notifications().request_permission(
    ValueCommand::new(|app: &mut App, result| {
        app.notification_status.set(format!("{result:?}"));
    }),
);"#;

const CODE_NOTIFICATION_ACTION: &str = r#"ctx.notifications().send_with_actions(
    NotificationOptions::new("TGUI Demo")
        .body("请选择一个动作")
        .action(NotificationAction::new("accept", "接受")),
    ValueCommand::new(|app: &mut App, result| {
        app.notification_status.set(format!("{result:?}"));
    }),
);"#;

pub(crate) fn page(app: &App) -> Element<App> {
    demo_section::page(
        "Feedback",
        "反馈页面展示进度、加载、骨架屏、应用内提示和系统通知。",
        vec![
            progress_component(app),
            spinner_component(app),
            skeleton_component(app),
            toast_component(app),
            notification_component(app),
        ],
    )
}

fn progress_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "ProgressBar",
        "ProgressBar 支持确定态和不确定态，适合任务进度或后台处理状态。",
        vec![
            UsageDemo::new(
                "progress/value",
                "确定态进度",
                "进度值复用 Forms 页的 Slider 状态。",
                ProgressBar::new(app.slider_value.signal().map(|value| value / 100.0))
                    .width(dp(320.0))
                    .show_label(true)
                    .label(
                        app.slider_value
                            .signal()
                            .map(|value| format!("当前进度 {value:.0}%")),
                    ),
                CODE_PROGRESS_VALUE,
            ),
            UsageDemo::new(
                "progress/indeterminate",
                "不确定态进度",
                "用于无法预估完成比例的后台任务。",
                ProgressBar::indeterminate(true)
                    .width(dp(320.0))
                    .show_label(true)
                    .label("处理中"),
                CODE_PROGRESS_INDETERMINATE,
            ),
        ],
    )
}

fn spinner_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Spinner",
        "Spinner 用于轻量加载提示，并响应 reduced motion 设置。",
        vec![
            UsageDemo::new(
                "spinner/basic",
                "尺寸变体",
                "默认尺寸和较大尺寸可以并排展示。",
                Flex::horizontal().gap(dp(12.0)).child(el![
                    Spinner::new(),
                    Spinner::new().size(dp(32.0), dp(32.0)).style_full(|ctx| {
                        let mut style = SpinnerStyle::default_for_theme(ctx.theme);
                        style.indicator_color = Color::hexa(0x2563EBFF).into();
                        style.track_color = Color::hexa(0x93C5FD88).into();
                        style.size = dp(32.0);
                        style.thickness = dp(4.0);
                        style
                    }),
                ]),
                CODE_SPINNER_BASIC,
            ),
            UsageDemo::new(
                "spinner/reduced-motion",
                "Reduced motion",
                "切换后应用级 reduced motion 绑定会影响循环动画。",
                Flex::horizontal().gap(dp(10.0)).wrap(Wrap::Wrap).child(el![
                    Switch::new(app.reduced_motion.signal()).on_change(ValueCommand::new(
                        |app: &mut App, enabled| app.reduced_motion.set(enabled),
                    )),
                    Text::new(app.reduced_motion.signal().map(|enabled| {
                        if enabled {
                            "reduced-motion: 已开启".to_string()
                        } else {
                            "reduced-motion: 已关闭".to_string()
                        }
                    }))
                    .style_full(styles::status_style),
                ]),
                CODE_SPINNER_REDUCED,
            ),
        ],
    )
}

fn skeleton_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Skeleton",
        "Skeleton 用于异步内容加载占位，减少页面跳动并保持结构感。",
        vec![
            UsageDemo::new(
                "skeleton/lines",
                "文本骨架",
                "多行骨架适合列表、摘要和详情加载态。",
                Flex::vertical().gap(dp(6.0)).child(el![
                    Skeleton::line().width(dp(180.0)),
                    Skeleton::lines(2),
                ]),
                CODE_SKELETON_LINES,
            ),
            UsageDemo::new(
                "skeleton/card",
                "内容块骨架",
                "组合不同宽度的 line 可以模拟卡片或面板内容。",
                Flex::vertical().gap(dp(10.0)).child(el![
                    Skeleton::line().width(dp(220.0)),
                    Skeleton::lines(3),
                ]),
                CODE_SKELETON_CARD,
            ),
        ],
    )
}

fn toast_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Toast / Snackbar",
        "ToastQueue 用于应用内短提示，ToastHost 负责具体位置和展示样式。",
        vec![
            UsageDemo::new(
                "toast/kinds",
                "语义提示",
                "不同 kind 会使用对应的语义色。",
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("Success").on_click(Command::new(|app: &mut App| {
                        app.toast_queue.push(
                            Toast::new("文件已成功保存到云端")
                                .title("保存成功")
                                .kind(ToastKind::Success),
                        );
                        app.toast_status.set("最近操作: success toast".to_string());
                    })),
                    Button::new("Error")
                        .danger()
                        .on_click(Command::new(|app: &mut App| {
                            app.toast_queue.push(
                                Toast::new("网络连接失败，请检查设置")
                                    .title("上传失败")
                                    .kind(ToastKind::Error),
                            );
                            app.toast_status.set("最近操作: error toast".to_string());
                        })),
                    Button::new("Warning")
                        .secondary()
                        .on_click(Command::new(|app: &mut App| {
                            app.toast_queue.push(
                                Toast::new("检测到网络波动，已切换离线模式")
                                    .title("网络提醒")
                                    .kind(ToastKind::Warning),
                            );
                            app.toast_status.set("最近操作: warning toast".to_string());
                        })),
                    Button::new("Info")
                        .ghost()
                        .on_click(Command::new(|app: &mut App| {
                            app.toast_queue.push(
                                Toast::new("后台同步将在稍后开始")
                                    .title("提示")
                                    .kind(ToastKind::Info),
                            );
                            app.toast_status.set("最近操作: info toast".to_string());
                        })),
                ]),
                CODE_TOAST_KINDS,
            ),
            UsageDemo::new(
                "toast/action",
                "Snackbar action",
                "Toast 可以附加一个短动作，适合撤销类操作。",
                Flex::horizontal().gap(dp(8.0)).child(el![
                    Button::new("弹出撤销提示").on_click(Command::new(|app: &mut App| {
                        app.toast_queue.push(
                            Toast::new("文件已移入回收站")
                                .title("Snackbar")
                                .kind(ToastKind::Info)
                                .action(ToastAction::new(
                                    "撤销",
                                    Command::new(|app: &mut App| {
                                        app.toast_status.set("最近操作: 点击了撤销".to_string());
                                    }),
                                )),
                        );
                        app.toast_status
                            .set("最近操作: 弹出撤销 snackbar".to_string());
                    })),
                    Button::new("短时提示")
                        .secondary()
                        .on_click(Command::new(|app: &mut App| {
                            app.toast_queue.push(
                                Toast::new("这个提示将在 2 秒后自动消失")
                                    .title("快速提示")
                                    .kind(ToastKind::Success)
                                    .duration(Duration::from_secs(2)),
                            );
                            app.toast_status
                                .set("最近操作: 弹出 2 秒 toast".to_string());
                        })),
                    Text::new(app.toast_status.signal()).style_full(styles::status_style),
                ]),
                CODE_TOAST_ACTION,
            ),
            UsageDemo::new(
                "toast/placement",
                "位置示例",
                "根节点挂载多个 ToastHost，用不同队列演示位置。",
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("左上")
                        .ghost()
                        .on_click(Command::new(|app: &mut App| {
                            app.toast_top_start.push(
                                Toast::new("左上角弹出的提示")
                                    .title("TopStart")
                                    .kind(ToastKind::Info)
                                    .duration(Duration::from_secs(3)),
                            );
                        })),
                    Button::new("顶部居中")
                        .ghost()
                        .on_click(Command::new(|app: &mut App| {
                            app.toast_top_center.push(
                                Toast::new("顶部居中弹出的提示")
                                    .title("TopCenter")
                                    .kind(ToastKind::Success)
                                    .duration(Duration::from_secs(3)),
                            );
                        })),
                    Button::new("右上")
                        .ghost()
                        .on_click(Command::new(|app: &mut App| {
                            app.toast_top_end.push(
                                Toast::new("右上角弹出的提示")
                                    .title("TopEnd")
                                    .kind(ToastKind::Warning)
                                    .duration(Duration::from_secs(3)),
                            );
                        })),
                    Button::new("左下")
                        .ghost()
                        .on_click(Command::new(|app: &mut App| {
                            app.toast_bottom_start.push(
                                Toast::new("左下角弹出的提示")
                                    .title("BottomStart")
                                    .kind(ToastKind::Error)
                                    .duration(Duration::from_secs(3)),
                            );
                        })),
                    Button::new("底部居中")
                        .ghost()
                        .on_click(Command::new(|app: &mut App| {
                            app.toast_bottom_center.push(
                                Toast::new("底部居中弹出的提示")
                                    .title("BottomCenter")
                                    .kind(ToastKind::Success)
                                    .duration(Duration::from_secs(3)),
                            );
                        })),
                ]),
                CODE_TOAST_PLACEMENT,
            ),
        ],
    )
}

fn notification_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Notification",
        "系统通知通过 CommandContext 访问平台服务，动作通知会把结果回调到 ViewModel。",
        vec![
            UsageDemo::new(
                "notification/permission",
                "权限请求和普通通知",
                "Windows 下需要稳定 app_id；demo 已在 Application 上设置。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                        Button::new("请求权限").on_click(Command::new_with_context(
                            |_: &mut App, ctx| App::request_notification_permission(ctx),
                        )),
                        Button::new("发送通知").on_click(Command::new_with_context(
                            |app: &mut App, ctx| app.send_plain_notification(ctx),
                        )),
                    ]),
                    Text::new(app.notification_status.signal()).style_full(styles::status_style),
                ]),
                CODE_NOTIFICATION_PERMISSION,
            ),
            UsageDemo::new(
                "notification/action",
                "动作通知",
                "最多两个 action，点击结果会派发回当前 App；macOS 当前不支持系统通知 action。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Button::new("发送动作通知").on_click(Command::new_with_context(
                        |app: &mut App, ctx| app.send_action_notification(ctx),
                    )),
                    Text::new(app.notification_status.signal()).style_full(styles::status_style),
                ]),
                CODE_NOTIFICATION_ACTION,
            ),
        ],
    )
}
