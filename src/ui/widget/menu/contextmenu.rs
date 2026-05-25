//! [`ContextMenu`] widget builder。
//!
//! ContextMenu 是一个修饰符，把任意 child element 包起来并挂上 `context_menu`
//! 描述符。长按 / 鼠标右键自动通过 `GestureRecognizer::on_long_press` 触发
//! `on_show(LongPressEvent)` 回调——调用方在 callback 里写入自己的
//! `State<bool>`（open）与 `State<Point>`（anchor），然后这两个 State
//! 信号驱动菜单浮层的可见性。点击外部 / Esc 会通过 overlay 的统一关闭
//! 通道触发 `on_open_change(false)`，调用方在那里清掉 open State。
//!
//! 这是当前 MVP 接法。把 open / anchor 收编进 runtime 内部状态（无需用户
//! 维护两个 State）属于路线图长尾——需要给 `CollectContext` 加 anchor 字段
//! 并贯穿调用链 + 在 gesture 派发里直接写入 runtime state，单独 PR 落地。
//!
//! ```ignore
//! use tgui::mvvm::{State, ViewModelContext};
//! use tgui::widgets::{ContextMenu, Image, LongPressEvent, MenuItem};
//!
//! struct PhotoVm {
//!     ctx_open: State<bool>,
//!     ctx_anchor: State<tgui::core::Point>,
//! }
//!
//! impl PhotoVm {
//!     fn view(&self) -> tgui::widgets::Element<Self> {
//!         ContextMenu::new(Image::new("photo.png"))
//!             .items(vec![
//!                 MenuItem::new("复制").on_select(/* ... */),
//!                 MenuItem::new("删除").on_select(/* ... */),
//!             ])
//!             // 长按/右键时写两个 State
//!             .on_show(tgui::mvvm::ValueCommand::new(
//!                 |vm: &mut Self, ev: LongPressEvent| {
//!                     vm.ctx_open.set(true);
//!                     vm.ctx_anchor.set(ev.position);
//!                 },
//!             ))
//!             // 外部点击 / Esc / item 选中 → on_open_change(false)
//!             .on_open_change(tgui::mvvm::ValueCommand::new(
//!                 |vm: &mut Self, open: bool| vm.ctx_open.set(open),
//!             ))
//!             .into()
//!     }
//! }
//! ```

use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::Value;
use crate::ui::widget::core::Element;
use crate::ui::widget::gesture::{GestureRecognizer, LongPressEvent};
use crate::ui::widget::style::MenuStyle;

use super::descriptor::{ContextMenuDescriptor, MenuItemState};
use super::types::MenuItem;

/// ContextMenu widget。
pub struct ContextMenu<VM> {
    child: Element<VM>,
    items: Vec<MenuItem<VM>>,
    on_show: Option<ValueCommand<VM, LongPressEvent>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    disabled: Value<bool>,
    style: Option<MenuStyle>,
}

impl<VM> ContextMenu<VM> {
    /// 用一个 child element 包出 ContextMenu。
    pub fn new(child: impl Into<Element<VM>>) -> Self {
        Self {
            child: child.into(),
            items: Vec::new(),
            on_show: None,
            on_open_change: None,
            disabled: Value::Static(false),
            style: None,
        }
    }

    /// 设置所有菜单项。
    pub fn items(mut self, items: Vec<MenuItem<VM>>) -> Self {
        self.items = items;
        self
    }

    /// 追加单个菜单项。
    pub fn item(mut self, item: MenuItem<VM>) -> Self {
        self.items.push(item);
        self
    }

    /// 长按 / 右键触发时的回调，参数携带触发坐标。调用方应在 callback 里
    /// 写入自己的 open / anchor State。
    pub fn on_show(mut self, command: ValueCommand<VM, LongPressEvent>) -> Self {
        self.on_show = Some(command);
        self
    }

    /// 菜单关闭时的回调（外部点击 / Esc / item 选中触发）。
    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    /// 禁用整个 ContextMenu（child 仍可视、但长按不会触发菜单）。
    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        self.disabled = disable.into();
        self
    }

    /// 覆盖默认主题样式。
    pub fn style(mut self, style: MenuStyle) -> Self {
        self.style = Some(style);
        self
    }
}

impl<VM> From<ContextMenu<VM>> for Element<VM>
where
    VM: 'static,
{
    fn from(ctx_menu: ContextMenu<VM>) -> Element<VM> {
        let ContextMenu {
            mut child,
            items,
            on_show,
            on_open_change,
            disabled,
            style,
        } = ctx_menu;

        // 接长按手势：长按 / 右键命中时调用 on_show，把触发坐标传出去。
        if let Some(on_show) = on_show {
            let recognizer = match child.interactions.gesture.take() {
                Some(existing) => existing.on_long_press(on_show),
                None => GestureRecognizer::new().on_long_press(on_show),
            };
            child.interactions.gesture = Some(recognizer);
        }

        let descriptor = ContextMenuDescriptor {
            items: items.into_iter().map(MenuItemState::from_public).collect(),
            on_open_change,
            disabled,
            style,
        };
        child.context_menu = Some(descriptor);
        child
    }
}
