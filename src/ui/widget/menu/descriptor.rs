//! MenuDescriptor / ContextMenuDescriptor —— 挂在 Element 上的菜单修饰符。
//!
//! 设计参考 `Tooltip`：菜单本身不是独立的 `WidgetKind`，而是任意 widget 的
//! 可选修饰。collect 阶段统一在 widget 的 trigger frame 之上调用 Overlay 引擎渲染。
//!
//! 内部 `MenuItemState<VM>` 是从公开 `MenuItem<VM>` 烤干后留下的运行时表示——
//! 字段全部为已 scope 过、可直接 dispatch 的 `Command<VM>` / `Value<T>`。

use std::sync::Arc;

use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::Value;
use crate::ui::widget::overlay::{Alignment, FlipPolicy, Placement};
use crate::ui::widget::style::MenuStyle;

use super::types::{KeyChord, MenuBarGroupId, MenuIcon, MenuItem, MenuItemKind};

/// Menu / ContextMenu 内部存储的运行时项状态。所有 `Command<VM>` 已 scope 完毕。
pub(crate) struct MenuItemState<VM> {
    pub kind: MenuItemKind,
    pub label: Option<Value<String>>,
    pub icon: Option<MenuIcon>,
    pub shortcut_hint: Option<Value<String>>,
    pub shortcut: Option<KeyChord>,
    pub disabled: Value<bool>,
    pub checked: Option<Value<bool>>,
    pub on_select: Option<Command<VM>>,
    pub submenu: Vec<MenuItemState<VM>>,
}

impl<VM> Clone for MenuItemState<VM> {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            label: self.label.clone(),
            icon: self.icon.clone(),
            shortcut_hint: self.shortcut_hint.clone(),
            shortcut: self.shortcut.clone(),
            disabled: self.disabled.clone(),
            checked: self.checked.clone(),
            on_select: self.on_select.clone(),
            submenu: self.submenu.clone(),
        }
    }
}

impl<VM> MenuItemState<VM> {
    pub(crate) fn from_public(item: MenuItem<VM>) -> Self {
        Self {
            kind: item.kind,
            label: item.label,
            icon: item.icon,
            shortcut_hint: item.shortcut_hint,
            shortcut: item.shortcut,
            disabled: item.disabled,
            checked: item.checked,
            on_select: item.on_select,
            submenu: item
                .submenu
                .into_iter()
                .map(MenuItemState::from_public)
                .collect(),
        }
    }

    /// 把所有 `Command<VM>` 重新映射到父 view model。其它 `Value<T>` 字段
    /// 不变（它们已是数据，不绑定 view model）。
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> MenuItemState<RootVm>
    where
        VM: 'static,
    {
        MenuItemState {
            kind: self.kind,
            label: self.label,
            icon: self.icon,
            shortcut_hint: self.shortcut_hint,
            shortcut: self.shortcut,
            disabled: self.disabled,
            checked: self.checked,
            on_select: self.on_select.map(|cmd| cmd.scope(selector.clone())),
            submenu: self
                .submenu
                .into_iter()
                .map(|child| child.scope(selector.clone()))
                .collect(),
        }
    }
}

/// Menu 描述符。挂在 `Element::menu` 上，由 collect 阶段读取。
///
/// 公开字段大部分通过 `Menu` widget builder 设置；这里只暴露内部
/// `pub(crate)` 字段以便 runtime / collect 访问。
pub struct MenuDescriptor<VM> {
    pub(crate) items: Vec<MenuItemState<VM>>,
    pub(crate) open: Value<bool>,
    pub(crate) on_open_change: Option<ValueCommand<VM, bool>>,
    pub(crate) placement: Placement,
    pub(crate) flip_policy: FlipPolicy,
    pub(crate) disabled: Value<bool>,
    pub(crate) style: Option<MenuStyle>,
    /// 关联到同一个 MenuBar 的标记（None=独立 Menu）。
    pub(crate) menubar_group: Option<MenuBarGroupId>,
    /// 在所属 MenuBar 里的索引（与 menubar_group 同时出现或都为 None）。
    pub(crate) menubar_index: Option<usize>,
    /// 由 MenuBar 注入：调用即可把 MenuBar 的 active_index 切到目标条目；
    /// runtime 的 Left/Right 键导航靠这条命令切相邻 entry。
    pub(crate) menubar_set_active: Option<ValueCommand<VM, Option<usize>>>,
}

