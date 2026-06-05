use crate::app::App;
use crate::demo_section::{self, UsageDemo};
use crate::styles;
use tgui::prelude::*;

const CODE_TOOLTIP_PLACEMENT: &str = r#"Button::new("下方").tooltip(
    Tooltip::new("浮层贴在按钮下边缘")
        .placement(OverlayPlacement::bottom()),
)"#;

const CODE_TOOLTIP_CONTENT: &str = r#"Button::new("富内容").tooltip(
    Tooltip::content(
        Flex::vertical()
            .gap(dp(8.0))
            .child(Text::new("Tooltip 也能承载任意子树")),
    ),
)"#;

const CODE_POPOVER_CONTROLLED: &str = r#"Popover::new(Button::new("打开设置"))
    .content(panel)
    .open(app.popover_open.signal())
    .on_open_change(ValueCommand::new(|app: &mut App, open| {
        app.popover_open.set(open);
    }))"#;

const CODE_POPOVER_HOVER: &str = r#"Popover::new(Button::new("点击或悬停"))
    .content(panel)
    .trigger_mode(PopoverTriggerMode::ClickAndHoverPreview)"#;

const CODE_MENU_BUTTON: &str = r#"Menu::new(Button::new("更多操作"))
    .item(MenuItem::new("复制").on_select(Command::new(|app: &mut App| {
        app.toast_status.set("复制".to_string());
    })))"#;

const CODE_MENU_BAR: &str = r#"MenuBar::uncontrolled()
    .entry("文件", vec![MenuItem::new("新建"), MenuItem::separator()])
    .entry("编辑", vec![MenuItem::new("撤销"), MenuItem::new("重做")])"#;

const CODE_MODAL_ALERT: &str = r#"Modal::new(app.modal_alert_open.signal())
    .title("提示")
    .content(Text::new("这是一个简单 alert。"))
    .action(ModalAction::primary("OK").on_click(Command::new(|app: &mut App| {
        app.modal_alert_open.set(false);
    })))"#;

const CODE_MODAL_CONFIRM: &str = r#"Modal::new(app.modal_confirm_open.signal())
    .title("确认操作")
    .action(ModalAction::new("取消").on_click(Command::new(App::confirm_cancel)))
    .action(ModalAction::primary("确认").on_click(Command::new(App::confirm_yes)))"#;

const CODE_MODAL_FORM: &str = r#"Modal::new(app.modal_form_open.signal())
    .title("编辑名称")
    .content(Input::new(app.modal_form_name.clone()))
    .action(ModalAction::primary("保存").on_click(Command::new(App::submit_form_modal)))"#;

const CODE_DRAWER_PLACEMENT: &str = r#"Drawer::new(app.drawer_left_open.signal())
    .placement(DrawerPlacement::Left)
    .content(panel)
    .on_open_change(ValueCommand::new(|app: &mut App, open| {
        app.drawer_left_open.set(open);
    }))"#;

const CODE_DRAWER_EDGES: &str = r#"Drawer::new(app.drawer_top_open.signal())
    .placement(DrawerPlacement::Top)
    .content(Text::new("顶部面板"))"#;

const CODE_DRAWER_PUSH: &str = r#"DrawerHost::new(
    main_content,
    Drawer::new(app.drawer_push_open.signal())
        .mode(DrawerMode::Push)
        .placement(DrawerPlacement::Left)
        .content(sidebar),
)"#;

pub(crate) fn page(app: &App) -> Element<App> {
    demo_section::page(
        "Overlays",
        "浮层页面展示 tooltip、popover、菜单、modal 和 drawer。",
        vec![
            tooltip_component(app),
            popover_component(app),
            menu_component(app),
            modal_component(app),
            drawer_component(app),
        ],
    )
}

