//! Popover：锚定到 trigger 的轻量浮层。
//!
//! Popover 不是独立的 `WidgetKind`，而是任意 widget 的可选修饰（`Element::popover`）。
//! 它介于 Tooltip 与 Modal 之间：支持任意内容子树、外部点击 / `Esc` 关闭、
//! 以及 click 固定打开 + hover 预览。

mod descriptor;
mod widget;

pub use descriptor::{PopoverDescriptor, PopoverTriggerMode};
pub use widget::Popover;
