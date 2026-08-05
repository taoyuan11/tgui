//! [`ContextMenu`] widget builder。
//!
//! ContextMenu 是一个修饰符，把任意 child element 包起来并挂上 `context_menu`
//! 描述符。长按 / 鼠标右键自动通过 `GestureRecognizer::on_long_press` 触发
//! `on_show(LongPressEvent)` 回调。打开状态与锚点由 runtime 维护；`on_show` 用于观察
//! 触发来源/坐标，`on_open_change` 会收到实际的开合通知。点击外部 / Esc 或选择菜单项
//! 都会关闭浮层。
//!
//! ```ignore
//! use tgui::core::Point;
//! use tgui::widgets::{ContextMenu, Image, LongPressEvent, MenuItem};
//!
//! struct PhotoVm {
//!     context_open: bool,
//!     last_context_position: Option<Point>,
//! }
//!
//! impl PhotoVm {
//!     fn view(&self) -> tgui::widgets::Element<Self> {
//!         ContextMenu::new(Image::new("photo.png"))
//!             .items(vec![
//!                 MenuItem::new("复制").on_select(/* ... */),
//!                 MenuItem::new("删除").on_select(/* ... */),
//!             ])
//!             // 可选：观察长按/右键来源与坐标
//!             .on_show(tgui::mvvm::ValueCommand::new(
//!                 |vm: &mut Self, ev: LongPressEvent| {
//!                     vm.last_context_position = Some(ev.position);
//!                 },
//!             ))
//!             // 外部点击 / Esc / item 选中也会通知 false
//!             .on_open_change(tgui::mvvm::ValueCommand::new(
//!                 |vm: &mut Self, open: bool| vm.context_open = open,
//!             ))
//!             .into()
//!     }
//! }
//! ```

use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::Value;
use crate::ui::theme::StyleContext;
use crate::ui::widget::core::Element;
use crate::ui::widget::gesture::{GestureRecognizer, LongPressEvent};
use crate::ui::widget::style::{MenuStyle, StyleResolver};

use super::descriptor::{ContextMenuDescriptor, MenuItemState};
use super::types::MenuItem;

/// ContextMenu widget。
pub struct ContextMenu<VM> {
    child: Element<VM>,
    items: Vec<MenuItem<VM>>,
    on_show: Option<ValueCommand<VM, LongPressEvent>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    disabled: Value<bool>,
    style: Option<StyleResolver<MenuStyle>>,
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

    /// 长按 / 右键触发时的回调，参数携带触发来源与坐标。开合状态与锚点由 runtime
    /// 维护，调用方无需额外保存它们。
    pub fn on_show(mut self, command: ValueCommand<VM, LongPressEvent>) -> Self {
        self.on_show = Some(command);
        self
    }

    /// 菜单开合变化回调（右键/长按打开，以及外部点击 / Esc / item 选中关闭）。
    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    /// 禁用整个 ContextMenu（child 仍可视、但长按不会触发菜单）。
    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        self.disabled = disable.into();
        self
    }

    /// Patch the theme-derived style.
    pub fn style(
        mut self,
        mutator: impl Fn(&mut MenuStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| MenuStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    /// Replace the full resolved style.
    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> MenuStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
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

        // 始终保证存在长按识别器，让 runtime 能自动打开内部 ContextMenu。用户已经
        // 安装的 long-press handler 必须保留；ContextMenu 自己的 `on_show` 存在描述符
        // 中，由 runtime 对长按和右键统一派发，避免真实右键序列重复调用。
        let noop = || ValueCommand::new(|_: &mut VM, _: LongPressEvent| {});
        let recognizer = match child.interactions.gesture.take() {
            Some(existing) if existing.on_long_press.is_some() => existing,
            Some(existing) => existing.on_long_press(noop()),
            None => GestureRecognizer::new().on_long_press(noop()),
        };
        child.interactions.gesture = Some(recognizer);

        let descriptor = ContextMenuDescriptor {
            items: items.into_iter().map(MenuItemState::from_public).collect(),
            on_show,
            on_open_change,
            disabled,
            style,
        };
        child.context_menu = Some(Box::new(descriptor));
        child
    }
}
