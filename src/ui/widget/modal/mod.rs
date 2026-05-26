//! Modal / Dialog 应用内对话框组件。
//!
//! - [`Modal`]：阻塞式应用内对话框 widget builder；
//! - [`ModalAction`]：动作区按钮描述符。
//!
//! Modal 与 [`tgui::dialog`](crate::dialog) 区分——后者是系统原生对话框。
//! Modal 由 builder 构造出一个绝对定位、全屏覆盖的 Stack 子树：semi-transparent
//! backdrop + 居中 card。card 上自动启用 `FocusScopeOptions::trap(true)`。
//! Esc 关闭、focus return、关闭命令派发通过 collect 阶段额外 emit 的 sentinel
//! overlay piggyback runtime overlay close 机制。

mod action;
mod descriptor;
mod widget;

pub use action::ModalAction;
pub(crate) use descriptor::ModalDescriptor;
pub use widget::Modal;