fn tooltip_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Tooltip",
        "Tooltip 为控件补充短说明，支持方向、自定义样式和任意内容。",
        vec![
            UsageDemo::new(
                "tooltip/placement",
                "方向",
                "hover 或键盘聚焦按钮即可查看提示。",
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("上方").tooltip(Tooltip::new("默认 Placement::top()")),
                    Button::new("下方").tooltip(
                        Tooltip::new("浮层贴在按钮下边缘").placement(OverlayPlacement::bottom()),
                    ),
                    Button::new("左侧").tooltip(
                        Tooltip::new("Placement::left()").placement(OverlayPlacement::left()),
                    ),
                    Button::new("右侧").tooltip(
                        Tooltip::new("Placement::right()").placement(OverlayPlacement::right()),
                    ),
                ]),
                CODE_TOOLTIP_PLACEMENT,
            ),
            UsageDemo::new(
                "tooltip/content",
                "长文本和富内容",
                "Tooltip 可以自动换行，也能承载小型组件树。",
                Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(el![
                    Button::new("长文本").tooltip(Tooltip::new(
                        "Tooltip 会在超过 max_width 时自动换行，适合较长说明文字。",
                    )),
                    Button::new("强调样式").tooltip(
                        Tooltip::new("自定义 background / radius 即可换风格")
                            .style(styles::accent_tooltip_style(ResolvedThemeMode::Dark)),
                    ),
                    Button::new("富内容").tooltip(Tooltip::content(
                        Flex::vertical()
                            .gap(dp(8.0))
                            .padding(Insets::all(dp(10.0)))
                            .child(Text::new("Tooltip 也能承载任意子树"))
                            .child(Button::new("浮层里按钮").ghost()),
                    )),
                ]),
                CODE_TOOLTIP_CONTENT,
            ),
        ],
    )
}

fn popover_component(app: &App) -> Element<App> {
    let trigger_text = app.popover_open.signal().map(|open| {
        if open {
            "已固定打开"
        } else {
            "打开设置"
        }
        .to_string()
    });

    let panel = || {
        Flex::vertical()
            .gap(dp(10.0))
            .width(dp(280.0))
            .padding(Insets::all(dp(14.0)))
            .style(styles::popover_panel_style)
            .child(el![
                Text::new("快速设置").style(styles::usage_title_style),
                Input::new(app.popover_note.clone())
                    .placeholder("输入浮层里的备注")
                    .width(dp(240.0)),
                Switch::new(app.popover_switch.signal()).on_change(ValueCommand::new(
                    |app: &mut App, enabled| app.popover_switch.set(enabled),
                )),
                Button::new("关闭")
                    .ghost()
                    .on_click(Command::new(|app: &mut App| {
                        app.popover_open.set(false);
                    })),
            ])
    };

    demo_section::component_doc(
        app,
        "Popover",
        "Popover 适合可交互浮层，可受控打开，也可支持 hover 预览。",
        vec![
            UsageDemo::new(
                "popover/controlled",
                "受控打开",
                "open 和 on_open_change 将浮层状态交给 ViewModel。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Popover::new(Button::new(trigger_text).secondary().width(dp(180.0)))
                        .content(panel())
                        .open(app.popover_open.signal())
                        .on_open_change(ValueCommand::new(|app: &mut App, open| {
                            app.popover_open.set(open);
                        })),
                    Text::new(app.popover_switch.signal().map(|enabled| {
                        if enabled {
                            "浮层内开关: 开启"
                        } else {
                            "浮层内开关: 关闭"
                        }
                        .to_string()
                    }))
                    .style(styles::status_style),
                ]),
                CODE_POPOVER_CONTROLLED,
            ),
            UsageDemo::new(
                "popover/hover",
                "点击与悬停预览",
                "同一 Popover 可以点击固定打开，也能 hover 预览。",
                Popover::new(Button::new("点击或悬停").secondary().width(dp(180.0)))
                    .content(panel())
                    .trigger_mode(PopoverTriggerMode::ClickAndHoverPreview),
                CODE_POPOVER_HOVER,
            ),
        ],
    )
}

