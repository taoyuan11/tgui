//! Menu / ContextMenu / MenuBar 三件套：层级化操作命令。
//!
//! - [`Menu`]：按钮触发的下拉操作菜单；
//! - [`ContextMenu`]：右键 / 长按触发的浮层菜单；
//! - [`MenuBar`]：顶部主菜单条，每个条目展开一个 Menu。
//!
//! 三件套共享同一套 [`MenuItem`] / [`MenuIcon`] / [`KeyChord`] 数据类型。
//! 菜单本体通过修饰符（`Element::menu` / `Element::context_menu`）挂在任意
//! widget 上——与 [`Tooltip`](crate::widgets::Tooltip) 同源风格。

mod contextmenu;
mod descriptor;
mod menubar;
mod types;
mod widget;

pub use contextmenu::ContextMenu;
pub(crate) use descriptor::{ContextMenuDescriptor, MenuDescriptor, MenuItemState};
pub use menubar::{MenuBar, MenuBarEntry};
pub(crate) use types::menu_item_state_owner;
pub use types::{ChordKey, KeyChord, MenuIcon, MenuItem, MenuItemKind};
pub use widget::Menu;
