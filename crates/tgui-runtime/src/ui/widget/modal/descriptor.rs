//! ModalDescriptor —— 挂在 `Element::modal` 上的修饰符。
//!
//! 与 `MenuDescriptor` / `ContextMenuDescriptor` / `Tooltip` 同位：collect 阶段
//! 由 `src/ui/widget/core/resolved/collect/modal.rs` 读取此描述符，按 `open`
//! 状态解算动画 visibility、注册 sentinel overlay（用于 Esc 关闭 / focus 返回 /
//! `on_close` 派发），并向 ComputedScene 注入"backdrop / card visibility"通道。
//!
//! card / backdrop 本身是 Modal builder 在 `Element` 主树里构造的普通 widget
//! 子树（详见 `widget.rs`）。

use std::sync::Arc;

use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::Value;
use crate::ui::widget::style::{ModalStyle, StyleResolver};
use crate::ui::widget::WidgetId;

/// 挂在 `Element::modal` 上的 Modal 状态描述符。
pub(crate) struct ModalDescriptor<VM> {
    pub(crate) open: Value<bool>,
    pub(crate) on_open_change: Option<ValueCommand<VM, bool>>,
    pub(crate) close_on_escape: bool,
    pub(crate) close_on_backdrop_click: bool,
    pub(crate) return_focus_to: Option<WidgetId>,
    pub(crate) backdrop_widget_id: WidgetId,
    pub(crate) card_widget_id: WidgetId,
    pub(crate) style: Option<StyleResolver<ModalStyle>>,
}

impl<VM> Clone for ModalDescriptor<VM> {
    fn clone(&self) -> Self {
        Self {
            open: self.open.clone(),
            on_open_change: self.on_open_change.clone(),
            close_on_escape: self.close_on_escape,
            close_on_backdrop_click: self.close_on_backdrop_click,
            return_focus_to: self.return_focus_to,
            backdrop_widget_id: self.backdrop_widget_id,
            card_widget_id: self.card_widget_id,
            style: self.style.clone(),
        }
    }
}

impl<VM> ModalDescriptor<VM> {
    /// 把 `on_open_change` 重新映射到父 view model。其它字段不变。
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> ModalDescriptor<RootVm>
    where
        VM: 'static,
    {
        ModalDescriptor {
            open: self.open,
            on_open_change: self.on_open_change.map(|cmd| cmd.scope(selector)),
            close_on_escape: self.close_on_escape,
            close_on_backdrop_click: self.close_on_backdrop_click,
            return_focus_to: self.return_focus_to,
            backdrop_widget_id: self.backdrop_widget_id,
            card_widget_id: self.card_widget_id,
            style: self.style,
        }
    }
}