fn menu_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Menu / MenuBar",
        "Menu 提供按钮下拉操作，MenuBar 适合应用级主菜单。",
        vec![
            UsageDemo::new(
                "menu/button",
                "按钮菜单",
                "菜单项通过 on_select 触发命令。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Menu::new(Button::new("更多操作").secondary())
                        .item(
                            MenuItem::new("复制").on_select(Command::new(|app: &mut App| {
                                app.toast_status.set("菜单操作: 复制".to_string());
                            }))
                        )
                        .item(
                            MenuItem::new("归档").on_select(Command::new(|app: &mut App| {
                                app.toast_status.set("菜单操作: 归档".to_string());
                            }))
                        )
                        .item(MenuItem::separator())
                        .item(
                            MenuItem::new("删除").on_select(Command::new(|app: &mut App| {
                                app.toast_status.set("菜单操作: 删除".to_string());
                            }))
                        ),
                    Text::new(app.toast_status.signal()).style(styles::status_style),
                ]),
                CODE_MENU_BUTTON,
            ),
            UsageDemo::new(
                "menu/bar",
                "MenuBar",
                "uncontrolled 模式由 runtime 管理展开状态。",
                MenuBar::uncontrolled()
                    .entry(
                        "文件",
                        vec![
                            MenuItem::new("新建"),
                            MenuItem::separator(),
                            MenuItem::new("退出"),
                        ],
                    )
                    .entry("编辑", vec![MenuItem::new("撤销"), MenuItem::new("重做")])
                    .entry("视图", vec![MenuItem::new("缩放"), MenuItem::new("全屏")]),
                CODE_MENU_BAR,
            ),
        ],
    )
}

fn modal_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Modal",
        "Modal 是应用内对话框，支持 backdrop、Esc 关闭和 focus trap。",
        vec![
            UsageDemo::new(
                "modal/alert",
                "Alert",
                "单按钮提示适合确认已读信息。",
                modal_preview(
                    Button::new("打开 Alert").on_click(Command::new(App::open_alert_modal)),
                    Modal::new(app.modal_alert_open.signal())
                        .on_open_change(ValueCommand::new(App::dismiss_alert_modal))
                        .title("提示")
                        .content(Text::new("这是一个简单的 alert modal。"))
                        .action(ModalAction::primary("OK").on_click(Command::new(
                            |app: &mut App| {
                                app.modal_alert_open.set(false);
                            },
                        ))),
                ),
                CODE_MODAL_ALERT,
            ),
            UsageDemo::new(
                "modal/confirm",
                "Confirm",
                "双按钮对话框会把用户选择写回状态文本。",
                modal_preview(
                    Flex::vertical().gap(dp(8.0)).child(el![
                        Button::new("打开 Confirm").on_click(Command::new(App::open_confirm_modal)),
                        Text::new(app.modal_confirm_result.signal()).style(styles::status_style),
                    ]),
                    Modal::new(app.modal_confirm_open.signal())
                        .on_open_change(ValueCommand::new(App::dismiss_confirm_modal))
                        .title("确认操作")
                        .content(Text::new("是否继续此操作？"))
                        .action(
                            ModalAction::new("取消").on_click(Command::new(App::confirm_cancel)),
                        )
                        .action(
                            ModalAction::primary("确认").on_click(Command::new(App::confirm_yes)),
                        ),
                ),
                CODE_MODAL_CONFIRM,
            ),
            UsageDemo::new(
                "modal/form",
                "自定义内容",
                "Modal content 可以承载输入控件和任意布局。",
                modal_preview(
                    Flex::vertical().gap(dp(8.0)).child(el![
                        Button::new("编辑名称").on_click(Command::new(App::open_form_modal)),
                        Text::new(app.modal_form_result.signal()).style(styles::status_style),
                    ]),
                    Modal::new(app.modal_form_open.signal())
                        .on_open_change(ValueCommand::new(App::dismiss_form_modal))
                        .title("编辑名称")
                        .content(
                            Flex::vertical()
                                .gap(dp(8.0))
                                .child(Text::new("请填写名称:"))
                                .child(
                                    Input::new(app.modal_form_name.clone())
                                        .placeholder("Anonymous"),
                                ),
                        )
                        .action(
                            ModalAction::new("取消").on_click(Command::new(|app: &mut App| {
                                app.modal_form_open.set(false);
                            })),
                        )
                        .action(
                            ModalAction::primary("保存")
                                .on_click(Command::new(App::submit_form_modal)),
                        ),
                ),
                CODE_MODAL_FORM,
            ),
        ],
    )
}

fn modal_preview(trigger: impl Into<Element<App>>, modal: Modal<App>) -> Element<App> {
    Stack::new()
        .height(dp(220.0))
        .child(Flex::vertical().height(pct(100.0)).center().child(trigger))
        .child(modal)
        .into()
}

