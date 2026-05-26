//! [`ModalAction`] —— Modal 动作区单个按钮的描述符。
//!
//! 一个 Modal 通常配一组动作按钮（例如 "Cancel" + "OK" / "Yes" + "No"）。
//! `primary` 标记主按钮，渲染时使用 `ButtonVariantKind::Primary`，否则用
//! `ButtonVariantKind::Secondary`。主按钮自动获得 `tab_index(0)`，使得
//! Modal 打开后按 Tab 第一次落到主按钮，按 Enter 触发其 `on_click`（依赖
//! `DefaultActivation::EnterAndSpace`）。

use std::sync::Arc;

use crate::foundation::view_model::Command;
use crate::ui::layout::Value;

/// Modal 动作区按钮描述符。
pub struct ModalAction<VM> {
    pub(crate) label: Value<String>,
    pub(crate) on_click: Option<Command<VM>>,
    pub(crate) primary: bool,
    pub(crate) disabled: Value<bool>,
}

impl<VM> Clone for ModalAction<VM> {
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            on_click: self.on_click.clone(),
            primary: self.primary,
            disabled: self.disabled.clone(),
        }
    }
}

impl<VM> ModalAction<VM> {
    /// 创建一个次按钮（secondary）样式的动作。
    pub fn new(label: impl Into<Value<String>>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
            primary: false,
            disabled: Value::Static(false),
        }
    }

    /// 创建一个主按钮（primary）样式的动作。
    /// 主按钮获得 `tab_index(0)`，Modal 打开按 Tab 首次落到此按钮。
    pub fn primary(label: impl Into<Value<String>>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
            primary: true,
            disabled: Value::Static(false),
        }
    }

    /// 设置点击命令。
    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.on_click = Some(command);
        self
    }

    /// 禁用本按钮。
    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }
}

impl<VM> ModalAction<VM> {
    /// 把 `Command<VM>` scope 到父 view model。配合 `Modal::scope` / 嵌套使用。
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> ModalAction<RootVm>
    where
        VM: 'static,
    {
        ModalAction {
            label: self.label,
            on_click: self.on_click.map(|cmd| cmd.scope(selector)),
            primary: self.primary,
            disabled: self.disabled,
        }
    }
}
