use std::time::Duration;

use tgui::prelude::*;

fn text_style(mode: ResolvedThemeMode, size: Sp) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.typography.size = size;
    style
}

fn title_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    let mut style = text_style(mode, sp(28.0));
    style.typography.weight = FontWeight::SemiBold;
    style
}

fn body_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    let style = text_style(mode, sp(15.0));
    style
}

fn status_style(mode: ResolvedThemeMode) -> TextWidgetStyle {
    let mut style = text_style(mode, sp(14.0));
    style.color = Color::BLUE.into();
    style
}

fn panel_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.border_color = Some(Color::hexa(0x334155FF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

fn toast_style(mode: ResolvedThemeMode) -> ToastStyle {
    let mut style = ToastStyle::default_for(mode);
    style.radius = Value::Static(dp(8.0));
    style.border_width = Value::Static(dp(0.0));
    style.padding = Insets::all(dp(12.0));
    style.gap = dp(9.0);
    style.min_width = dp(220.0);
    style.max_width = dp(340.0);
    style.margin = dp(18.0);
    style.stack_gap = dp(10.0);
    style.shadow = Shadow {
        offset_x: dp(0.0),
        offset_y: dp(10.0),
        blur: dp(30.0),
        spread: dp(0.0),
        color: match mode {
            ResolvedThemeMode::Light => Color::rgba(15, 23, 42, 45),
            ResolvedThemeMode::Dark => Color::rgba(0, 0, 0, 125),
        },
    };
    style.title_text_style.weight = FontWeight::SemiBold;
    style.action_button.min_height = dp(26.0);
    style.action_button.padding_x = dp(8.0);
    style.action_button.padding_y = dp(4.0);
    style.action_button.radius = Value::Static(dp(6.0));
    style
}

struct ToastDemoVm {
    status: State<String>,
    main_queue: ToastQueue<Self>,
    top_start_queue: ToastQueue<Self>,
    top_center_queue: ToastQueue<Self>,
    top_end_queue: ToastQueue<Self>,
    bottom_start_queue: ToastQueue<Self>,
    bottom_center_queue: ToastQueue<Self>,
}

impl ToastDemoVm {
    fn push_success(&mut self) {
        self.main_queue.push(
            Toast::new("文件已成功保存到云端")
                .title("保存成功")
                .kind(ToastKind::Success),
        );
        self.status.set("最近操作: success toast".to_string());
    }

    fn push_error(&mut self) {
        self.main_queue.push(
            Toast::new("网络连接失败, 请检查您的网络设置")
                .title("上传失败")
                .kind(ToastKind::Error),
        );
        self.status.set("最近操作: error toast".to_string());
    }

    fn push_warning(&mut self) {
        self.main_queue.push(
            Toast::new("检测到网络波动, 已自动切换为离线模式")
                .title("网络提醒")
                .kind(ToastKind::Warning),
        );
        self.status.set("最近操作: warning toast".to_string());
    }

    fn push_info(&mut self) {
        self.main_queue.push(
            Toast::new("后台同步将在 30 秒后自动开始")
                .title("提示")
                .kind(ToastKind::Info),
        );
        self.status.set("最近操作: info toast".to_string());
    }

    fn push_snackbar(&mut self) {
        self.main_queue.push(
            Toast::new("文件已移入回收站")
                .title("Snackbar")
                .kind(ToastKind::Info)
                .action(ToastAction::new(
                    "撤销",
                    Command::new(|vm: &mut Self| {
                        vm.status.set("最近操作: 点击了撤销".to_string());
                    }),
                )),
        );
        self.status.set("最近操作: 弹出撤销 snackbar".to_string());
    }

    fn push_persistent(&mut self) {
        self.main_queue.push(
            Toast::new("正在连接远程设备, 完成后请手动关闭此提示")
                .title("持久连接")
                .kind(ToastKind::Warning)
                .persistent(true)
                .show_close_button(true),
        );
        self.status.set("最近操作: 弹出持久 toast".to_string());
    }

    fn push_short(&mut self) {
        self.main_queue.push(
            Toast::new("这个提示将在 2 秒后自动消失")
                .title("快速提示")
                .kind(ToastKind::Success)
                .duration(Duration::from_secs(2)),
        );
        self.status.set("最近操作: 弹出 2 秒 toast".to_string());
    }

    fn clear_toasts(&mut self) {
        self.main_queue.clear();
        self.top_start_queue.clear();
        self.top_center_queue.clear();
        self.top_end_queue.clear();
        self.bottom_start_queue.clear();
        self.bottom_center_queue.clear();
        self.status.set("最近操作: 已清空全部 toast".to_string());
    }

    fn push_top_start(&mut self) {
        self.top_start_queue.push(
            Toast::new("左上角弹出的提示")
                .title("TopStart")
                .kind(ToastKind::Info)
                .duration(Duration::from_secs(3)),
        );
        self.status.set("最近操作: 左上 toast".to_string());
    }

    fn push_top_center(&mut self) {
        self.top_center_queue.push(
            Toast::new("顶部居中弹出的提示")
                .title("TopCenter")
                .kind(ToastKind::Success)
                .duration(Duration::from_secs(3)),
        );
        self.status.set("最近操作: 顶部居中 toast".to_string());
    }

    fn push_top_end(&mut self) {
        self.top_end_queue.push(
            Toast::new("右上角弹出的提示")
                .title("TopEnd")
                .kind(ToastKind::Warning)
                .duration(Duration::from_secs(3)),
        );
        self.status.set("最近操作: 右上 toast".to_string());
    }

    fn push_bottom_start(&mut self) {
        self.bottom_start_queue.push(
            Toast::new("左下角弹出的提示")
                .title("BottomStart")
                .kind(ToastKind::Error)
                .duration(Duration::from_secs(3)),
        );
        self.status.set("最近操作: 左下 toast".to_string());
    }

    fn push_bottom_center(&mut self) {
        self.bottom_center_queue.push(
            Toast::new("底部居中弹出的提示")
                .title("BottomCenter")
                .kind(ToastKind::Success)
                .duration(Duration::from_secs(3)),
        );
        self.status.set("最近操作: 底部居中 toast".to_string());
    }

    fn push_bottom_end(&mut self) {
        self.main_queue.push(
            Toast::new("右下角弹出的提示")
                .title("BottomEnd / Adaptive")
                .kind(ToastKind::Info)
                .duration(Duration::from_secs(3)),
        );
        self.status
            .set("最近操作: 右下 toast (默认位置)".to_string());
    }

    fn controls(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .width(dp(760.0))
            .max_width(pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .gap(dp(18.0))
            .style(panel_style)
            .child(Text::new("Toast / Snackbar").style(title_style))
            .child(Text::new("单独展示 ToastHost + ToastQueue 的常见用法: 语义类型、action、持久提示、短时提示、清空队列和不同屏幕位置。").style(body_style))
            .child(
                Flex::new(Axis::Horizontal)
                    .gap(dp(10.0))
                    .wrap(Wrap::Wrap)
                    .child(
                        Button::new("Success")
                            .on_click(Command::new(Self::push_success)),
                    )
                    .child(
                        Button::new("Error")
                            .danger()
                            .on_click(Command::new(Self::push_error)),
                    )
                    .child(
                        Button::new("Warning")
                            .secondary()
                            .on_click(Command::new(Self::push_warning)),
                    )
                    .child(
                        Button::new("Info")
                            .ghost()
                            .on_click(Command::new(Self::push_info)),
                    ),
            )
            .child(
                Flex::new(Axis::Horizontal)
                    .gap(dp(10.0))
                    .wrap(Wrap::Wrap)
                    .child(
                        Button::new("Snackbar Action")
                            .on_click(Command::new(Self::push_snackbar)),
                    )
                    .child(
                        Button::new("Persistent")
                            .secondary()
                            .on_click(Command::new(Self::push_persistent)),
                    )
                    .child(
                        Button::new("2s Toast")
                            .secondary()
                            .on_click(Command::new(Self::push_short)),
                    )
                    .child(
                        Button::new("Clear")
                            .ghost()
                            .on_click(Command::new(Self::clear_toasts)),
                    ),
            )
            .child(Text::new("Placement").style(body_style))
            .child(
                Flex::new(Axis::Horizontal)
                    .gap(dp(10.0))
                    .wrap(Wrap::Wrap)
                    .child(
                        Button::new("TopStart")
                            .ghost()
                            .on_click(Command::new(Self::push_top_start)),
                    )
                    .child(
                        Button::new("TopCenter")
                            .ghost()
                            .on_click(Command::new(Self::push_top_center)),
                    )
                    .child(
                        Button::new("TopEnd")
                            .ghost()
                            .on_click(Command::new(Self::push_top_end)),
                    )
                    .child(
                        Button::new("BottomStart")
                            .ghost()
                            .on_click(Command::new(Self::push_bottom_start)),
                    )
                    .child(
                        Button::new("BottomCenter")
                            .ghost()
                            .on_click(Command::new(Self::push_bottom_center)),
                    )
                    .child(
                        Button::new("BottomEnd")
                            .ghost()
                            .on_click(Command::new(Self::push_bottom_end)),
                    ),
            )
            .child(Text::new(self.status.signal()).style(status_style))
            .into()
    }
}

impl ViewModel for ToastDemoVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            status: ctx.state("尚未触发 Toast 操作".to_string()),
            main_queue: ToastQueue::new(ctx),
            top_start_queue: ToastQueue::new(ctx),
            top_center_queue: ToastQueue::new(ctx),
            top_end_queue: ToastQueue::new(ctx),
            bottom_start_queue: ToastQueue::new(ctx),
            bottom_center_queue: ToastQueue::new(ctx),
        }
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(28.0)))
            .center()
            .child(self.controls())
            .child(ToastHost::new(self.main_queue.clone()).style(toast_style))
            .child(
                ToastHost::new(self.top_start_queue.clone())
                    .placement(ToastPlacement::TopStart)
                    .style(toast_style),
            )
            .child(
                ToastHost::new(self.top_center_queue.clone())
                    .placement(ToastPlacement::TopCenter)
                    .style(toast_style),
            )
            .child(
                ToastHost::new(self.top_end_queue.clone())
                    .placement(ToastPlacement::TopEnd)
                    .style(toast_style),
            )
            .child(
                ToastHost::new(self.bottom_start_queue.clone())
                    .placement(ToastPlacement::BottomStart)
                    .style(toast_style),
            )
            .child(
                ToastHost::new(self.bottom_center_queue.clone())
                    .placement(ToastPlacement::BottomCenter)
                    .style(toast_style),
            )
            .into()
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .app_id("com.example.toast_snackbar")
        .title("tgui Toast / Snackbar")
        .window_size(dp(960.0), dp(640.0))
        .with_view_model(ToastDemoVm::new)
        .root_view(ToastDemoVm::view)
        .run()
}