fn drawer_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Drawer",
        "Drawer 提供侧边、顶部、底部抽屉，也可通过 DrawerHost 使用 push 模式。",
        vec![
            UsageDemo::new(
                "drawer/placement",
                "左侧抽屉",
                "Overlay 模式覆盖当前预览区域，Esc 或遮罩可关闭。",
                drawer_preview(
                    Button::new("打开左侧抽屉").on_click(Command::new(App::toggle_left_drawer)),
                    Drawer::new(app.drawer_left_open.signal())
                        .placement(DrawerPlacement::Left)
                        .on_open_change(ValueCommand::new(|app: &mut App, open| {
                            app.drawer_left_open.set(open);
                        }))
                        .content(drawer_panel("左侧导航", App::toggle_left_drawer)),
                ),
                CODE_DRAWER_PLACEMENT,
            ),
            UsageDemo::new(
                "drawer/edges",
                "不同方向",
                "同一 API 可从任意边缘滑出。",
                Stack::new()
                    .height(dp(220.0))
                    .child(
                        Flex::vertical()
                            .height(pct(100.0))
                            .center()
                            .gap(dp(8.0))
                            .child(el![Flex::horizontal().gap(dp(8.0)).wrap(Wrap::Wrap).child(
                                el![
                                    Button::new("右侧")
                                        .on_click(Command::new(App::toggle_right_drawer)),
                                    Button::new("顶部")
                                        .on_click(Command::new(App::toggle_top_drawer)),
                                    Button::new("底部")
                                        .on_click(Command::new(App::toggle_bottom_drawer)),
                                ]
                            ),]),
                    )
                    .child(
                        Drawer::new(app.drawer_right_open.signal())
                            .placement(DrawerPlacement::Right)
                            .on_open_change(ValueCommand::new(|app: &mut App, open| {
                                app.drawer_right_open.set(open);
                            }))
                            .content(drawer_panel("右侧面板", App::toggle_right_drawer)),
                    )
                    .child(
                        Drawer::new(app.drawer_top_open.signal())
                            .placement(DrawerPlacement::Top)
                            .on_open_change(ValueCommand::new(|app: &mut App, open| {
                                app.drawer_top_open.set(open);
                            }))
                            .content(drawer_panel("顶部面板", App::toggle_top_drawer)),
                    )
                    .child(
                        Drawer::new(app.drawer_bottom_open.signal())
                            .placement(DrawerPlacement::Bottom)
                            .on_open_change(ValueCommand::new(|app: &mut App, open| {
                                app.drawer_bottom_open.set(open);
                            }))
                            .content(drawer_panel("底部面板", App::toggle_bottom_drawer)),
                    ),
                CODE_DRAWER_EDGES,
            ),
            UsageDemo::new(
                "drawer/push",
                "Push 模式",
                "DrawerHost 会让主内容为抽屉让位。",
                DrawerHost::new(
                    Flex::vertical()
                        .gap(dp(8.0))
                        .padding(Insets::all(dp(16.0)))
                        .child(Text::new("主内容区域").style(styles::usage_title_style))
                        .child(
                            Text::new("打开 Push 模式时，这块内容会被侧栏同步推开。")
                                .style(styles::status_style),
                        )
                        .child(
                            Button::new("Push 模式")
                                .on_click(Command::new(App::toggle_push_drawer)),
                        ),
                    Drawer::new(app.drawer_push_open.signal())
                        .mode(DrawerMode::Push)
                        .placement(DrawerPlacement::Left)
                        .on_open_change(ValueCommand::new(|app: &mut App, open| {
                            app.drawer_push_open.set(open);
                        }))
                        .content(drawer_panel("Push Sidebar", App::toggle_push_drawer)),
                )
                .height(dp(220.0)),
                CODE_DRAWER_PUSH,
            ),
        ],
    )
}

fn drawer_preview(trigger: impl Into<Element<App>>, drawer: Drawer<App>) -> Element<App> {
    Stack::new()
        .height(dp(220.0))
        .child(Flex::vertical().height(pct(100.0)).center().child(trigger))
        .child(drawer)
        .into()
}

fn drawer_panel(title: &'static str, close: fn(&mut App)) -> Element<App> {
    Flex::vertical()
        .gap(dp(12.0))
        .padding(Insets::all(dp(16.0)))
        .child(Text::new(title).style(styles::usage_title_style))
        .child(Text::new("这里可以放置导航、表单或上下文操作。").style(styles::status_style))
        .child(Button::new("关闭").ghost().on_click(Command::new(close)))
        .into()
}