impl<VM> Clone for MenuDescriptor<VM> {
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
            open: self.open.clone(),
            on_open_change: self.on_open_change.clone(),
            placement: self.placement,
            flip_policy: self.flip_policy,
            disabled: self.disabled.clone(),
            style: self.style.clone(),
            menubar_group: self.menubar_group,
            menubar_index: self.menubar_index,
            menubar_set_active: self.menubar_set_active.clone(),
        }
    }
}

impl<VM> MenuDescriptor<VM> {
    pub(crate) fn new(items: Vec<MenuItem<VM>>) -> Self {
        Self {
            items: items.into_iter().map(MenuItemState::from_public).collect(),
            open: Value::Static(false),
            on_open_change: None,
            placement: Placement::bottom().align(Alignment::Start),
            flip_policy: FlipPolicy::FlipSide,
            disabled: Value::Static(false),
            style: None,
            menubar_group: None,
            menubar_index: None,
            menubar_set_active: None,
        }
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> MenuDescriptor<RootVm>
    where
        VM: 'static,
    {
        MenuDescriptor {
            items: self
                .items
                .into_iter()
                .map(|item| item.scope(selector.clone()))
                .collect(),
            open: self.open,
            on_open_change: self.on_open_change.map(|cmd| cmd.scope(selector.clone())),
            placement: self.placement,
            flip_policy: self.flip_policy,
            disabled: self.disabled,
            style: self.style,
            menubar_group: self.menubar_group,
            menubar_index: self.menubar_index,
            menubar_set_active: self
                .menubar_set_active
                .map(|cmd| cmd.scope(selector.clone())),
        }
    }

    /// 按主题模式解析最终样式（用户未提供则取主题默认值）。
    pub(crate) fn resolved_style(&self, mode: ResolvedThemeMode) -> MenuStyle {
        self.style
            .clone()
            .unwrap_or_else(|| MenuStyle::default_for(mode))
    }
}

/// ContextMenu 描述符。挂在 `Element::context_menu` 上。
///
/// 与 `MenuDescriptor` 的差异：
/// - 触发由 runtime 自动接长按 / 右键事件，不需要 `open` 字段；
/// - anchor 为长按 / 右键点击的 viewport 坐标（runtime 写入 `context_menu_anchor`），
///   而非 widget frame。
pub struct ContextMenuDescriptor<VM> {
    pub(crate) items: Vec<MenuItemState<VM>>,
    pub(crate) on_open_change: Option<ValueCommand<VM, bool>>,
    pub(crate) disabled: Value<bool>,
    pub(crate) style: Option<MenuStyle>,
}

impl<VM> Clone for ContextMenuDescriptor<VM> {
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
            on_open_change: self.on_open_change.clone(),
            disabled: self.disabled.clone(),
            style: self.style.clone(),
        }
    }
}

impl<VM> ContextMenuDescriptor<VM> {
    pub(crate) fn new(items: Vec<MenuItem<VM>>) -> Self {
        Self {
            items: items.into_iter().map(MenuItemState::from_public).collect(),
            on_open_change: None,
            disabled: Value::Static(false),
            style: None,
        }
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> ContextMenuDescriptor<RootVm>
    where
        VM: 'static,
    {
        ContextMenuDescriptor {
            items: self
                .items
                .into_iter()
                .map(|item| item.scope(selector.clone()))
                .collect(),
            on_open_change: self.on_open_change.map(|cmd| cmd.scope(selector)),
            disabled: self.disabled,
            style: self.style,
        }
    }

    /// 按主题模式解析最终样式。
    pub(crate) fn resolved_style(&self, mode: ResolvedThemeMode) -> MenuStyle {
        self.style
            .clone()
            .unwrap_or_else(|| MenuStyle::default_for(mode))
    }
}
