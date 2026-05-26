//! Modal / Dialog 应用内对话框示例。
//!
//! 展示三种典型用法：
//! 1. alert（标题 + 信息 + 单一 OK 按钮）
//! 2. confirm（标题 + 信息 + Cancel / Confirm 双按钮）
//! 3. 自定义内容（标题 + 输入框 + Save / Cancel）
//!
//! 三个 modal 均演示 fade 动画、Esc 关闭、点击 backdrop 关闭、Tab focus trap，
//! Enter 触发主按钮（Button 自带 DefaultActivation::EnterAndSpace + 主按钮
//! tab_index=0）。

use tgui::prelude::*;

struct DemoVm {
    alert_open: State<bool>,
    confirm_open: State<bool>,
    confirm_result: State<String>,
    form_open: State<bool>,
    form_name: TextController,
    form_submitted: State<String>,
}

impl DemoVm {
    fn open_alert(&mut self) {
        self.alert_open.set(true);
    }

    fn dismiss_alert(&mut self, _open: bool) {
        self.alert_open.set(false);
    }

    fn open_confirm(&mut self) {
        self.confirm_open.set(true);
    }

    fn confirm_cancel(&mut self) {
        self.confirm_result.set("已取消".into());
        self.confirm_open.set(false);
    }

    fn confirm_yes(&mut self) {
        self.confirm_result.set("已确认".into());
        self.confirm_open.set(false);
    }

    fn dismiss_confirm(&mut self, _open: bool) {
        self.confirm_open.set(false);
    }

    fn open_form(&mut self) {
        self.form_open.set(true);
    }

    fn dismiss_form(&mut self, _open: bool) {
        self.form_open.set(false);
    }

    fn submit_form(&mut self) {
        let name = self.form_name.snapshot().text;
        self.form_submitted.set(if name.is_empty() {
            "（未填写）".into()
        } else {
            name
        });
        self.form_open.set(false);
    }
}

impl ViewModel for DemoVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            alert_open: ctx.state(false),
            confirm_open: ctx.state(false),
            confirm_result: ctx.state("尚未触发 confirm".to_string()),
            form_open: ctx.state(false),
            form_name: ctx.text_controller(""),
            form_submitted: ctx.state("尚未提交".to_string()),
        }
    }

    fn view(&self) -> Element<Self> {
        // 主界面：垂直 Flex，3 个触发按钮 + 状态文本。
        let main_ui = Flex::new(Axis::Vertical)
            .padding(Insets::all(dp(32.0)))
            .gap(dp(16.0))
            .child(
                Text::new("tgui Modal / Dialog Demo")
                    .style(|mode| {
                        let mut s = TextWidgetStyle::default_for(mode);
                        s.typography.size = sp(24.0);
                        s
                    }),
            )
            .child(
                Text::new("点击下方按钮试用三种用法。Esc 关闭、Tab 在 modal 内循环、Enter 触发主按钮。"),
            )
            .child(
                Flex::new(Axis::Horizontal)
                    .gap(dp(12.0))
                    .child(
                        Button::new("Alert（提示）").on_click(Command::new(Self::open_alert)),
                    )
                    .child(
                        Button::new("Confirm（二选一）").on_click(Command::new(Self::open_confirm)),
                    )
                    .child(
                        Button::new("自定义内容（表单）").on_click(Command::new(Self::open_form)),
                    ),
            )
            .child(
                Flex::new(Axis::Vertical)
                    .gap(dp(4.0))
                    .child(Text::new(
                        self.confirm_result
                            .signal()
                            .map(|r| format!("Confirm 结果：{r}")),
                    ))
                    .child(Text::new(
                        self.form_submitted
                            .signal()
                            .map(|r| format!("表单提交内容：{r}")),
                    )),
            );

        // alert modal：一个 OK 按钮
        let alert_modal = Modal::new(self.alert_open.signal())
            .on_open_change(ValueCommand::new(Self::dismiss_alert))
            .title("提示")
            .content(Text::new("这是一个简单的 alert modal。按 Esc 或点击 backdrop 也能关闭。"))
            .action(
                ModalAction::primary("OK")
                    .on_click(Command::new(|vm: &mut Self| vm.alert_open.set(false))),
            );

        // confirm modal：Cancel + Confirm 双按钮
        let confirm_modal = Modal::new(self.confirm_open.signal())
            .on_open_change(ValueCommand::new(Self::dismiss_confirm))
            .title("确认操作")
            .content(Text::new("是否继续此操作？此操作不可撤销。"))
            .action(ModalAction::new("取消").on_click(Command::new(Self::confirm_cancel)))
            .action(ModalAction::primary("确认").on_click(Command::new(Self::confirm_yes)));

        // 表单 modal：任意子树作为内容（Input + Text）
        let form_modal = Modal::new(self.form_open.signal())
            .on_open_change(ValueCommand::new(Self::dismiss_form))
            .title("编辑名称")
            .content(
                Flex::new(Axis::Vertical)
                    .gap(dp(8.0))
                    .child(Text::new("请填写名称（可为空）："))
                    .child(Input::new(self.form_name.clone()).placeholder("Anonymous")),
            )
            .action(ModalAction::new("取消").on_click(Command::new(|vm: &mut Self| {
                vm.form_open.set(false)
            })))
            .action(ModalAction::primary("保存").on_click(Command::new(Self::submit_form)));

        // 根 Stack：主界面 + 三个 modal 叠加在上面。
        // Modal 内部使用 size(100%,100%) 充满父 Stack，并以 Stack 内 align/justify=Center
        // 居中 card——保证只在 open=true 时可见，关闭后 fade 到 alpha=0。
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .child(main_ui)
            .child(alert_modal)
            .child(confirm_modal)
            .child(form_modal)
            .into()
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .window_size(dp(720.0), dp(480.0))
        .with_view_model(DemoVm::new)
        .root_view(DemoVm::view)
        .run()
}
